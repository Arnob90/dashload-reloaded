use crate::commandenum::{Command, CommandMaybe, construct_command_sender_reciever};
use crate::downloadrunerror::DownloadRunError;
use crate::normal_download_strategies::{self, FullDownloadStrategy};
use crate::segmented_verifier::{SegmentVerificationError, verify_segmented};
use crate::statuses::{DownloadStatus, calculate_download_speed, calculate_eta_in_ms};
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
#[derive(Clone, Serialize, Deserialize, Debug)] // Clone is useful for retrying
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
    command_reciever: watch::Receiver<Option<Command>>,
    pub command_sender: watch::Sender<Option<Command>>,
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
    #[error("No status to get")]
    NoStatusError,
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
            download_speed: 0.0,
            eta_ms: None,
            downloaded_portion: Some(0.0),
        });
        let reciever_clone = reciever.clone();
        let (command_sender, command_reciever) = construct_command_sender_reciever();
        let required_segment = Segment {
            data: given_data,
            sender,
            client,
            polling_rate,
            reciever: reciever_clone,
            strategy: Box::new(strategy),
            command_sender,
            command_reciever,
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
            download_speed: 0.0,
            eta_ms: None,
            downloaded_portion: Some(0.0),
        });
        let reciever_clone = reciever.clone();
        let (command_sender, command_reciever) = construct_command_sender_reciever();
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
            command_sender,
            command_reciever,
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
            download_speed: 0.0,
            eta_ms: None,
            downloaded_portion: Some(0.0),
        });
        let reciever_clone = reciever.clone();
        let (command_sender, command_reciever) = construct_command_sender_reciever();
        let required_segment = Segment {
            data: given_data,
            sender,
            client,
            polling_rate,
            strategy: Box::new(strategy),
            reciever: reciever_clone,
            command_reciever,
            command_sender,
        };
        Ok((required_segment, SegmentStatusGetter { reciever }))
    }
    pub fn length(&self) -> Option<u64> {
        self.data.end.map(|end| end - self.data.start + 1)
    }
    fn get_state_if_terminal(
        current_command: CommandMaybe,
        downloaded: u64,
        total: Option<u64>,
    ) -> Option<DownloadStatus> {
        match current_command {
            Some(Command::Pause) => Some(DownloadStatus::Paused { downloaded, total }),
            Some(Command::Stop) => Some(DownloadStatus::Stopped),
            None => None,
        }
    }
    pub async fn start(&self) -> Result<(), DownloadRunError> {
        let total = self.length();
        let _ = self.sender.send(DownloadStatus::Running {
            total,
            downloaded: self.strategy.get_start() - self.data.start,
            download_speed: 0.0,
            eta_ms: None,
            downloaded_portion: Some(0.0),
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
        let mut previous_downloaded: u64 = 0;
        let mut polling_interval = tokio::time::interval(Duration::from_millis(self.polling_rate));
        println!("I am here 1");
        loop {
            let current_command = self.command_sender.borrow().clone();
            let current_state = Segment::get_state_if_terminal(current_command, downloaded, total);
            if let Some(current_state) = current_state {
                let _ = self.sender.send(current_state);
                return Ok(());
            }
            let mut command_reciever_clone = self.command_reciever.clone();
            tokio::select! {
                    _ = command_reciever_clone.changed() => {
                        let current_command = self.command_reciever.borrow().clone();
                        let current_state = Segment::get_state_if_terminal(current_command,downloaded, total);
                        if let Some(current_state) = current_state {
                            let _ = self.sender.send(current_state);
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
                    let download_speed = calculate_download_speed(previous_downloaded, downloaded, polling_interval.period());
                    let eta = total.and_then(|total| calculate_eta_in_ms(total, download_speed));
                    let _ = self.sender.send(DownloadStatus::Running { total: self.length(), downloaded, download_speed, eta_ms: eta, downloaded_portion: self.length().map(|len|(downloaded as f64)/(len as f64))});
                    previous_downloaded = downloaded;
                }
            }
            let _ = self
                .sender
                .send(DownloadStatus::Completed { total: downloaded });
        }
        Ok(())
    }
    pub fn pause(&self) {
        let _ = self.command_sender.send(Some(Command::Pause));
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
