use crate::download_data::DownloadData;
use crate::misc::FutureVec;
use crate::segment::SegmentStatusGetter;
use crate::{
    idownloaditem::IRunningDownloadItem,
    length_getter,
    segment::{self, DownloadRunError, DownloadStatusCheckError, Segment, SegmentData},
    segmented_verifier,
    segmenter::{self, segment_into_portions},
    statuses::{self, DownloadStatus},
};
use async_trait::async_trait;
use futures::future::join_all;
use getset;
use getset::{CloneGetters, Getters};
use log::info;
use reqwest::{self};
use serde::Serialize;
use std::path::{self, Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::{fs, io::AsyncWriteExt};

#[derive(Getters, CloneGetters)]
pub struct SegmentedRunningDownloadItem {
    #[getset(get = "pub")]
    segments_to_run: Vec<segment::Segment>,
    #[getset(get = "pub")]
    status_getters: Vec<segment::SegmentStatusGetter>,
    #[getset(get_clone = "pub")]
    client: reqwest::Client,
    status_sender: tokio::sync::watch::Sender<statuses::DownloadStatus>,
    status_reciever: tokio::sync::watch::Receiver<statuses::DownloadStatus>,
    #[getset(get = "pub")]
    url: String,
    #[getset(get = "pub")]
    filepath: std::path::PathBuf,
    #[getset(get_copy = "pub")]
    filesize: u64,
    #[getset(get_copy = "pub")]
    polling_rate_ms: u64,
}

#[derive(Error, Debug, Serialize, Clone)]
pub enum DownloadSerializedError {
    #[error("Given path can't be converted to unicode")]
    NonUnicodePath,
    #[error("Cannot serialize download data. Reason: {0}")]
    SerializationError(String),
    #[error("Failed to write download data to disk. Reason: {0}")]
    WriteError(String),
}
impl From<serde_json::Error> for DownloadSerializedError {
    fn from(err: serde_json::Error) -> Self {
        DownloadSerializedError::SerializationError(err.to_string())
    }
}
impl From<std::io::Error> for DownloadSerializedError {
    fn from(err: std::io::Error) -> Self {
        DownloadSerializedError::WriteError(err.to_string())
    }
}

#[derive(Error, Debug, Serialize, Clone)]
#[serde(tag = "kind", content = "payload")]
pub enum SegmentedDownloadItemCreationError {
    #[error("Request failed! Error: {0}")]
    RequestError(String),
    #[error("There are more segments than length")]
    SegmentationLengthError(#[from] segmenter::MakingSegmentError),
    #[error("Server doesn't supported segmented downloads")]
    SegmentFailed(#[from] segmented_verifier::SegmentVerificationError),
    #[error("Given path can't be converted to unicode")]
    NonUnicodePath,
    #[error("Failed to get length from server")]
    LengthGettingError(#[from] length_getter::LengthGettingErrors),
    #[error("Cannot serialize download data to disk")]
    SerializationError(#[from] DownloadSerializedError),
    #[error("Invalid filepath: {0}")]
    InvalidFilepathError(String),
    #[error("There is no segment to deserialize in resume")]
    NoSegmentsInDownloadData,
}
impl From<reqwest::Error> for SegmentedDownloadItemCreationError {
    fn from(err: reqwest::Error) -> Self {
        SegmentedDownloadItemCreationError::RequestError(err.to_string())
    }
}
impl SegmentedRunningDownloadItem {
    async fn divide_into_segments(
        client: reqwest::Client,
        url: &str,
        filepath: &Path,
        number_of_segments: u64,
        length: u64,
        polling_rate: u64,
    ) -> Result<Vec<(Segment, SegmentStatusGetter)>, SegmentedDownloadItemCreationError> {
        let segment_ranges = segment_into_portions(length, number_of_segments)?;
        let mut required_segments: Vec<(Segment, SegmentStatusGetter)> = vec![];
        for (i, segment_range) in segment_ranges.iter().enumerate() {
            let segment_path = format!("{}.{}", filepath.to_str().unwrap(), i);
            info!("Segment path: {}", segment_path);
            let segment_data = SegmentData {
                start: segment_range.start,
                end: segment_range.end,
                url: url.to_string(),
                filepath: segment_path.clone().into(),
            };
            let segment_and_getter =
                Segment::new(client.clone(), segment_data, polling_rate).await?;
            required_segments.push(segment_and_getter);
        }
        Ok(required_segments)
    }
    async fn create_helper(
        client: reqwest::Client,
        url: &str,
        filepath: PathBuf,
        number_of_segments: u64,
        polling_rate_ms: u64,
    ) -> Result<SegmentedRunningDownloadItem, SegmentedDownloadItemCreationError> {
        let filepath_to_download = &filepath;
        filepath_to_download.file_name().ok_or(
            SegmentedDownloadItemCreationError::InvalidFilepathError(
                filepath.to_str().unwrap().to_string(),
            ),
        )?;
        let length = length_getter::get_length(client.clone(), url).await?;
        let required_segments_and_getters = Self::divide_into_segments(
            client.clone(),
            url,
            &filepath,
            number_of_segments,
            length,
            polling_rate_ms,
        )
        .await?;
        let (required_segments, required_segment_getters) =
            required_segments_and_getters.into_iter().unzip();
        let (status_sender, status_reciever) =
            tokio::sync::watch::channel(statuses::DownloadStatus::Running {
                total: Some(length),
                downloaded: 0,
            });
        let required_segment = Self {
            segments_to_run: required_segments,
            status_getters: required_segment_getters,
            client,
            status_sender,
            status_reciever,
            url: url.to_string(),
            filepath: filepath_to_download.to_path_buf(),
            filesize: length,
            polling_rate_ms,
        };
        required_segment.serialize_data().await?;
        Ok(required_segment)
    }
    pub async fn new(
        client: reqwest::Client,
        url: &str,
        filepath: PathBuf,
        number_of_segments: u64,
        polling_rate_ms: u64,
    ) -> Result<Arc<SegmentedRunningDownloadItem>, SegmentedDownloadItemCreationError> {
        let required_item = Arc::new(
            Self::create_helper(client, url, filepath, number_of_segments, polling_rate_ms).await?,
        );
        let required_item_clone = required_item.clone();
        let handle = tokio::spawn(async move {
            let _ = required_item_clone.start_internal().await;
        });
        std::mem::drop(handle);
        Ok(required_item)
    }
    async fn resume(
        client: reqwest::Client,
        filepath: PathBuf,
        polling_rate_ms: u64,
    ) -> Result<SegmentedRunningDownloadItem, SegmentedDownloadItemCreationError> {
        let metadata_file_name: String =
            format!("{}.json", filepath.file_name().unwrap().to_str().unwrap());
        let metadata_path = filepath
            .parent()
            .ok_or(SegmentedDownloadItemCreationError::InvalidFilepathError(
                "Given download path is not a file".to_string(),
            ))?
            .join(metadata_file_name);
        let filepath_to_download = &filepath;
        filepath_to_download.file_name().ok_or(
            SegmentedDownloadItemCreationError::InvalidFilepathError(
                filepath.to_str().unwrap().to_string(),
            ),
        )?;
        let data = fs::read_to_string(metadata_path).await.unwrap();
        let required_data: DownloadData = serde_json::from_str(&data).unwrap();
        let segment_datas_maybe = required_data.segments;
        if segment_datas_maybe.is_none() {
            return Err(SegmentedDownloadItemCreationError::SegmentFailed(
                segmented_verifier::SegmentVerificationError::NoSegments,
            ));
        }
        let segment_datas = segment_datas_maybe.unwrap();
        let mut required_segments: Vec<Segment> = vec![];
        let mut required_segment_getters: Vec<SegmentStatusGetter> = vec![];
        for segment_data in segment_datas {
            let (segment, getter) =
                Segment::resume_from_file(client.clone(), segment_data, polling_rate_ms).await?;
            required_segments.push(segment);
            required_segment_getters.push(getter);
        }
        let (status_sender, status_reciever) =
            tokio::sync::watch::channel(statuses::DownloadStatus::Running {
                total: required_data.length,
                downloaded: 0,
            });
        let required_item = Self {
            segments_to_run: required_segments,
            status_getters: required_segment_getters,
            client,
            status_sender,
            status_reciever,
            url: required_data.url,
            filepath: filepath_to_download.to_path_buf(),
            filesize: required_data.length.unwrap(),
            polling_rate_ms,
        };
        Ok(required_item)
    }
    pub async fn create_with_resume(
        client: reqwest::Client,
        filepath: path::PathBuf,
        polling_rate_ms: u64,
    ) -> Result<Arc<SegmentedRunningDownloadItem>, SegmentedDownloadItemCreationError> {
        let required_item = Arc::new(Self::resume(client, filepath, polling_rate_ms).await?);
        let required_item_clone = required_item.clone();
        let handle = tokio::spawn(async move {
            let _ = required_item_clone.start_internal().await;
        });
        std::mem::drop(handle);
        Ok(required_item)
    }
    pub async fn serialize_data(&self) -> Result<(), DownloadSerializedError> {
        let to_serialize = self.get_download_data();
        //There must be a filename or something has gone horribly wrong!!
        let filename = self.filepath.file_name().unwrap();
        let to_serialize_to = format!("{}.json", filename.to_str().unwrap());
        let serialized = serde_json::to_string_pretty(&to_serialize)?;
        fs::write(
            path::Path::join(self.filepath.parent().unwrap(), to_serialize_to),
            serialized,
        )
        .await?;
        Ok(())
    }
}

impl SegmentedRunningDownloadItem {
    async fn start_helper(&self) -> Result<(), DownloadRunError> {
        let status_sender = self.status_sender.clone();
        let _ = status_sender.send(statuses::DownloadStatus::Running {
            downloaded: 0,
            total: Some(self.filesize),
        });
        let mut futures: FutureVec<Result<(), DownloadRunError>> = vec![];
        for segment in self.segments_to_run.iter() {
            futures.push(Box::pin(segment.start()));
        }
        //Download in segments
        let results = join_all(futures).await;
        let current_status = status_sender.borrow().clone();
        if let DownloadStatus::Paused = current_status {
            return Ok(());
        }
        for result in results {
            //Propagate any errors
            result?;
        }
        let filepath_to_merge_to = self.filepath.clone();
        //This must've succeeded so far
        let mut final_file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(filepath_to_merge_to)
            .await?;
        for segment in &self.segments_to_run {
            let data_read = fs::read(&segment.data.filepath).await?;
            final_file.write_all(&data_read).await?;
            fs::remove_file(&segment.data.filepath).await?;
        }
        let _ = status_sender.send(DownloadStatus::Completed);
        Ok(())
    }
    async fn start_internal(&self) -> Result<(), DownloadRunError> {
        let err_maybe = self.start_helper().await;
        if let Err(err) = err_maybe {
            let _ = self
                .status_sender
                .send(statuses::DownloadStatus::Failed(err));
        }
        Ok(())
    }
}

#[async_trait]
impl IRunningDownloadItem for SegmentedRunningDownloadItem {
    fn get_status(&self) -> Result<statuses::DownloadStatus, segment::DownloadStatusCheckError> {
        let current_status_maybe = self.status_reciever.borrow();
        let current_status_maybe_clone = current_status_maybe.clone();
        let current_status = current_status_maybe_clone;
        if let DownloadStatus::Paused = current_status {
            Ok(DownloadStatus::Paused)
        } else if let DownloadStatus::Running {
            total,
            downloaded: _,
        } = current_status
        {
            let segment_statuses_results: Vec<Result<DownloadStatus, DownloadStatusCheckError>> =
                self.status_getters
                    .iter()
                    .map(|getter| getter.get_status())
                    .collect();
            let mut segment_statuses: Vec<DownloadStatus> = vec![];
            for result in segment_statuses_results {
                segment_statuses.push(result?);
            }
            let mut total_downloaded: u64 = 0;
            for (i, segment_status) in segment_statuses.iter_mut().enumerate() {
                if let statuses::DownloadStatus::Running {
                    downloaded: segment_downloaded,
                    total: _,
                } = segment_status
                {
                    total_downloaded += *segment_downloaded;
                } else if let statuses::DownloadStatus::Completed = segment_status {
                    total_downloaded += self.segments_to_run[i].length();
                }
            }
            Ok(statuses::DownloadStatus::Running {
                downloaded: total_downloaded,
                total,
            })
        } else {
            Ok(current_status.clone())
        }
    }
    fn filename(&self) -> String {
        self.filepath
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    }
    fn filepath(&self) -> String {
        if self.filepath.is_dir() {
            return self.filepath.to_str().unwrap().to_string();
        }
        self.filepath.to_str().unwrap().to_string()
    }
    fn get_update_interval_ms(&self) -> u64 {
        self.polling_rate_ms
    }
    fn get_url(&self) -> String {
        self.url.clone()
    }
    fn pause(&self) {
        for segment in &self.segments_to_run {
            segment.pause();
        }
        let _ = self.status_sender.send(statuses::DownloadStatus::Paused);
    }
    fn get_status_reciever(&self) -> tokio::sync::watch::Receiver<statuses::DownloadStatus> {
        self.status_reciever.clone()
    }
    fn get_download_data(&self) -> DownloadData {
        DownloadData {
            url: self.url.clone(),
            filepath: self.filepath.to_string_lossy().to_string(),
            length: Some(self.filesize),
            segments: Some(
                self.segments_to_run
                    .iter()
                    .map(|segment| segment.data.clone())
                    .collect(),
            ),
        }
    }
}
