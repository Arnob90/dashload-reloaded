use crate::download_data::DownloadData;
use crate::download_run_error::DownloadRunError;
use crate::download_status::{DownloadState, DownloadStatus};
use crate::in_place_segment::{InPlaceSegment, InPlaceSegmentData, SegmentCommand, SegmentInfo};
use futures::future::join_all;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio::sync::watch;

pub enum CreateOpts {
    Segmented { num_segments: u64, filesize: u64 },
    Normal,
}
#[derive(Error, Debug)]
enum DownloadItemCreationError {
    #[error("Given download data is invalid")]
    InvalidData,
}
pub struct DownloadItem {
    pub segments: Vec<InPlaceSegment>,
    pub status_receivers: Vec<tokio::sync::watch::Receiver<DownloadStatus>>,
    pub filepath: PathBuf,
    pub url: String,
    pub polling_rate_ms: u64,
    pub segment_command_sender: watch::Sender<SegmentCommand>,
}

impl DownloadItem {
    /// Helper to divide file into segments pointing to the SAME target file
    fn divide_into_segments(
        length: u64,
        num_segments: u64,
        url: &str,
        filepath: &Path,
    ) -> Vec<InPlaceSegmentData> {
        let num_segments = num_segments.max(1); // Prevent division by zero
        let chunk_size = length / num_segments;
        let mut segments = Vec::new();

        for i in 0..num_segments {
            let start = i * chunk_size;
            let end = if i == num_segments - 1 {
                length - 1
            } else {
                (i + 1) * chunk_size - 1
            };

            segments.push(InPlaceSegmentData {
                url: url.to_string(),
                filepath: filepath.to_path_buf(),
                last_downloaded: 0,
                info: Some(SegmentInfo { start, end }),
            });
        }
        segments
    }
    pub async fn create_from_data(
        client: reqwest::Client,
        data: DownloadData,
        polling_rate_ms: u64,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        // 1. Pre-allocate target file on disk if filesize is known
        let is_fresh = data.segments.iter().map(|s| s.last_downloaded).sum::<u64>() == 0u64;
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(is_fresh)
            .open(&data.filepath)
            .await?;
        if let Some(filesize) = data.length {
            file.set_len(filesize).await?;
        }

        // 2. Generate segment data definitions
        let segment_datas = data.segments;
        // 3. Initialize InPlaceSegments and collect their receivers
        let mut segments = Vec::new();
        let mut status_receivers = Vec::new();
        let (segment_commander, reciever) = watch::channel(SegmentCommand::Nothing);
        for data in segment_datas {
            let req_reciever = reciever.clone();
            let (segment, rx) =
                InPlaceSegment::new(client.clone(), data, polling_rate_ms, req_reciever).await?;
            segments.push(segment);
            status_receivers.push(rx);
        }

        let item = Arc::new(Self {
            segments,
            status_receivers,
            filepath: data.filepath,
            url: data.url,
            polling_rate_ms,
            segment_command_sender: segment_commander,
        });
        Ok(item)
    }
    pub async fn start(&self) -> Result<(), DownloadRunError> {
        let mut futures = Vec::new();
        for segment in &self.segments {
            futures.push(segment.start());
        }

        // Run all segment workers concurrently
        let results = join_all(futures).await;
        // Delete the file for a stop command
        if *self.segment_command_sender.borrow() == SegmentCommand::Stop {
            //TODO: Warn user if file deletion fails
            let _ = tokio::fs::remove_file(&self.filepath).await;
        }

        // Propagate the first error encountered, if any
        for result in results {
            result?;
        }

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

            match &status.status {
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
            }
        }

        let overall_state = if let Some(err) = any_failed {
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

        DownloadStatus {
            total: total_filesize,
            downloaded: total_downloaded,
            status: overall_state,
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
