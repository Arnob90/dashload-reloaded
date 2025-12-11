use crate::commandenum::{Command, CommandMaybe, construct_command_sender_reciever};
use crate::download_data::DownloadData;
use crate::downloadrunerror::DownloadRunErrors;
use crate::misc::FutureVec;
use crate::normal_download_strategies::GetFileError;
use crate::segment::{DeleteSegmentError, SegmentStatusGetter};
use crate::segmented_verifier::verify_segmented;
use crate::{
    downloadrunerror::DownloadRunError,
    idownloaditem::DeleteFileError,
    idownloaditem::IRunningDownloadItem,
    length_getter,
    segment::{self, DownloadStatusCheckError, Segment, SegmentData},
    segmented_verifier,
    segmenter::{self, segment_into_portions},
    statuses::{self, DownloadStatus},
};
use async_trait::async_trait;
use futures::TryFutureExt;
use futures::future::join_all;
use getset;
use getset::{CloneGetters, Getters};
use log::info;
use reqwest::{self};
use serde::Serialize;
use std::path::{self, Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::watch;
use tokio::{fs, io::AsyncWriteExt};
type DownloadRunErrorsMaybe = Option<DownloadRunErrors>;
#[derive(Getters, CloneGetters)]
pub struct SegmentedRunningDownloadItem {
    #[getset(get = "pub")]
    segments_to_run: Vec<segment::Segment>,
    #[getset(get = "pub")]
    status_getters: Vec<segment::SegmentStatusGetter>,
    #[getset(get_clone = "pub")]
    client: reqwest::Client,
    command_sender: watch::Sender<CommandMaybe>,
    command_reciever: watch::Receiver<CommandMaybe>,
    #[getset(get = "pub")]
    url: String,
    #[getset(get = "pub")]
    filepath: std::path::PathBuf,
    #[getset(get_copy = "pub")]
    filesize: Option<u64>,
    #[getset(get_copy = "pub")]
    polling_rate_ms: u64,
    error_reciever: watch::Receiver<DownloadRunErrorsMaybe>,
    error_sender: watch::Sender<DownloadRunErrorsMaybe>,
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
        if segment_ranges.len() == 1 {
            let required_segment = Segment::use_single_segment(
                client.clone(),
                url,
                format!("{}.0", filepath.to_string_lossy()).into(),
                polling_rate,
            )
            .await?;
            return Ok(vec![required_segment]);
        }
        let mut required_segments: Vec<(Segment, SegmentStatusGetter)> = vec![];
        for (i, segment_range) in segment_ranges.iter().enumerate() {
            let segment_path = format!("{}.{}", filepath.to_str().unwrap(), i);
            info!("Segment path: {}", segment_path);
            let segment_data = SegmentData {
                start: segment_range.start,
                end: Some(segment_range.end),
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
        let (command_sender, command_reciever) = construct_command_sender_reciever();
        let (error_sender, error_reciever) = watch::channel::<DownloadRunErrorsMaybe>(None);
        let required_segment = Self {
            segments_to_run: required_segments,
            status_getters: required_segment_getters,
            client,
            command_sender,
            command_reciever,
            url: url.to_string(),
            filepath: filepath_to_download.to_path_buf(),
            filesize: Some(length),
            polling_rate_ms,
            error_sender,
            error_reciever,
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
            let _ = required_item_clone.start_helper().await;
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
        let segment_datas = required_data.segments;
        let mut required_segments: Vec<Segment> = vec![];
        let mut required_segment_getters: Vec<SegmentStatusGetter> = vec![];
        for segment_data in segment_datas {
            let (segment, getter) =
                Segment::resume_from_file(client.clone(), segment_data, polling_rate_ms).await?;
            required_segments.push(segment);
            required_segment_getters.push(getter);
        }
        let (command_sender, command_reciever) = construct_command_sender_reciever();
        let (error_sender, error_reciever) = watch::channel::<DownloadRunErrorsMaybe>(None);
        let required_item = Self {
            segments_to_run: required_segments,
            status_getters: required_segment_getters,
            client,
            url: required_data.url,
            filepath: filepath_to_download.to_path_buf(),
            filesize: required_data.length,
            polling_rate_ms,
            command_sender,
            command_reciever,
            error_sender,
            error_reciever,
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
            let _ = required_item_clone.start_helper().await;
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
    fn send_err(&self, err: DownloadRunError) {
        let _ = self
            .error_sender
            .send(Some(DownloadRunErrors { errors: vec![err] }));
    }
    async fn start_helper(&self) {
        //There should be at least one segment
        let first_segment = &self.segments_to_run[0];
        let verify = verify_segmented(
            self.client.clone(),
            &self.url,
            first_segment.data.start,
            first_segment.data.end,
        )
        .await
        .map_err(DownloadRunError::SegmentVerificationError);
        if let Err(err) = verify {
            self.send_err(err);
            return;
        }
        let mut futures: FutureVec<Result<(), DownloadRunError>> = vec![];
        for segment in self.segments_to_run.iter() {
            futures.push(Box::pin(segment.start()));
        }
        //Download in segments
        let results = join_all(futures).await;
        let errs: Vec<DownloadRunError> = results
            .into_iter()
            .filter(|e| e.is_err())
            .map(|e| e.unwrap_err())
            .collect();
        if !errs.is_empty() {
            let _ = self
                .error_sender
                .send(Some(DownloadRunErrors { errors: errs }));
            return;
        }
        let command = self.command_sender.borrow().clone();
        if let Some(command) = command {
            if command == Command::Pause || command == Command::Stop {
                return;
            }
        }
        let filepath_to_merge_to = self.filepath.clone();
        if self.segments_to_run.len() == 1 {
            let err_maybe =
                fs::rename(&self.segments_to_run[0].data.filepath, &self.filepath).await;
            if let Err(_) = err_maybe {
                self.send_err(DownloadRunError::FileCreationError(GetFileError::Creation));
                return;
            }
        }
        //This must've succeeded so far
        let final_file_maybe = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(filepath_to_merge_to)
            .await;
        if final_file_maybe.is_err() {
            self.send_err(DownloadRunError::FileOpenError);
            return;
        }
        let mut final_file = final_file_maybe.unwrap();
        for segment in &self.segments_to_run {
            let data_read_maybe = fs::read(&segment.data.filepath).await;
            if let Err(_) = data_read_maybe {
                self.send_err(DownloadRunError::FileOpenError);
                return;
            }
            let data_read = data_read_maybe.unwrap();
            let file_write_err = final_file.write_all(&data_read).await;
            if let Err(err) = file_write_err {
                self.send_err(DownloadRunError::FileWriteError(err.to_string()));
            }
            fs::remove_file(&segment.data.filepath).await;
        }
    }
}

#[async_trait]
impl IRunningDownloadItem for SegmentedRunningDownloadItem {
    fn get_status(&self) -> Result<statuses::DownloadStatus, segment::DownloadStatusCheckError> {
        let current_failed = self.error_reciever.borrow().clone();
        if let Some(failed) = current_failed {
            return Ok(statuses::DownloadStatus::Failed(failed));
        }
        let statuses_maybe_vec: Vec<Result<statuses::DownloadStatus, DownloadStatusCheckError>> =
            self.status_getters
                .iter()
                .map(|getter| getter.get_status())
                .collect();
        let mut status_vec: Vec<DownloadStatus> = vec![];
        for status_maybe in statuses_maybe_vec {
            status_vec.push(status_maybe?);
        }
        let first_status = status_vec
            .get(0)
            .ok_or(DownloadStatusCheckError::NoStatusError)?
            .clone();
        let mut total_downloaded: u64 = 0;
        //Speed must add up due to sum rule of derivative
        let mut total_download_speed: f64 = 0.0;
        let mut completed_num = 0;
        let status_vec_len = status_vec.len();
        for status in status_vec.into_iter() {
            if !matches!(first_status, DownloadStatus::Completed { .. }) && first_status != status {
                //We are in the midst of a phase transtion, I hope
                return Ok(DownloadStatus::Running {
                    total: None,
                    downloaded: 0,
                    download_speed: 0.0,
                    downloaded_portion: None,
                    eta_ms: None,
                });
            }
            match status {
                DownloadStatus::Running {
                    downloaded,
                    download_speed,
                    ..
                } => {
                    total_downloaded += downloaded;
                    total_download_speed += download_speed;
                }
                DownloadStatus::Stopped => {
                    return Ok(DownloadStatus::Stopped);
                }
                DownloadStatus::Paused { downloaded, .. } => {
                    total_downloaded += downloaded;
                }
                DownloadStatus::Completed { total } => {
                    total_downloaded += total;
                    completed_num += 1;
                }
                DownloadStatus::Failed(err) => {
                    return Ok(DownloadStatus::Failed(err));
                }
            }
        }
        if let DownloadStatus::Paused { .. } = first_status {
            return Ok(DownloadStatus::Paused {
                downloaded: total_downloaded,
                total: self.filesize,
            });
        }
        //Otherwise, it must be running
        else {
            if completed_num == status_vec_len {
                return Ok(DownloadStatus::Completed {
                    total: total_downloaded,
                });
            } else {
                return Ok(DownloadStatus::Running {
                    total: self.filesize,
                    downloaded: total_downloaded,
                    download_speed: total_download_speed,
                    downloaded_portion: self
                        .filesize
                        .map(|total| (total_downloaded as f64) / (total as f64)),
                    eta_ms: None,
                });
            }
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
        let _ = self.command_sender.send(Some(Command::Pause));
    }
    fn get_download_data(&self) -> DownloadData {
        DownloadData {
            url: self.url.clone(),
            filepath: self.filepath.to_string_lossy().to_string(),
            length: self.filesize,
            segments: self
                .segments_to_run
                .iter()
                .map(|segment| segment.data.clone())
                .collect(),
        }
    }
    async fn cancel(&self) -> Result<(), DeleteFileError> {
        self.command_sender.send(Some(Command::Stop));
        for segment in &self.segments_to_run {
            segment
                .delete()
                .map_err(|e| match e {
                    DeleteSegmentError::DeleteError(err) => DeleteFileError::IoError(err),
                })
                .await?
        }
        Ok(())
    }
}
