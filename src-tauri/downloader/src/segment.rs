use crate::statuses::{self, DownloadStatus};
use reqwest::Error;
use reqwest::{self, Client, header::HeaderMap};
use std::future::Future;
use std::io::Write;
use std::ops::Deref;
use std::os::linux::raw::stat;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::sync::watch;
use url::{ParseError, Url};

// --- A cleaner error enum ---
#[derive(Debug, Error)] // Add Debug for easier printing
pub enum DownloadStartError {
    #[error("Server not available")]
    ServerNotAvailable,
    #[error("Request failed!")]
    RequestError(#[from] reqwest::Error),
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
}

// --- Composition: Common data is in one place ---
#[derive(Clone)] // Clone is useful for retrying
pub struct SegmentData {
    pub start: u64,
    pub end: u64,
    pub url: String,
    pub filepath: String,
    pub client: Client,
}

// --- State 1: Pending ---
pub struct PendingSegment {
    pub data: SegmentData,
}

// --- State 2: Running ---
pub struct RunningSegment {
    pub data: SegmentData,
    pub sender: watch::Sender<DownloadStatus>,
}
pub enum GetFilePathError {
    RequestError(reqwest::Error),
    IllBehavedServer,
}

pub struct SegmentStatusGetter {
    pub reciever: watch::Receiver<DownloadStatus>,
}

impl PendingSegment {
    pub fn new(start: u64, end: u64, url: String, filepath: String, client: Client) -> Self {
        Self {
            data: SegmentData {
                start,
                end,
                url,
                filepath,
                client,
            },
        }
    }

    // pub async fn get_filepath(&self) -> Result<String, GetFilePathError> {
    //     let client = self.data.client.clone();
    //     let res = client.head(&self.data.url).send().await?;
    //     let headers = res.headers();
    //     let content_dispositon = headers.get("Content-Disposition");
    //     if content_dispositon.is_none() {
    //         if &self.data.url.trim().ends_with("/") {
    //             return Err();
    //         }
    //         let filepath_url = Url::parse(&self.data.url)?;
    //     }
    //
    //
    // --- The State Transition Method ---
    // Consumes the PendingSegment and produces a RunningSegment.
    pub async fn start_running(
        self,
    ) -> Result<(RunningSegment, SegmentStatusGetter), DownloadStartError> {
        // Use the client from self.data!
        let client = &self.data.client;

        // --- Metadata Check with HEAD ---
        let response = client.head(&self.data.url).send().await?;

        if !response.status().is_success() {
            return Err(DownloadStartError::ServerNotAvailable);
        }

        if let Some(header_value) = response.headers().get("Accept-Ranges") {
            if header_value.to_str().map_or(true, |s| s != "bytes") {
                return Err(DownloadStartError::ServerNotRangable);
            }
        } else {
            return Err(DownloadStartError::ServerNotRangable);
        }

        let length_str = response
            .headers()
            .get("Content-Length")
            .ok_or(DownloadStartError::LengthlessServerError)?
            .to_str()
            .map_err(|_| DownloadStartError::IllBehavedServer)?;

        let length_num: u64 = length_str
            .parse()
            .map_err(|_| DownloadStartError::IllBehavedServer)?;

        if self.data.end >= length_num {
            return Err(DownloadStartError::RangeError);
        }

        // --- Prepare and return the GET request Future ---
        let mut headers = HeaderMap::new();
        let range_header = format!("bytes={}-{}", self.data.start, self.data.end)
            .parse()
            .map_err(|_| DownloadStartError::InvalidHeaderValue)?;
        headers.insert("Range", range_header);

        let download_future = client.get(&self.data.url).headers(headers).send().await?;
        let (sender, mut reciever) = watch::channel::<DownloadStatus>(DownloadStatus::Stopped);
        if download_future.status().is_success() {
            let required_segment = RunningSegment {
                data: self.data.clone(),
                sender,
            };
            let required_status_getter = SegmentStatusGetter { reciever };
            return Ok((required_segment, required_status_getter));
        }
        Err(DownloadStartError::ServerNotAvailable)
    }
}
#[derive(Debug, Error)]
enum DownloadRunError {
    #[error("Request failed!")]
    RequestError(#[from] reqwest::Error),
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
}

#[derive(Debug, Error)]
enum DownloadStatusCheckError {
    #[error("Failed to open file")]
    FileOpenError,
    #[error("Download is dropped and cannot be checked")]
    DroppedDownloadError,
}

impl RunningSegment {
    pub async fn start(self: Arc<RunningSegment>) -> Result<(), DownloadRunError> {
        self.sender.send(DownloadStatus::Running {
            total: self.data.end - self.data.start + 1,
            downloaded: 0,
        });
        let mut res = self
            .data
            .client
            .get(&self.data.url)
            .send()
            .await
            .map_err(DownloadRunError::RequestError)?;
        if res.status() != reqwest::StatusCode::OK {
            return Err(DownloadRunError::ServerNotAvailable);
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.data.filepath)
            .await
            .map_err(|_| DownloadRunError::FileOpenError)?;
        //No need to seek. Each chunk is it's own file
        let mut now = Instant::now();
        let mut downloaded: u64 = 0;
        loop {
            let elasped_time = now.elapsed().as_millis();
            //Poll every half second
            //TODO: Make this configurable

            if elasped_time >= 500 {
                self.sender.send(DownloadStatus::Running {
                    total: self.data.end - self.data.start + 1,
                    downloaded,
                });
                now = Instant::now();
            }
            let next_chunk_maybe = res.chunk().await.map_err(DownloadRunError::RequestError)?;
            if next_chunk_maybe.is_none() {
                break;
            }
            let next_chunk = next_chunk_maybe.unwrap();
            let chunk_size = next_chunk.len() as u64;
            downloaded += chunk_size;
            file.write_all(&next_chunk)
                .await
                .map_err(|_| DownloadRunError::FileOpenError)?;
        }
        self.sender.send(DownloadStatus::Completed);
        Ok(())
    }
}

impl SegmentStatusGetter {
    pub async fn get_status(&mut self) -> Result<DownloadStatus, DownloadStatusCheckError> {
        if self.reciever.changed().await.is_err() {
            return Err(DownloadStatusCheckError::DroppedDownloadError);
        }
        return Ok(self.reciever.borrow_and_update().clone());
    }
}
