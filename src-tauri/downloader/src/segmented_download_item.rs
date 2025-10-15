use crate::lazy::AsyncLazy;
use crate::misc::FutureVec;
use crate::{
    filename_getter,
    idownloaditem::IDownloadItem,
    length_getter,
    segment::{self, DownloadRunError, DownloadStatusCheckError, Segment, SegmentData},
    segmented_verifier,
    segmenter::{self, segment_into_portions},
    statuses::{self, DownloadStatus},
};
use async_trait::async_trait;
use filename_getter::get_filename;
use futures::future::join_all;
use getset;
use getset::{CloneGetters, Getters};
use reqwest::{self};
use serde::{Deserialize, Serialize};
use std::path;
use thiserror::Error;
use tokio::{fs, io::AsyncWriteExt};
#[derive(Serialize, Deserialize)]
pub struct DownloadData {
    pub url: String,
    pub filepath: String,
    pub segments: Option<Vec<SegmentData>>,
}

#[derive(Getters, CloneGetters)]
pub struct SegmentedDownloadItem {
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
    filename: crate::lazy::AsyncLazy<String>,
}

#[derive(Error, Debug, Serialize)]
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

#[derive(Error, Debug, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadItemCreationError {
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
}
impl From<reqwest::Error> for DownloadItemCreationError {
    fn from(err: reqwest::Error) -> Self {
        DownloadItemCreationError::RequestError(err.to_string())
    }
}
impl SegmentedDownloadItem {
    pub async fn new(
        client: reqwest::Client,
        url: &str,
        filepath_str: &str,
        number_of_segments: u64,
        polling_rate_ms: u64,
    ) -> Result<SegmentedDownloadItem, DownloadItemCreationError> {
        let filepath_to_download = path::Path::new(filepath_str);
        let full_path = if filepath_to_download.is_dir() {
            let filename = filename_getter::get_filename(client.clone(), url)
                .await
                .unwrap_or("untitled".to_string());
            filepath_to_download.join(filename)
        } else {
            filepath_to_download.to_path_buf()
        };

        let full_path_str = full_path
            .to_str()
            .ok_or(DownloadItemCreationError::NonUnicodePath)?
            .to_string();
        let length = length_getter::get_length(client.clone(), url).await?;
        let segment_ranges = segment_into_portions(length, number_of_segments)?;
        let mut required_segments: Vec<Segment> = vec![];
        let mut required_segment_getters: Vec<segment::SegmentStatusGetter> = vec![];
        for (i, segment_range) in segment_ranges.iter().enumerate() {
            let segment_path = format!("{}.{}", full_path_str, i);
            let segment_data = SegmentData {
                start: segment_range.start,
                end: segment_range.end,
                url: url.to_string(),
                filepath: segment_path.clone(),
            };
            let (required_segment, segment_status_getter) =
                Segment::new(client.clone(), segment_data, polling_rate_ms).await?;
            required_segments.push(required_segment);
            required_segment_getters.push(segment_status_getter);
        }
        let (status_sender, status_reciever) =
            tokio::sync::watch::channel(statuses::DownloadStatus::Pending);
        let filepath_clone = full_path.clone();
        let url_clone = url.to_string();
        let client_clone = client.clone();
        let filename = AsyncLazy::new(move || {
            let client = client_clone.clone();
            let url = url_clone.to_string();
            let filepath = filepath_clone.clone();
            async move {
                if filepath.is_file() {
                    return filepath.file_name().unwrap().to_str().unwrap().to_string();
                }
                get_filename(client, &url)
                    .await
                    .unwrap_or("untitled".to_string())
            }
        });
        let required_segment = Self {
            segments_to_run: required_segments,
            status_getters: required_segment_getters,
            client,
            status_sender,
            status_reciever,
            url: url.to_string(),
            filepath: full_path,
            filesize: length,
            polling_rate_ms,
            filename,
        };
        required_segment.serialize_data().await?;
        Ok(required_segment)
    }
    pub async fn serialize_data(&self) -> Result<(), DownloadSerializedError> {
        let to_serialize = DownloadData {
            url: self.url.clone(),
            filepath: self
                .filepath
                .to_str()
                .ok_or(DownloadSerializedError::NonUnicodePath)?
                .to_string(),
            segments: Some(
                self.segments_to_run
                    .iter()
                    .map(|segment| segment.data.clone())
                    .collect(),
            ),
        };
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

impl SegmentedDownloadItem {
    async fn start_helper(&self) -> Result<(), DownloadRunError> {
        let mut status_reviever_borrow_maybe_err = self.status_reciever.borrow().clone();
        if let DownloadStatus::Failed(current_err) = status_reviever_borrow_maybe_err {
            if current_err == DownloadRunError::LengthlessServerError
                || current_err == DownloadRunError::AlreadyRunning
                || current_err == DownloadRunError::IllBehavedServer
                || current_err == DownloadRunError::RangeError
                || current_err == DownloadRunError::ServerNotRangable
            {
                return Err(current_err);
            }
            let _ = self.status_sender.send(statuses::DownloadStatus::Pending);
            status_reviever_borrow_maybe_err = statuses::DownloadStatus::Pending;
        }
        let current_status = status_reviever_borrow_maybe_err.clone();
        if current_status != statuses::DownloadStatus::Pending {
            return Err(DownloadRunError::AlreadyRunning);
        }
        let status_sender = &self.status_sender;
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
        for result in results {
            //Propagate any errors
            result?;
        }
        //This must've succeeded so far
        let mut final_file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&self.filepath)
            .await?;
        for segment in &self.segments_to_run {
            let data_read = fs::read(&segment.data.filepath).await?;
            final_file.write_all(&data_read).await?;
            fs::remove_file(&segment.data.filepath).await?;
        }
        let _ = status_sender.send(DownloadStatus::Completed);
        Ok(())
    }
}

#[async_trait]
impl IDownloadItem for SegmentedDownloadItem {
    async fn start(&self) -> Result<(), DownloadRunError> {
        let err_maybe = self.start_helper().await;
        if let Err(err) = err_maybe {
            let _ = self
                .status_sender
                .send(statuses::DownloadStatus::Failed(err));
        }
        return Ok(());
    }
    fn get_status(&self) -> Result<statuses::DownloadStatus, segment::DownloadStatusCheckError> {
        let current_status_maybe = self.status_reciever.borrow();
        let current_status_maybe_clone = current_status_maybe.clone();
        if let statuses::DownloadStatus::Failed(current_status_err) = current_status_maybe_clone {
            return Err(segment::DownloadStatusCheckError::DownloadRunError(
                current_status_err,
            ));
        }
        let current_status = current_status_maybe_clone;
        if let DownloadStatus::Running {
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
            return Ok(statuses::DownloadStatus::Running {
                downloaded: total_downloaded,
                total,
            });
        }
        Ok(current_status.clone())
    }
    async fn filename(&self) -> String {
        self.filename.get().await
    }
    async fn filepath(&self) -> String {
        if self.filepath.is_dir() {
            return self
                .filepath
                .join(self.filename.get().await)
                .to_str()
                .unwrap()
                .to_string();
        }
        self.filepath.to_str().unwrap().to_string()
    }
    fn get_update_interval_ms(&self) -> u64 {
        self.polling_rate_ms
    }
    fn get_url(&self) -> String {
        self.url.clone()
    }
}
