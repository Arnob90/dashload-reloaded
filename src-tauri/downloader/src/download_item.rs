use crate::download_data::DownloadData;
use crate::download_run_error::DownloadRunError;
use crate::download_status::{DownloadState, DownloadStatus};
use crate::in_place_segment::{InPlaceSegment, SegmentCommand};
use crate::verifier::VerifierMaybe;
use futures::future::try_join_all;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::watch;

pub enum CreateOpts {
    Segmented { num_segments: u64, filesize: u64 },
    Normal,
}

pub enum VerificationState {
    Verifying,
    Verified,
    VerifyFailed,
}

pub struct DownloadItem {
    pub segments: Vec<InPlaceSegment>,
    pub status_receivers: Vec<tokio::sync::watch::Receiver<DownloadStatus>>,
    pub filepath: PathBuf,
    pub url: String,
    pub polling_rate_ms: u64,
    pub segment_command_sender: watch::Sender<SegmentCommand>,
    pub verifier: VerifierMaybe,
    pub verify_result_receiver: watch::Receiver<VerificationState>,
    pub verify_result_sender: watch::Sender<VerificationState>,
}

impl DownloadItem {
    pub async fn create_from_data(
        client: reqwest::Client,
        mut download_data: DownloadData,
        polling_rate_ms: u64,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        // 1. Pre-allocate target file on disk if filesize is known
        let file_exists = tokio::fs::metadata(&download_data.filepath).await.is_ok();
        let has_progress = download_data.segments.iter().any(|s| s.last_downloaded > 0);
        let is_fresh = !file_exists || !has_progress;
        if is_fresh {
            for segment in download_data.segments.iter_mut() {
                segment.last_downloaded = 0;
            }
        }
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(is_fresh)
            .open(&download_data.filepath)
            .await?;
        if let Some(filesize) = download_data.length
            && is_fresh
        {
            file.set_len(filesize).await?;
        }

        // 2. Generate segment data definitions
        let segment_datas = download_data.segments;
        // 3. Initialize InPlaceSegments and collect their receivers
        let mut segments = Vec::new();
        let mut status_receivers = Vec::new();
        let (segment_commander, reciever) = watch::channel(SegmentCommand::Nothing);
        for data in segment_datas {
            let req_reciever = reciever.clone();
            let (segment, rx) = InPlaceSegment::new(
                client.clone(),
                data,
                polling_rate_ms,
                req_reciever,
                download_data.state.clone(),
            )
            .await?;
            segments.push(segment);
            status_receivers.push(rx);
        }
        let (sender, receiver) = watch::channel(VerificationState::Verifying);

        let item = Arc::new(Self {
            segments,
            status_receivers,
            filepath: download_data.filepath,
            url: download_data.url,
            polling_rate_ms,
            segment_command_sender: segment_commander,
            verifier: download_data.verifier.clone(),
            verify_result_receiver: receiver,
            verify_result_sender: sender,
        });
        Ok(item)
    }
    pub async fn start(&self) -> Result<(), DownloadRunError> {
        let mut futures = Vec::new();
        for segment in &self.segments {
            futures.push(segment.start());
        }

        // Run all segment workers concurrently
        let results = try_join_all(futures).await;
        // Delete the file for a stop command
        if *self.segment_command_sender.borrow() == SegmentCommand::Stop {
            //TODO: Warn user if file deletion fails
            let _ = tokio::fs::remove_file(&self.filepath).await;
        }
        results?;
        let verify_clone = self.verifier.clone();
        let filepath_clone = self.filepath.clone();
        let verified = tokio::task::spawn_blocking(move || verify_clone.verify(&filepath_clone))
            .await
            .map_err(|e| DownloadRunError::DiskError(format!("Verification task panicked {}", e)))?
            .map_err(|e| {
                DownloadRunError::DiskError(format!("IO error during validation {}", e))
            })?;
        if !verified {
            let _ = self
                .verify_result_sender
                .send(VerificationState::VerifyFailed);
            return Err(DownloadRunError::VerifyError);
        }
        let _ = self.verify_result_sender.send(VerificationState::Verified);
        Ok(())
    }

    /// Computes aggregated status across all active segment receivers (Instantaneous)
    pub fn get_status(&self) -> DownloadStatus {
        let mut total_downloaded: u64 = 0;
        let mut total_speed: f64 = 0.0;
        let mut total_filesize: Option<u64> = Some(0);
        let mut all_completed = true;
        let mut any_failed = None;
        let mut any_paused = false;
        let mut any_stopped = false;

        for rx in &self.status_receivers {
            let status = rx.borrow();
            total_downloaded += status.downloaded;

            if let Some(t) = status.total {
                total_filesize = total_filesize.map(|acc| acc + t);
            } else {
                total_filesize = None;
            }

            match &status.state {
                DownloadState::Running { download_speed } => {
                    total_speed += download_speed;
                    all_completed = false;
                }
                DownloadState::Completed { .. } => {}
                DownloadState::Failed(err) => {
                    any_failed = Some(err.clone());
                    all_completed = false;
                }
                DownloadState::Paused => {
                    any_paused = true;
                    all_completed = false;
                }
                DownloadState::Stopped => {
                    all_completed = false;
                    any_stopped = true
                }
                DownloadState::Verifying => {}
            }
        }

        let mut overall_state = if let Some(err) = any_failed {
            DownloadState::Failed(err)
        } else if all_completed {
            DownloadState::Completed {
                size: total_downloaded,
            }
        } else if any_paused {
            DownloadState::Paused
        } else if any_stopped {
            DownloadState::Stopped
        } else {
            DownloadState::Running {
                download_speed: total_speed,
            }
        };
        if let DownloadState::Completed { .. } = overall_state {
            overall_state = match *self.verify_result_receiver.borrow() {
                VerificationState::Verifying => DownloadState::Verifying,
                VerificationState::VerifyFailed => {
                    DownloadState::Failed(DownloadRunError::VerifyError)
                }
                VerificationState::Verified => overall_state,
            }
        }

        DownloadStatus {
            total: total_filesize,
            downloaded: total_downloaded,
            state: overall_state,
        }
    }

    /// Generates up-to-date DownloadData payload for JSON serialization
    pub fn snapshot_data(&self) -> DownloadData {
        DownloadData {
            url: self.url.clone(),
            filepath: self.filepath.clone(),
            length: self.get_total_filesize(),
            segments: self
                .segments
                .iter()
                .map(|segment| segment.snapshot())
                .collect(),
            state: self.get_status().state,
            verifier: self.verifier.clone(),
        }
    }

    /// Helper to get total filesize across segments
    fn get_total_filesize(&self) -> Option<u64> {
        let mut total_size = 0;
        for rx in &self.status_receivers {
            let status = rx.borrow();
            total_size += status.total?;
        }
        Some(total_size)
    }
    pub fn pause(&self) {
        let _ = self.segment_command_sender.send(SegmentCommand::Pause);
    }
    pub async fn stop(&self) {
        let _ = self.segment_command_sender.send(SegmentCommand::Stop);
    }
}
