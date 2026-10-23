use crate::download_run_error::DownloadRunError;
use crate::download_status::{DownloadState, DownloadStatus};
use crate::info_getters::get_filesize;
use crate::request_headers::RequestHeaders;
use crate::segmented_verifier::SegmentVerificationError;
use bytes::Bytes;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::watch;
use tokio::time::{Duration, Instant};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentInfo {
    pub start: u64,
    pub end: u64,
}
impl SegmentInfo {
    pub fn size(&self) -> u64 {
        self.end - self.start + 1
    }
}
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum SegmentCommand {
    Stop,
    Pause,
    Nothing,
}

pub enum NextChunkMaybe {
    Interrupted(SegmentCommand),
    Chunk(Option<Bytes>),
}

#[derive(Debug, Clone, Serialize, Deserialize)] // Clone is useful for retrying
pub struct InPlaceSegmentData {
    pub url: String,
    pub filepath: PathBuf,
    pub last_downloaded: u64,
    pub info: Option<SegmentInfo>,
    pub headers: RequestHeaders,
}
pub struct InPlaceSegment {
    data: InPlaceSegmentData,
    pub sender: watch::Sender<DownloadStatus>,
    pub client: Client,
    pub polling_rate: u64,
    pub command_reciever: watch::Receiver<SegmentCommand>,
}

fn get_range_header(data: &InPlaceSegmentData) -> Option<String> {
    match (&data.info, data.last_downloaded) {
        // Ranged segment
        (Some(i), downloaded) => Some(format!("bytes={}-{}", i.start + downloaded, i.end)),
        // Non-ranged segment being resumed
        (None, downloaded) if downloaded > 0 => Some(format!("bytes={}-", downloaded)),
        // Fresh download starting at 0
        _ => None,
    }
}

fn calculate_speed(download_diff: u64, time_diff: Duration) -> f64 {
    let secs_diff = time_diff.as_secs_f64();
    (download_diff as f64) / secs_diff
}

