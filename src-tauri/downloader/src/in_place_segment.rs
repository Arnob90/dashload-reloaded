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
    Connect { downloaded: u64, attempts: u32 },
    Download { downloaded: u64, attempts: u32 },
    Retry { downloaded: u64, attempts: u32 },
    Finish { downloaded: u64, completed: bool },
}

/// Non-`Copy` resources owned by the state machine driver for a single run.
/// Handlers borrow this mutably so the `Response` can survive the transition
/// from `handle_connect` to `handle_stream`.
struct DownloadSession {
    file: tokio::fs::File,
    total_size: Option<u64>,
    chunk_wait_timeout: Duration,
    response: Option<reqwest::Response>,
}

/// Outcome of the cancel-safe HTTP handshake.
enum FetchResponseOutcome {
    Response(reqwest::Response),
    Paused,
    Stopped,
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
        eprintln!("Starting segment download for {}", self.data.url);
        let res = self.start_internal().await;
        // If start_internal returned an Err, emit the failure status to self.sender
        if let Err(ref err) = res {
            eprintln!("Segment download failed: {:?}", err);
            self.sender.send_modify(|status| {
                status.state = DownloadState::Failed(err.clone());
            });
            eprintln!("Sent failure status to channel");
            eprintln!("Current status: {:?}", self.sender.borrow());
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
        attempts: &mut u32,
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
                Ok(Ok(NextChunkMaybe::Chunk(chunk))) => {
                    if let Some(ref chunk) = chunk
                        && !chunk.is_empty()
                    {
                        *attempts = 0; // Reset attempts on successful read
                    }
                    eprintln!(
                        "Read {} bytes from stream",
                        chunk.as_ref().map_or(0, |c| c.len())
                    );
                    chunk
                }

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
    ) -> Result<FetchResponseOutcome, DownloadRunError> {
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
                        return Ok(FetchResponseOutcome::Paused);
                    }
                    SegmentCommand::Stop => {
                        let _ = self.sender.send(DownloadStatus {
                            total: total_size,
                            downloaded,
                            state: DownloadState::Stopped,
                        });
                        return Ok(FetchResponseOutcome::Stopped);
                    }
                    SegmentCommand::Nothing => {
                        return Err(DownloadRunError::NetworkError(
                            "Unexpected command received".to_owned(),
                        ));
                    }
                }
            }
        };
        res_result
            .map(FetchResponseOutcome::Response)
            .map_err(|e| DownloadRunError::NetworkError(e.to_string()))
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

        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.data.filepath)
            .await?;

        let mut session = DownloadSession {
            file,
            total_size,
            //TODO: Make this configurable in the future, but for now, 10s is a reasonable default.
            chunk_wait_timeout: Duration::from_secs(10),
            response: None,
        };

        let mut state = DownloadingState::Connect {
            downloaded: self.data.last_downloaded,
            attempts: 0,
        };

        loop {
            eprintln!(
                "Segment state: {:?}, downloaded: {}, attempts: {}",
                state,
                self.data.last_downloaded,
                match state {
                    DownloadingState::Connect { attempts, .. } => attempts,
                    DownloadingState::Download { attempts, .. } => attempts,
                    DownloadingState::Retry { attempts, .. } => attempts,
                    DownloadingState::Finish { .. } => 0,
                }
            );
            state = match state {
                DownloadingState::Connect {
                    downloaded,
                    attempts,
                } => {
                    let err_maybe = self
                        .handle_connect(&mut session, downloaded, attempts)
                        .await;
                    if err_maybe.is_err() {
                        eprintln!("Error during connect: {:?}", err_maybe);
                    }
                    err_maybe?
                }
                DownloadingState::Download {
                    downloaded,
                    attempts,
                } => {
                    self.handle_stream(&mut session, downloaded, attempts)
                        .await?
                }
                DownloadingState::Retry {
                    downloaded,
                    attempts,
                } => {
                    self.handle_backoff(&mut session, downloaded, attempts)
                        .await?
                }
                DownloadingState::Finish {
                    downloaded,
                    completed,
                } => {
                    session.file.flush().await?;
                    if completed {
                        let _ = self.sender.send(DownloadStatus {
                            downloaded,
                            total: Some(downloaded),
                            state: DownloadState::Completed { size: downloaded },
                        });
                    }
                    break;
                }
            };
        }

        Ok(())
    }

    /// State machine phase 1: seek, build the request, and perform the
    /// cancel-safe handshake. Retryable failures transition to `Retry`.
    async fn handle_connect(
        &self,
        session: &mut DownloadSession,
        downloaded: u64,
        attempts: u32,
    ) -> Result<DownloadingState, DownloadRunError> {
        let base_start = self.data.info.as_ref().map(|i| i.start).unwrap_or(0);
        session
            .file
            .seek(std::io::SeekFrom::Start(base_start + downloaded))
            .await?;

        let range_header = get_range_header(&self.data, downloaded);
        let mut req = self.client.get(&self.data.url);
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

        let cmd_rx = self.command_reciever.clone();
        let state = *cmd_rx.borrow();
        match state {
            SegmentCommand::Stop => {
                let _ = self.sender.send(DownloadStatus {
                    total: session.total_size,
                    downloaded,
                    state: DownloadState::Stopped,
                });
                return Ok(DownloadingState::Finish {
                    downloaded,
                    completed: false,
                });
            }
            SegmentCommand::Pause => {
                let _ = self.sender.send(DownloadStatus {
                    total: session.total_size,
                    downloaded,
                    state: DownloadState::Paused,
                });
                session.file.flush().await?;
                return Ok(DownloadingState::Finish {
                    downloaded,
                    completed: false,
                });
            }
            SegmentCommand::Nothing => {}
        }
        // TODO: Make this configurable
        let res_timed_out_maybe = tokio::time::timeout(
            Duration::from_secs(10),
            self.fetch_res_unless_interrupt(
                req,
                cmd_rx.clone(),
                session.total_size,
                downloaded,
                &mut session.file,
            ),
        )
        .await;
        eprintln!(
            "Fetch result has timed out: {:?}",
            res_timed_out_maybe.is_err()
        );
        if res_timed_out_maybe.is_err() {
            return Ok(DownloadingState::Retry {
                downloaded,
                attempts: attempts + 1,
            });
        }
        let res_maybe = res_timed_out_maybe.unwrap();
        let res = match res_maybe {
            Ok(FetchResponseOutcome::Response(res)) => {
                eprintln!("Fetch request succeeded with status: {:?}", res);
                res
            }
            Ok(FetchResponseOutcome::Paused | FetchResponseOutcome::Stopped) => {
                eprintln!("Fetch request interrupted by command");
                return Ok(DownloadingState::Finish {
                    downloaded,
                    completed: false,
                });
            }
            Err(_e) if attempts < self.retry_scheme.max_attempts => {
                eprintln!(
                    "Fetch request failed, retrying (attempt {}/{})",
                    attempts + 1,
                    self.retry_scheme.max_attempts
                );
                return Ok(DownloadingState::Retry {
                    downloaded,
                    attempts: attempts + 1,
                });
            }
            Err(e) => return Err(e),
        };

        let status = res.status();

        // Retryable transient server failures share the connect-phase retry
        // budget. This must run before the range check so that a transient
        // `503` on a ranged request is not misreported as a missing range
        // capability.
        if is_retryable_http_status(status) {
            if attempts < self.retry_scheme.max_attempts {
                return Ok(DownloadingState::Retry {
                    downloaded,
                    attempts: attempts + 1,
                });
            }
            return Err(DownloadRunError::NetworkError(format!(
                "Server returned HTTP {}",
                status
            )));
        }

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
        eprintln!("It didn't stop?");

        session.response = Some(res);
        Ok(DownloadingState::Download {
            downloaded,
            attempts,
        })
    }

    /// State machine phase 2: stream chunks from the active response, mapping
    /// completion, interruption, and retryable errors to the next state.
    async fn handle_stream(
        &self,
        session: &mut DownloadSession,
        downloaded: u64,
        given_attempts: u32,
    ) -> Result<DownloadingState, DownloadRunError> {
        // The watch receiver only notifies on changes made after `changed()` is
        // armed, so a pause/stop that arrived between handshake and stream must
        // be caught by reading the live value here.
        let mut attempts = given_attempts;
        let state = *self.command_reciever.borrow();
        match state {
            SegmentCommand::Pause => {
                let _ = self.sender.send(DownloadStatus {
                    total: session.total_size,
                    downloaded,
                    state: DownloadState::Paused,
                });
                session.file.flush().await?;
                return Ok(DownloadingState::Finish {
                    downloaded,
                    completed: false,
                });
            }
            SegmentCommand::Stop => {
                let _ = self.sender.send(DownloadStatus {
                    total: session.total_size,
                    downloaded,
                    state: DownloadState::Stopped,
                });
                return Ok(DownloadingState::Finish {
                    downloaded,
                    completed: false,
                });
            }
            SegmentCommand::Nothing => {}
        }

        let Some(mut res) = session.response.take() else {
            return Err(DownloadRunError::NetworkError(
                "No active response in Download state".to_owned(),
            ));
        };

        let mut current_downloaded = downloaded;
        let result = self
            .stream_chunks(
                &mut res,
                &mut session.file,
                &mut current_downloaded,
                &mut attempts,
                session.total_size,
                session.chunk_wait_timeout,
            )
            .await;

        match result {
            Ok(StreamChunkResult::Finished) => Ok(DownloadingState::Finish {
                downloaded: current_downloaded,
                completed: true,
            }),
            Ok(StreamChunkResult::Interrupted(cmd)) => {
                let state = match cmd {
                    SegmentCommand::Pause => DownloadState::Paused,
                    SegmentCommand::Stop => DownloadState::Stopped,
                    SegmentCommand::Nothing => unreachable!(),
                };
                let _ = self.sender.send(DownloadStatus {
                    total: session.total_size,
                    downloaded: current_downloaded,
                    state,
                });
                session.file.flush().await?;
                Ok(DownloadingState::Finish {
                    downloaded: current_downloaded,
                    completed: false,
                })
            }
            Err(DownloadRunError::NetworkError(_) | DownloadRunError::TimeoutError)
                if attempts < self.retry_scheme.max_attempts =>
            {
                Ok(DownloadingState::Retry {
                    downloaded: current_downloaded,
                    attempts: attempts + 1,
                })
            }
            Err(fatal_err) => Err(fatal_err),
        }
    }

    /// State machine phase 3: publish `Retrying`, flush, sleep for the backoff
    /// delay, then reconnect from the current offset.
    async fn handle_backoff(
        &self,
        session: &mut DownloadSession,
        downloaded: u64,
        attempts: u32,
    ) -> Result<DownloadingState, DownloadRunError> {
        let delay = self
            .retry_internal(attempts, session.total_size, downloaded)
            .await;
        session.file.flush().await?;
        tokio::time::sleep(delay).await;
        Ok(DownloadingState::Connect {
            downloaded,
            attempts,
        })
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

/// Statuses that indicate a transient server-side condition worth retrying.
/// Deliberately excludes client errors such as 400/401/403/404/410/416.
fn is_retryable_http_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT            // 408
            | StatusCode::TOO_EARLY            // 425
            | StatusCode::TOO_MANY_REQUESTS    // 429
            | StatusCode::INTERNAL_SERVER_ERROR // 500
            | StatusCode::BAD_GATEWAY          // 502
            | StatusCode::SERVICE_UNAVAILABLE  // 503
            | StatusCode::GATEWAY_TIMEOUT // 504
    )
}

/// Constructs the HTTP `Range` header value for resuming or segmented downloads.
fn get_range_header(data: &InPlaceSegmentData, downloaded: u64) -> Option<String> {
    match (&data.info, downloaded) {
        // Ranged segment: request from current offset to segment end
        (Some(i), d) => Some(format!("bytes={}-{}", i.start + d, i.end)),
        // Non-ranged segment being resumed: request from current offset to end
        (None, d) if d > 0 => Some(format!("bytes={}-", d)),
        // Fresh download starting at 0: no Range header needed
        _ => None,
    }
}

/// Calculates download speed in bytes per second.
fn calculate_speed(download_diff: u64, time_diff: Duration) -> f64 {
    let secs_diff = time_diff.as_secs_f64();
    (download_diff as f64) / secs_diff
}
