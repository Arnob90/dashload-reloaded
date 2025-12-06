use crate::normal_download_strategies::{self, FullDownloadStrategy, GetFileError};
use crate::segmented_verifier::{SegmentVerificationError, verify_segmented};
use crate::statuses::DownloadStatus;
use log::info;
use reqwest::{self, Client};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
// --- Composition: Common data is in one place ---
#[derive(Clone, Serialize, Deserialize)] // Clone is useful for retrying
pub struct SegmentData {
    pub start: u64,
    pub end: Option<u64>,
    pub url: String,
    pub filepath: PathBuf,
}

pub struct Segment {
    pub data: SegmentData,
    pub sender: watch::Sender<DownloadStatus>,
    reciever: watch::Receiver<DownloadStatus>,
    pub client: Client,
    pub polling_rate: u64,
    pub strategy: Box<dyn normal_download_strategies::DownloadStrategy>,
}
pub enum GetFilePathError {
    RequestError(reqwest::Error),
    IllBehavedServer,
}
#[derive(Error, Debug, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DeleteSegmentError {
    #[error("Failed to delete the segment")]
    DeleteError(String),
}

pub struct SegmentStatusGetter {
    pub reciever: watch::Receiver<DownloadStatus>,
}

#[derive(Debug, Error, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadRunError {
    #[error("Failed to open file")]
    FileOpenError,
    #[error("Failed to seek file")]
    ServerNotAvailable,
    #[error("Failed to seek file")]
    FileSeekError,
    #[error("Cannot open file to write error, reason: {0}")]
    FileWriteError(String),
    #[error("Download cannot be run twice")]
    AlreadyRunning,
    #[error("The file does not match the one kept before resumption")]
    FileMismatch,
    #[error("Given path is one to a dir")]
    DirectoryGivenError,
    #[error("Failed to create or resume from file")]
    FileCreationError(GetFileError),
    #[error("Failed to verify that the download can be segmented")]
    SegmentVerificationError(SegmentVerificationError),
}

impl From<std::io::Error> for DownloadRunError {
    fn from(value: std::io::Error) -> Self {
        Self::FileWriteError(value.to_string())
    }
}

impl From<reqwest::Error> for DownloadRunError {
    fn from(value: reqwest::Error) -> Self {
        Self::SegmentVerificationError(SegmentVerificationError::RequestError(value.to_string()))
    }
}

#[derive(Debug, Error, Serialize, Clone)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadStatusCheckError {
    #[error("Download is dropped and cannot be checked")]
    DroppedDownloadError,
    #[error("Error on running download")]
    DownloadRunError(DownloadRunError),
}

