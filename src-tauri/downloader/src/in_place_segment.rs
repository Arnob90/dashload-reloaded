use crate::download_run_error::DownloadRunError;
use crate::download_status::{DownloadState, DownloadStatus};
use crate::info_getters::get_filesize;
use crate::request_headers::RequestHeaders;
use crate::retry_scheme::RetryScheme;
use crate::segmented_verifier::SegmentVerificationError;
use bytes::Bytes;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::watch;
use tokio::time::{Duration, Instant};

/// Defines the start and end byte offsets (inclusive) for a download segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentInfo {
    pub start: u64,
    pub end: u64,
}

impl SegmentInfo {
    /// Returns the total byte size of the segment (end - start + 1).
    pub fn size(&self) -> u64 {
        self.end - self.start + 1
    }
}

/// Commands that can be sent to control segment execution via the watch channel.
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum SegmentCommand {
    /// Stop the segment and mark the download as stopped.
    Stop,
    /// Pause the segment; it can be resumed later.
    Pause,
    /// No operation; continue downloading.
    Nothing,
}

pub enum StreamChunkResult {
    /// The segment finished downloading successfully.
    Finished,
    /// The segment was interrupted by a control command (Pause/Stop).
    Interrupted(SegmentCommand),
}

/// Represents the result of fetching the next chunk from a response stream,
/// or an interruption from a control command.
pub enum NextChunkMaybe {
    /// A control command (Pause/Stop) was received.
    Interrupted(SegmentCommand),
    /// A data chunk was received (None indicates end of stream).
    Chunk(Option<Bytes>),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DownloadingState {
    Connect,
    Download,
    Retry,
    Finish,
}

/// Persistent data defining a single file segment for download or resume operations.
/// Includes URL, file path, progress, segment boundaries, and request headers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InPlaceSegmentData {
    /// The source download URL.
    pub url: String,
    /// Local file path where the segment is written.
    pub filepath: PathBuf,
    /// Number of bytes already downloaded for this segment.
    pub last_downloaded: u64,
    /// Segment boundaries (None for non-segmented/single-stream downloads).
    pub info: Option<SegmentInfo>,
    /// Custom HTTP headers for this segment's requests.
    pub headers: RequestHeaders,
}

/// Worker responsible for downloading and writing a specific file segment directly to disk in-place.
/// Manages its own HTTP connection, file handle, progress reporting, and pause/stop control.
pub struct InPlaceSegment {
    data: InPlaceSegmentData,
    /// Watch sender for publishing segment download status updates.
    pub sender: watch::Sender<DownloadStatus>,
    /// HTTP client used for chunk requests.
    pub client: Client,
    /// Polling interval in milliseconds for progress/speed updates.
    pub polling_rate: u64,
    /// Watch receiver for incoming segment control commands (pause/stop).
    pub command_reciever: watch::Receiver<SegmentCommand>,
    /// Retry scheme for download
    pub retry_scheme: RetryScheme,
}

impl InPlaceSegment {
    /// Computes the total size of this segment or the full file (for non-segmented downloads).
    async fn get_total_size(client: reqwest::Client, data: &InPlaceSegmentData) -> Option<u64> {
        if let Some(info) = &data.info {
            return Some(info.size());
        }
        // Because this comes from DownloadManager, headers should be validated by now
        get_filesize(
            client,
            &data.url,
            data.headers
                .to_header_map()
                .expect("Headers are invalid somehow"),
        )
        .await
    }

