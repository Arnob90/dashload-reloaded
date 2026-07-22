use crate::normal_download_strategies::{self, GetFileError};
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
    pub end: u64,
    pub url: String,
    pub filepath: PathBuf,
}

pub struct Segment {
    pub data: SegmentData,
    pub sender: watch::Sender<DownloadStatus>,
    pub client: Client,
    pub polling_rate: u64,
    pub strategy: Box<dyn normal_download_strategies::DownloadStrategy>,
}
pub enum GetFilePathError {
    RequestError(reqwest::Error),
    IllBehavedServer,
}

pub struct SegmentStatusGetter {
    pub reciever: watch::Receiver<DownloadStatus>,
}

#[derive(Debug, Error, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadRunError {
    #[error("Request failed! Reason: {0}")]
    RequestError(String),
    #[error("The server does not have a concept of filesize")]
    LengthlessServerError,
    #[error("Given range is out of range")]
    RangeError,
    #[error("The server violates basic protocols")]
    IllBehavedServer,
    #[error("The server does not support range requests")]
    ServerNotRangable,
    #[error("Invalid header value")]
    InvalidHeaderValue, // New variant for the parse error
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
    #[error("Failed to open file")]
    GetFileError(GetFileError),
}

impl From<reqwest::Error> for DownloadRunError {
    fn from(value: reqwest::Error) -> Self {
        Self::RequestError(value.to_string())
    }
}

impl From<std::io::Error> for DownloadRunError {
    fn from(value: std::io::Error) -> Self {
        Self::FileWriteError(value.to_string())
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
        let client_to_verify = client.clone();
        verify_segmented(
            client_to_verify,
            &given_data.url,
            given_data.start,
            given_data.end,
        )
        .await?;
        let strategy = normal_download_strategies::ResumedDownloadStrategy {
            start_offset: given_data.start,
            segment_start_offset: given_data.start,
            end: Some(given_data.end),
        };
        let (sender, reciever) = watch::channel(DownloadStatus::Running {
            total: None,
            downloaded: 0,
        });
        let required_segment = Segment {
            data: given_data,
            sender,
            client,
            polling_rate,
            strategy: Box::new(strategy),
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
        println!(
            "DEBUG: Resuming segment. Path: {:?}. Metadata: {:?}",
            given_data.filepath, file
        );
        let len = file.len();
        let global_offset = given_data.start + len;
        let strategy = normal_download_strategies::ResumedDownloadStrategy {
            start_offset: global_offset,
            segment_start_offset: given_data.start,
            end: Some(given_data.end),
        };
        let (sender, reciever) = watch::channel(DownloadStatus::Running {
            total: None,
            downloaded: 0,
        });
        let required_segment = Segment {
            data: given_data,
            sender,
            client,
            polling_rate,
            strategy: Box::new(strategy),
        };
        Ok((required_segment, SegmentStatusGetter { reciever }))
    }
    pub fn length(&self) -> u64 {
        self.data.end - self.data.start + 1
    }
    pub async fn start(&self) -> Result<(), DownloadRunError> {
        let _ = self.sender.send(DownloadStatus::Running {
            total: Some(self.data.end - self.data.start + 1),
            downloaded: self.strategy.get_start() - self.data.start,
        });
        let mut res = self
            .strategy
            .run(self.client.clone(), &self.data.url)
            .await?;
        println!("I am here 0");
        if let reqwest::StatusCode::PARTIAL_CONTENT = res.status() {
            println!("I am here 1");
            info!("Current filepath {}", self.data.filepath.to_string_lossy());
            let mut file = self
                .strategy
                .get_file(&self.data.filepath)
                .await
                .map_err(DownloadRunError::GetFileError)?;
            let mut downloaded: u64 = self.strategy.get_start() - self.data.start;
            let mut polling_interval =
                tokio::time::interval(Duration::from_millis(self.polling_rate));
            println!("I am here 2");
            loop {
                if let DownloadStatus::Paused = *self.sender.borrow() {
                    return Ok(());
                }
                tokio::select! {
                    next_chunk_maybe_err = res.chunk() => {
                        let next_chunk_maybe = next_chunk_maybe_err?;
                        if let Some(next_chunk)=next_chunk_maybe {
                            let chunk_size = next_chunk.len() as u64;
                            downloaded += chunk_size;
                            file.write_all(&next_chunk)
                                .await
                                .map_err(|e| DownloadRunError::FileWriteError(e.to_string()))?;
                        }
                        else {
                            break;
                        }
                    },
                    _ = polling_interval.tick() => {
                        let _ = self.sender.send(DownloadStatus::Running { total: Some(self.length()), downloaded });
                    }
                }
            }
            let _ = self.sender.send(DownloadStatus::Completed);
        } else {
            return Err(DownloadRunError::ServerNotAvailable);
        }
        Ok(())
    }
    pub fn pause(&self) {
        let _ = self.sender.send(DownloadStatus::Paused);
    }
}

impl SegmentStatusGetter {
    pub fn get_status(&self) -> Result<DownloadStatus, DownloadStatusCheckError> {
        return Ok(self.reciever.borrow().clone());
    }
}