impl InPlaceSegment {
    async fn get_total_size(client: reqwest::Client, data: &InPlaceSegmentData) -> Option<u64> {
        if let Some(info) = &data.info {
            return Some(info.size());
        }
        get_filesize(client, &data.url, &data.headers).await
    }
    pub async fn new(
        client: reqwest::Client,
        given_data: InPlaceSegmentData,
        polling_rate: u64,
        command_reciever: watch::Receiver<SegmentCommand>,
        prev_state: DownloadState,
    ) -> Result<(InPlaceSegment, watch::Receiver<DownloadStatus>), SegmentVerificationError> {
        let (sender, reciever) = watch::channel(DownloadStatus {
            total: InPlaceSegment::get_total_size(client.clone(), &given_data).await,
            downloaded: given_data.last_downloaded,
            state: prev_state,
        });
        let required_segment = InPlaceSegment {
            data: given_data,
            sender,
            client,
            polling_rate,
            command_reciever,
        };
        Ok((required_segment, reciever))
    }
    async fn get_next_chunk_or_cancel(
        &self,
        res: &mut Response,
    ) -> Result<NextChunkMaybe, DownloadRunError> {
        let mut reciever = self.command_reciever.clone();
        Ok(tokio::select! {
            next_chunk = res.chunk() => {
                NextChunkMaybe::Chunk(next_chunk?)
            }
            _ = reciever.changed() => {
                NextChunkMaybe::Interrupted(*reciever.borrow())
            }

        })
    }
    /// Public entry point: Automatically catches any error and emits `DownloadState::Failed`
    pub async fn start(&self) -> Result<(), DownloadRunError> {
        let res = self.start_internal().await;

        // If start_internal returned an Err, emit the failure status to self.sender
        if let Err(ref err) = res {
            let _ = self.sender.send(DownloadStatus {
                total: self.sender.borrow().total,
                downloaded: self.sender.borrow().downloaded,
                state: DownloadState::Failed(err.clone()),
            });
        }

        res
    }
    async fn start_internal(&self) -> Result<(), DownloadRunError> {
        let status = self.sender.borrow().clone().state;
        if !matches!(status, DownloadState::Running { .. }) {
            return Ok(());
        }
        let total_size = if let Some(info) = &self.data.info {
            Some(info.size())
        } else {
            InPlaceSegment::get_total_size(self.client.clone(), &self.data).await
        };
        if let Some(size) = total_size
            && self.data.last_downloaded >= size
        {
            let _ = self.sender.send(DownloadStatus {
                total: total_size,
                downloaded: self.data.last_downloaded,
                state: DownloadState::Completed { size },
            });
            return Ok(());
        }
        let _ = self.sender.send(DownloadStatus {
            total: total_size,
            downloaded: self.data.last_downloaded,
            state: DownloadState::Running {
                download_speed: 0.0,
            },
        });
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.data.filepath)
            .await?;
        file.seek(std::io::SeekFrom::Start(
            self.data.info.as_ref().map(|i| i.start).unwrap_or(0) + self.data.last_downloaded,
        ))
        .await?;
        let mut req = self.client.get(&self.data.url);
        let range_header = get_range_header(&self.data);
        if let Some(header) = &range_header {
            req = req.header("Range", header);
        }
        let headers = &self.data.headers;
        if let Some(cookies) = &headers.cookies {
            req = req.header("Cookie", cookies);
        }
        if let Some(referrer) = &headers.referrer {
            req = req.header("Referer", referrer);
        }
        if let Some(user_agent) = &headers.user_agent {
            req = req.header("User-Agent", user_agent);
        }
        let mut res = req.send().await?;
        let status = res.status();
        if range_header.is_some() && status != StatusCode::PARTIAL_CONTENT {
            return Err(DownloadRunError::ServerError(
                "Server does not support range requests".to_owned(),
            ));
        } else if !status.is_success() {
            return Err(DownloadRunError::NetworkError(
                "Server is unavailable".to_owned(),
            ));
        }
        let polling_time = Duration::from_millis(self.polling_rate);
        let mut last_update = Instant::now();
        let mut downloaded = self.data.last_downloaded;
        let mut last_downloaded = downloaded;
        loop {
            //Do it here at start after file has been writen, to prevent half written stuff
            let recieved_command = *self.command_reciever.borrow();
            match recieved_command {
                SegmentCommand::Stop => {
                    let _ = self.sender.send(DownloadStatus {
                        total: total_size,
                        downloaded,
                        state: DownloadState::Stopped,
                    });
                    return Ok(());
                }
                SegmentCommand::Pause => {
                    let _ = self.sender.send(DownloadStatus {
                        total: total_size,
                        downloaded,
                        state: DownloadState::Paused,
                    });

                    file.flush().await?;
                    return Ok(());
                }
                SegmentCommand::Nothing => {}
            }
            let next_chunk_or_interrupt = self.get_next_chunk_or_cancel(&mut res).await?;
            let next_chunk_maybe = match next_chunk_or_interrupt {
                NextChunkMaybe::Chunk(c) => c,
                NextChunkMaybe::Interrupted(cmd) => {
                    let status = match cmd {
                        SegmentCommand::Pause => DownloadState::Paused,
                        SegmentCommand::Stop => DownloadState::Stopped,
                        SegmentCommand::Nothing => {
                            unreachable!("Nothing should never be sent as a command!")
                        }
                    };
                    let _ = self.sender.send(DownloadStatus {
                        total: total_size,
                        downloaded,
                        state: status,
                    });
                    file.flush().await?;
                    return Ok(());
                }
            };
            if let Some(next_chunk) = next_chunk_maybe {
                let chunk_size = next_chunk.len() as u64;
                file.write_all(&next_chunk).await?;
                downloaded += chunk_size;
                if last_update.elapsed() >= polling_time {
                    let time_diff = Instant::now() - last_update;
                    let download_speed = {
                        if time_diff == Duration::ZERO {
                            0.0
                        } else {
                            calculate_speed(downloaded - last_downloaded, time_diff)
                        }
                    };
                    let _ = self.sender.send(DownloadStatus {
                        downloaded,
                        total: total_size,
                        state: DownloadState::Running { download_speed },
                    });
                    last_update = Instant::now();
                    last_downloaded = downloaded;
                }
            } else {
                break;
            }
        }
        file.flush().await?;
        let _ = self.sender.send(DownloadStatus {
            downloaded,
            total: Some(downloaded),
            state: DownloadState::Completed { size: downloaded },
        });
        Ok(())
    }
    pub fn snapshot(&self) -> InPlaceSegmentData {
        let mut snapshot = self.data.clone();
        snapshot.last_downloaded = self.sender.borrow().downloaded;
        snapshot
    }
}