    /// Creates a new `InPlaceSegment` worker and returns its status watch receiver.
    ///
    /// # Arguments
    /// * `client` - Shared reqwest client.
    /// * `given_data` - Segment definition data.
    /// * `polling_rate` - Progress update interval in milliseconds.
    /// * `command_reciever` - Watch channel for pause/stop commands.
    /// * `prev_state` - Initial download state (typically Running).
    pub async fn new(
        client: reqwest::Client,
        given_data: InPlaceSegmentData,
        polling_rate: u64,
        command_reciever: watch::Receiver<SegmentCommand>,
        prev_state: DownloadState,
        retry_scheme: RetryScheme,
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
            retry_scheme,
        };
        Ok((required_segment, reciever))
    }

    /// Selects between receiving the next chunk from the response stream
    /// or handling a command interruption (pause/stop).
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

    /// Public entry point: starts the segment download loop.
    /// Automatically catches any error and emits `DownloadState::Failed` to the status channel.
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
    /// Reads chunks from an active HTTP response and writes them to disk.
    /// If an idle timeout or connection drop occurs, returns an Err so the caller can reconnect.
    async fn stream_chunks(
        &self,
        res: &mut reqwest::Response,
        file: &mut tokio::fs::File,
        downloaded: &mut u64,
        total_size: Option<u64>,
        chunk_wait_timeout: Duration,
    ) -> Result<StreamChunkResult, DownloadRunError> {
        let polling_time = Duration::from_millis(self.polling_rate);
        let mut last_update = Instant::now();
        let mut last_downloaded = *downloaded;

        loop {
            // Apply idle timeout between chunks
            let next_chunk_or_timeout =
                tokio::time::timeout(chunk_wait_timeout, self.get_next_chunk_or_cancel(res)).await;

            let next_chunk_maybe = match next_chunk_or_timeout {
                // Read succeeded
                Ok(Ok(NextChunkMaybe::Chunk(chunk))) => chunk,

                // User clicked Pause or Stop
                Ok(Ok(NextChunkMaybe::Interrupted(cmd))) => {
                    return Ok(StreamChunkResult::Interrupted(cmd));
                }

                // Read timed out (no data for 10s)
                Err(_) => {
                    return Err(DownloadRunError::TimeoutError);
                }

                // Socket error / connection reset
                Ok(Err(e)) => {
                    return Err(e);
                }
            };

            if let Some(next_chunk) = next_chunk_maybe {
                let chunk_size = next_chunk.len() as u64;
                file.write_all(&next_chunk).await?;
                *downloaded += chunk_size;

                if last_update.elapsed() >= polling_time {
                    let elapsed = last_update.elapsed();
                    let speed = calculate_speed(*downloaded - last_downloaded, elapsed);
                    let _ = self.sender.send(DownloadStatus {
                        downloaded: *downloaded,
                        total: total_size,
                        state: DownloadState::Running {
                            download_speed: speed,
                        },
                    });
                    last_update = Instant::now();
                    last_downloaded = *downloaded;
                }
            } else {
                // Server closed the stream cleanly (EOF)
                return Ok(StreamChunkResult::Finished);
            }
        }
    }
    async fn fetch_res_unless_interrupt(
        &self,
        req: reqwest::RequestBuilder,
        command_reciever: watch::Receiver<SegmentCommand>,
        total_size: Option<u64>,
        downloaded: u64,
        file: &mut tokio::fs::File,
    ) -> Result<Option<reqwest::Response>, DownloadRunError> {
        let mut cmd_rx = command_reciever.clone();
        let res_result = tokio::select! {
            http_res = req.send() => http_res,
            _ = cmd_rx.changed() => {
                let cmd = *cmd_rx.borrow();
                match cmd {
                    SegmentCommand::Pause => {
                        let _ = self.sender.send(DownloadStatus {
                            total: total_size,
                            downloaded,
                            state: DownloadState::Paused,
                        });
                        file.flush().await?;
                        return Ok(None);
                    }
                    SegmentCommand::Stop => {
                        let _ = self.sender.send(DownloadStatus {
                            total: total_size,
                            downloaded,
                            state: DownloadState::Stopped,
                        });
                        return Ok(None);
                    }
                    SegmentCommand::Nothing => {
                        panic!("Unexpected command received during fetch_res_unless_interrupt");
                    }
                }
            }
        };
        res_result
            .map(Some)
            .map_err(|e| DownloadRunError::NetworkError(e.to_string()))
    }
    async fn connect(
        &self,
        file: &mut tokio::fs::File,
        downloaded: u64,
        total_size: Option<u64>,
    ) -> DownloadingState {
        let base_start = 0;
        let _ = file
            .seek(std::io::SeekFrom::Start(base_start + downloaded))
            .await;
        let range_header = get_range_header(&self.data);
        let mut req = self.client.get(&self.data.url);
        let headers = &[
            ("Range", range_header.as_ref()),
            ("Cookie", self.data.headers.cookies.as_ref()),
            ("Referer", self.data.headers.referrer.as_ref()),
            ("User-Agent", self.data.headers.user_agent.as_ref()),
        ];
        for (name, value) in headers {
            if let Some(v) = value {
                req = req.header(*name, *v);
            }
        }
        let cmd_rx = self.command_reciever.clone();
        let command = *cmd_rx.borrow();
        match command {
            SegmentCommand::Stop => {
                let _ = self.sender.send(DownloadStatus {
                    total: total_size,
                    downloaded,
                    state: DownloadState::Stopped,
                });
                return DownloadingState::Retry;
            }
            SegmentCommand::Pause => {
                let _ = self.sender.send(DownloadStatus {
                    total: total_size,
                    downloaded,
                    state: DownloadState::Paused,
                });
                return DownloadingState::Retry;
            }
            SegmentCommand::Nothing => {}
        }
        let res_maybe_err = self
            .fetch_res_unless_interrupt(req, cmd_rx, total_size, downloaded, file)
            .await;
        if res_maybe_err.is_err() {
            return DownloadingState::Retry;
        }
        let res_maybe = res_maybe_err.unwrap();
        if res_maybe.is_none() {
            return DownloadingState::Finish;
        }
        let res = res_maybe.unwrap();
        todo!()
    }
    /// Internal execution loop for downloading the segment chunks directly to disk.
    /// Handles range requests, resume logic, progress polling, and pause/stop commands.
    /// In process of refactoring into a state machine
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

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.data.filepath)
            .await?;

        let mut downloaded = self.data.last_downloaded;
        let mut attempts = 0;
        //TODO: Make this configurable in the future, but for now, 10s is a reasonable default.
        let chunk_wait_timeout = Duration::from_secs(10);

        // OUTER RECONNECT / RETRY LOOP
        loop {
            // Seek to current downloaded position before connecting
            let base_start = self.data.info.as_ref().map(|i| i.start).unwrap_or(0);
            file.seek(std::io::SeekFrom::Start(base_start + downloaded))
                .await?;

            // Build fresh request from live `downloaded` offset
            let mut req = self.client.get(&self.data.url);
            let range_header = match (&self.data.info, downloaded) {
                (Some(i), d) => Some(format!("bytes={}-{}", i.start + d, i.end)),
                (None, d) if d > 0 => Some(format!("bytes={}-", d)),
                _ => None,
            };

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

            let mut cmd_rx = self.command_reciever.clone();
            let command = *cmd_rx.borrow();
            // Check for command before handshake
            match command {
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

            // Connect with cancel-safe handshake
            let res_result = tokio::select! {
                http_res = req.send() => http_res,
                _ = cmd_rx.changed() => {
                    let cmd = *cmd_rx.borrow();
                    match cmd {
                        SegmentCommand::Pause => {
                            let _ = self.sender.send(DownloadStatus {
                                total: total_size,
                                downloaded,
                                state: DownloadState::Paused,
                            });
                            file.flush().await?;
                            return Ok(());
                        }
                        SegmentCommand::Stop => {
                            let _ = self.sender.send(DownloadStatus {
                                total: total_size,
                                downloaded,
                                state: DownloadState::Stopped,
                            });
                            return Ok(());
                        }
                        SegmentCommand::Nothing => {
                            return Err(DownloadRunError::NetworkError(
                                "Unexpected command received".to_owned(),
                            ));
                        }
                    }
                }
            };

            // If initial handshake fails, retry with backoff
            let mut res = match res_result {
                Ok(r) => {
                    attempts = 0;
                    r
                }
                Err(_e) if attempts < self.retry_scheme.max_attempts => {
                    attempts += 1;
                    let delay = self.retry_internal(attempts, total_size, downloaded).await;
                    tokio::time::sleep(delay).await;
                    continue; // Reconnect!
                }
                Err(e) => return Err(DownloadRunError::NetworkError(e.to_string())),
            };

            let status = res.status();
            if range_header.is_some() && status != StatusCode::PARTIAL_CONTENT {
                return Err(DownloadRunError::ServerError(
                    "Server does not support range requests".to_owned(),
                ));
            } else if !status.is_success() {
                return Err(DownloadRunError::NetworkError(format!(
                    "Server returned HTTP {}",
                    status
                )));
            }

            // Stream chunks using the helper
            match self
                .stream_chunks(
                    &mut res,
                    &mut file,
                    &mut downloaded,
                    total_size,
                    chunk_wait_timeout,
                )
                .await
            {
                Ok(StreamChunkResult::Finished) => {
                    break; // Completed!
                }
                Ok(StreamChunkResult::Interrupted(cmd)) => {
                    let state = match cmd {
                        SegmentCommand::Pause => DownloadState::Paused,
                        SegmentCommand::Stop => DownloadState::Stopped,
                        SegmentCommand::Nothing => unreachable!(),
                    };
                    let _ = self.sender.send(DownloadStatus {
                        total: total_size,
                        downloaded,
                        state,
                    });
                    file.flush().await?;
                    return Ok(());
                }
                // On chunk read timeout or socket drop, retry with backoff
                Err(DownloadRunError::NetworkError(_) | DownloadRunError::TimeoutError)
                    if attempts < self.retry_scheme.max_attempts =>
                {
                    attempts += 1;
                    let delay = self.retry_internal(attempts, total_size, downloaded).await;
                    file.flush().await?;
                    tokio::time::sleep(delay).await;
                    continue; // Reconnects from the updated `downloaded` offset!
                }
                Err(fatal_err) => return Err(fatal_err),
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

    /// Takes a snapshot of the segment's current progress and data for state persistence.
    /// Updates `last_downloaded` from the latest status.
    pub fn snapshot(&self) -> InPlaceSegmentData {
        let mut snapshot = self.data.clone();
        snapshot.last_downloaded = self.sender.borrow().downloaded;
        snapshot
    }
    ///Note to user: YOU MUST FLUSH, SLEEP AND CONTINUE after calling this function, as it does not handle those steps itself.
    async fn retry_internal(
        &self,
        attempts: u32,
        total_size: Option<u64>,
        downloaded: u64,
    ) -> Duration {
        let delay = self.retry_scheme.backoff.next_delay(attempts - 1);
        let _ = self.sender.send(DownloadStatus {
            total: total_size,
            downloaded,
            state: DownloadState::Retrying {
                attempt: attempts,
                max_attempts: self.retry_scheme.max_attempts,
                wait_secs: delay.as_secs(),
            },
        });
        delay
    }
}

/// Constructs the HTTP `Range` header value for resuming or segmented downloads.
fn get_range_header(data: &InPlaceSegmentData) -> Option<String> {
    match (&data.info, data.last_downloaded) {
        // Ranged segment: request from current offset to segment end
        (Some(i), downloaded) => Some(format!("bytes={}-{}", i.start + downloaded, i.end)),
        // Non-ranged segment being resumed: request from current offset to end
        (None, downloaded) if downloaded > 0 => Some(format!("bytes={}-", downloaded)),
        // Fresh download starting at 0: no Range header needed
        _ => None,
    }
}

/// Calculates download speed in bytes per second.
fn calculate_speed(download_diff: u64, time_diff: Duration) -> f64 {
    let secs_diff = time_diff.as_secs_f64();
    (download_diff as f64) / secs_diff
}
