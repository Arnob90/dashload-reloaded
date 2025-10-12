use crate::segmented_verifier::{SegmentVerificationError, verify_segmented};
use crate::statuses::DownloadStatus;
use reqwest::{self, Client, Request};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

// --- Composition: Common data is in one place ---
#[derive(Clone, Serialize, Deserialize)] // Clone is useful for retrying
pub struct SegmentData {
    pub start: u64,
    pub end: u64,
    pub url: String,
    pub filepath: String,
}

pub struct Segment {
    pub data: SegmentData,
    pub sender: watch::Sender<DownloadStatus>,
    pub client: Client,
    pub polling_rate: u64,
}
pub enum GetFilePathError {
    RequestError(reqwest::Error),
    IllBehavedServer,
}

pub struct SegmentStatusGetter {
    pub reciever: watch::Receiver<DownloadStatus>,
}

#[derive(Debug, Error,Clone, PartialEq, Serialize)]
#[serde(tag="kind", content="payload")]
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

#[derive(Debug, Error,Serialize)]
#[serde(tag="kind", content="payload")]
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
        polling_rate: u64
    ) -> Result<(Segment, SegmentStatusGetter), SegmentVerificationError> {
        let client_to_verify = client.clone();
        verify_segmented(
            client_to_verify,
            &given_data.url,
            given_data.start,
            given_data.end,
        )
        .await?;
        let (sender, reciever) = watch::channel(DownloadStatus::Pending);
        let required_segment = Segment {
            data: given_data,
            sender,
            client,
            polling_rate
        };
        Ok((required_segment, SegmentStatusGetter { reciever }))
    }
    pub fn length(&self) -> u64 {
        self.data.end - self.data.start + 1
    }
    pub async fn start(&self) -> Result<(), DownloadRunError> {
        let _ = self.sender.send(DownloadStatus::Running {
            total: self.data.end - self.data.start + 1,
            downloaded: 0,
        });
        let mut res = self
            .client
            .get(&self.data.url)
            .send()
            .await?;
        println!("I am here 0");
        if res.status() != reqwest::StatusCode::OK {
            return Err(DownloadRunError::ServerNotAvailable);
        }
        println!("I am here 1");
        println!("Current filepath {}", &self.data.filepath);
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&self.data.filepath)
            .await?;
        //No need to seek. Each chunk is it's own file
        let mut downloaded: u64 = 0;
        let mut polling_interval = tokio::time::interval(Duration::from_millis(self.polling_rate));
        println!("I am here 2");
        loop {
            tokio::select! {
                next_chunk_maybe_err = res.chunk() => {
                    let next_chunk_maybe = next_chunk_maybe_err?;
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
                    let _ = self.sender.send(DownloadStatus::Running { total: self.length(), downloaded: downloaded });
                }
            }
        }
        let _ = self.sender.send(DownloadStatus::Completed);
        Ok(())
    }
}

impl SegmentStatusGetter {
    pub fn get_status(&self) -> Result<DownloadStatus, DownloadStatusCheckError> {
        return Ok(self.reciever.borrow().clone());
    }
}
