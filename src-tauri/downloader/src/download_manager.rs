use crate::downloaditem::{DownloadHandle, DownloadItem};
use crate::idownloaditem::IDownloadData;
use crate::misc::FutureVec;
use crate::resume_download_factory::{DownloadResumeFactory, ResumeFactoryErr};
use crate::segment::DownloadStatusCheckError;
use crate::statuses;
use crate::{
    idownloaditem::RichDownloadStatus,
    segmented_download_item::{SegmentedDownloadItemCreationError, SegmentedRunningDownloadItem},
};
use dashmap::DashMap;
use futures::future::join_all;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::time::Instant;
use uuid::Uuid;
#[derive(Debug, Error, Clone, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadStatusGetError {
    #[error("Failed to check status")]
    StatusCheckError(#[from] DownloadStatusCheckError),
    #[error("Given id does not exist")]
    IdDoesNotExist,
}

#[derive(Debug, Error, Clone, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum ResumeError {
    #[error("Given id does not exist")]
    IdDoesNotExist,
    #[error("Download is already running!")]
    NotPausedError,
    #[error("Error creating download item")]
    DownloadItemCreationError(ResumeFactoryErr),
    #[error("Something went wrong!")]
    MiscError,
}

#[derive(Debug, Clone, Error, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum PauseError {
    #[error("Given id does not exist")]
    IdDoesNotExist,
    #[error("Download is not running")]
    NotRunningError,
}
pub struct DownloadManager {
    pub download_items: DashMap<String, Arc<DownloadItem>>,
    pub client: reqwest::Client,
    pub poll_every_ms: u64,
    pub status_history: DashMap<String, (statuses::DownloadStatus, Instant)>,
}
impl DownloadManager {
    pub fn new(client: reqwest::Client, poll_every_ms: u64) -> DownloadManager {
        Self {
            download_items: DashMap::new(),
            poll_every_ms,
            client,
            status_history: DashMap::new(),
        }
    }
    pub async fn add_download_segmented(
        &self,
        url: String,
        filepath: PathBuf,
        number_of_segments: u64,
    ) -> Result<(), SegmentedDownloadItemCreationError> {
        let required_download_item = DownloadItem::new(DownloadHandle::Running(
            SegmentedRunningDownloadItem::new(
                self.client.clone(),
                &url,
                filepath,
                number_of_segments,
                self.poll_every_ms,
            )
            .await?,
        ))
        .await;
        let uuid = Uuid::new_v4().as_simple().to_string();
        self.download_items.insert(uuid, required_download_item);
        Ok(())
    }
    // Renamed to be more explicit about what it does.
    // We usually catch this in batch, so we might as well own the string
    pub async fn get_download_data_from_item(
        &self,
        item_id: String,
    ) -> Result<IDownloadData, DownloadStatusGetError> {
        let item = self
            .download_items
            .get(&item_id)
            .ok_or(DownloadStatusGetError::IdDoesNotExist)?;
        let item_handle_lock = item.handle.read().await;
        let item_handle = &*item_handle_lock;
        if let DownloadHandle::Terminal(item) = item_handle {
            let data = &item.data;
            let status = &item.status;
            let rich_status = match status {
                statuses::DownloadStatus::Running { .. } => {
                    unreachable!("A terminal item is never running!")
                }
                statuses::DownloadStatus::Completed { total } => {
                    RichDownloadStatus::Completed { total: *total }
                }
                statuses::DownloadStatus::Failed(e) => RichDownloadStatus::Failed(e.clone()),
                statuses::DownloadStatus::Stopped => RichDownloadStatus::Stopped,
                statuses::DownloadStatus::Paused => RichDownloadStatus::Paused,
            };
            let filename = Path::new(&data.filepath)
                .file_name()
                .unwrap()
                .to_string_lossy();
            Ok(IDownloadData {
                status: rich_status,
                filename: filename.into(),
                filepath: data.filepath.clone(),
                url: data.url.clone(),
            })
        } else if let DownloadHandle::Running(item) = item_handle {
            let filename = item.filename();
            let filepath = item.filepath();
            let url = item.get_url();
            let current_status = item.get_status()?;
            // --- Phase 1: Calculate speed and update history atomically ---
            let now = Instant::now();
            let mut speed_mbps = 0.0; // Default to 0

            // Get a mutable handle to the history entry for this ID.
            // This is an atomic operation that locks only one shard of the DashMap.
            let mut history_entry = self
                .status_history
                .entry(item_id.to_string())
                .or_insert_with(|| (current_status.clone(), now)); // Insert if it doesn't exist

            // Now we have the previous state, `history_entry` is a `RefMut` to `(status, time)`
            let (prev_status, prev_time) = &*history_entry;

            // Calculate speed based on the previous state.
            if let (
                statuses::DownloadStatus::Running {
                    downloaded: current_bytes,
                    ..
                },
                statuses::DownloadStatus::Running {
                    downloaded: prev_bytes,
                    ..
                },
            ) = (&current_status, prev_status)
            {
                let time_elapsed = now.duration_since(*prev_time).as_secs_f64();
                if time_elapsed > 0.0 {
                    let bytes_diff = ((*current_bytes as i64) - (*prev_bytes as i64)) as f64;
                    if bytes_diff >= 0.0 {
                        speed_mbps = (bytes_diff / 1024.0 / 1024.0) / time_elapsed;
                    } else {
                        speed_mbps = 0.0;
                    }
                }
            }

            // Update the history entry with the current values.
            *history_entry = (current_status.clone(), now);

            // The DashMap lock is released when `history_entry` goes out of scope here.

            // --- Phase 2: Translate to the rich DTO
            let rich_status = match current_status {
                statuses::DownloadStatus::Running { downloaded, total } => {
                    RichDownloadStatus::Running {
                        downloaded,
                        total,
                        download_speed: speed_mbps,
                        downloaded_portion: total.map(|total| downloaded as f64 / total as f64),
                        eta_secs: total.and_then(|total| {
                            let diff_bytes = total.saturating_sub(downloaded);
                            let diff = diff_bytes as f64 / 1024.0 / 1024.0;
                            if speed_mbps <= 0.0 {
                                return None;
                            }
                            Some(diff / speed_mbps)
                        }),
                    }
                }
                statuses::DownloadStatus::Completed { total } => {
                    RichDownloadStatus::Completed { total }
                }
                statuses::DownloadStatus::Failed(e) => RichDownloadStatus::Failed(e),
                statuses::DownloadStatus::Stopped => RichDownloadStatus::Stopped,
                statuses::DownloadStatus::Paused => RichDownloadStatus::Paused,
            };

            Ok(IDownloadData {
                status: rich_status,
                filename,
                filepath,
                url,
            })
        } else {
            unreachable!("A download is either terminal or running")
        }
    }
    pub async fn get_all_items(
        &self,
    ) -> Result<HashMap<String, IDownloadData>, DownloadStatusGetError> {
        //A side effect of this design is that if the frontend has a large delay in calling this,
        //then the download speed will be more averaged
        let mut download_datas_map = HashMap::<String, IDownloadData>::new();
        let mut download_data_futures: FutureVec<Result<IDownloadData, DownloadStatusGetError>> =
            FutureVec::new();
        let mut id_list: Vec<String> = vec![];
        for entry in self.download_items.iter() {
            let id = entry.key().clone();
            let download_data = self.get_download_data_from_item(id.clone());
            download_data_futures.push(Box::pin(download_data));
            id_list.push(id);
        }
        let _download_datas = join_all(download_data_futures).await;
        for (i, download_data_maybe) in _download_datas.into_iter().enumerate() {
            let download_data = download_data_maybe?;
            download_datas_map.insert(id_list[i].to_string(), download_data.clone());
        }
        Ok(download_datas_map)
    }
    pub async fn resume(&self, item_id: &str) -> Result<(), ResumeError> {
        let to_resume = self.get_download_data_from_item(item_id.to_string()).await;
        if let Err(DownloadStatusGetError::IdDoesNotExist) = to_resume {
            return Err(ResumeError::IdDoesNotExist);
        }
        let to_resume_data = to_resume.map_err(|_| ResumeError::MiscError)?;
        let resume_status = to_resume_data.status;
        if resume_status == RichDownloadStatus::Paused {
            let filepath_to_resume = Path::new(&to_resume_data.filepath);
            let handle = DownloadResumeFactory::resume_from_filepath(
                self.client.clone(),
                filepath_to_resume,
                self.poll_every_ms,
            )
            .await
            .map_err(ResumeError::DownloadItemCreationError)?;
            self.status_history.remove(item_id);
            self.download_items.insert(
                item_id.to_string(),
                DownloadItem::new(DownloadHandle::Running(handle)).await,
            );
            Ok(())
        } else {
            Err(ResumeError::NotPausedError)
        }
    }
    pub async fn pause(&self, item_id: &str) -> Result<(), PauseError> {
        let to_pause = self
            .download_items
            .get(item_id)
            .ok_or(PauseError::IdDoesNotExist)?;
        let handle_read_lock = to_pause.handle.read().await;
        let handle = &*handle_read_lock;
        if let DownloadHandle::Running(item) = handle {
            item.pause();
            Ok(())
        } else {
            Err(PauseError::NotRunningError)
        }
    }
}