impl Segment {
    pub async fn new(
        client: reqwest::Client,
        given_data: SegmentData,
        polling_rate: u64,
    ) -> Result<(Segment, SegmentStatusGetter), SegmentVerificationError> {
        if given_data.end.is_none() {
            return Err(SegmentVerificationError::RangeError);
        }
        let strategy = normal_download_strategies::ResumedDownloadStrategy {
            start_offset: given_data.start,
            segment_start_offset: given_data.start,
            end: Some(given_data.end.unwrap()),
            truncate_on_start: true,
        };
        let (sender, reciever) = watch::channel(DownloadStatus::Running {
            total: None,
            downloaded: 0,
        });
        let reciever_clone = reciever.clone();
        let required_segment = Segment {
            data: given_data,
            sender,
            client,
            polling_rate,
            reciever: reciever_clone,
            strategy: Box::new(strategy),
        };
        Ok((required_segment, SegmentStatusGetter { reciever }))
    }
    pub async fn use_single_segment(
        client: reqwest::Client,
        url: &str,
        filepath: PathBuf,
        polling_rate: u64,
    ) -> Result<(Segment, SegmentStatusGetter), SegmentVerificationError> {
        //No need to verify segment here obviously
        let strategy = FullDownloadStrategy {};
        let (sender, reciever) = watch::channel(DownloadStatus::Running {
            total: None,
            downloaded: 0,
        });
        let reciever_clone = reciever.clone();
        let required_segment = Segment {
            data: SegmentData {
                start: 0,
                end: None,
                url: url.to_string(),
                filepath,
            },
            sender,
            client,
            polling_rate,
            strategy: Box::new(strategy),
            reciever: reciever_clone,
        };
        Ok((required_segment, SegmentStatusGetter { reciever }))
    }
    pub async fn resume_from_file(
        client: reqwest::Client,
        given_data: SegmentData,
        polling_rate: u64,
    ) -> Result<(Segment, SegmentStatusGetter), SegmentVerificationError> {
        verify_segmented(
            client.clone(),
            &given_data.url,
            given_data.start,
            given_data.end,
        )
        .await?;
        let file = fs::metadata(&given_data.filepath)
            .await
            .map_err(|_| SegmentVerificationError::NoMetadataFile)?;
        let len = file.len();
        let global_offset = given_data.start + len;
        let strategy = normal_download_strategies::ResumedDownloadStrategy {
            start_offset: global_offset,
            segment_start_offset: given_data.start,
            end: given_data.end,
            truncate_on_start: false,
        };
        let (sender, reciever) = watch::channel(DownloadStatus::Running {
            total: None,
            downloaded: 0,
        });
        let reciever_clone = reciever.clone();
        let required_segment = Segment {
            data: given_data,
            sender,
            client,
            polling_rate,
            strategy: Box::new(strategy),
            reciever: reciever_clone,
        };
        Ok((required_segment, SegmentStatusGetter { reciever }))
    }
    pub fn length(&self) -> Option<u64> {
        self.data.end.map(|end| end - self.data.start + 1)
    }
    fn get_terminal_status_according_to_state(
        downloaded: u64,
        total: Option<u64>,
        current_state: DownloadStatus,
    ) -> Option<DownloadStatus> {
        if matches!(current_state, DownloadStatus::Paused { .. }) {
            return Some(DownloadStatus::Paused { downloaded, total });
        } else if matches!(current_state, DownloadStatus::Stopped) {
            return Some(DownloadStatus::Stopped);
        }
        return None;
    }
    pub async fn start(&self) -> Result<(), DownloadRunError> {
        let _ = self.sender.send(DownloadStatus::Running {
            total: self.length(),
            downloaded: self.strategy.get_start() - self.data.start,
            download_speed: 0,
        });
        let mut res = self
            .strategy
            .run(self.client.clone(), &self.data.url)
            .await?;
        println!("I am here 0");
        info!(
            "Current filepath {}",
            &self.data.filepath.to_string_lossy().to_string()
        );
        let mut file = self
            .strategy
            .get_file(&self.data.filepath)
            .await
            .map_err(DownloadRunError::FileCreationError)?;
        let mut downloaded: u64 = self.strategy.get_start() - self.data.start;
        let mut polling_interval = tokio::time::interval(Duration::from_millis(self.polling_rate));
        println!("I am here 1");
        loop {
            let current_status = self.sender.borrow().clone();
            if matches!(
                current_status,
                DownloadStatus::Paused { .. } | DownloadStatus::Stopped
            ) {
                return Ok(());
            }
            let mut reciever_to_use = self.reciever.clone();
            tokio::select! {
                    _ = reciever_to_use.changed() => {
                        let current_status = reciever_to_use.borrow().clone();
                        if matches!(
                            current_status,
                            DownloadStatus::Paused | DownloadStatus::Stopped
                        ) {
                            return Ok(());
                        }
                    },
                    next_chunk_maybe_err = res.chunk() => {
                    let next_chunk_maybe = next_chunk_maybe_err.map_err(|e|DownloadRunError::SegmentVerificationError(SegmentVerificationError::RequestError(e.to_string())))?;
                    if next_chunk_maybe.is_none() {
                        break;
                    }
                    let next_chunk = next_chunk_maybe.unwrap();
                    let chunk_size = next_chunk.len() as u64;
                    downloaded += chunk_size;
                    file.write_all(&next_chunk)
                        .await
                        .map_err(|_| DownloadRunError::FileOpenError)?;
                },
                _ = polling_interval.tick() => {
                    let _ = self.sender.send(DownloadStatus::Running { total: self.length(), downloaded });
                }
            }
            let _ = self
                .sender
                .send(DownloadStatus::Completed { total: downloaded });
        }
        Ok(())
    }
    pub fn pause(&self) {
        //We don't how much we have downloaded yet
        let _ = self.sender.send(DownloadStatus::Paused {
            total: self.length(),
            downloaded: 0,
        });
    }
    pub async fn delete(&self) -> Result<(), DeleteSegmentError> {
        let _ = self.sender.send(DownloadStatus::Stopped);
        tokio::fs::remove_file(&self.data.filepath)
            .await
            .map_err(|e| DeleteSegmentError::DeleteError(e.to_string()))
    }
}

impl SegmentStatusGetter {
    pub fn get_status(&self) -> Result<DownloadStatus, DownloadStatusCheckError> {
        return Ok(self.reciever.borrow().clone());
    }
}
