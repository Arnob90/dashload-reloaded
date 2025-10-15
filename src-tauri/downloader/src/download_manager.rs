use crate::idownloaditem::IDownloadData;
use crate::misc::FutureVec;
use crate::segment::DownloadStatusCheckError;
use crate::statuses;
use crate::{
    idownloaditem::{IDownloadItem, RichDownloadStatus},
    normal_download_item::NormalDownloadItem,
    segment,
    segmented_download_item::{DownloadItemCreationError, SegmentedDownloadItem},
};
use dashmap::DashMap;
use futures::future::join_all;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::time::Instant;
use uuid::Uuid;

pub struct DownloadManager {
    pub download_items: HashMap<String, Arc<dyn IDownloadItem>>,
    pub client: reqwest::Client,
    pub poll_every_ms: u64,
    pub status_history: DashMap<String, (statuses::DownloadStatus, Instant)>,
}
#[derive(Error, Debug, Serialize)]
pub enum SegmentedDownloadAddErrors {
    #[error("Cannot create segmented download")]
    SegmentDownloadCreationError(#[from] DownloadItemCreationError),
}

impl DownloadManager {
    pub fn new(poll_every_ms: u64) -> DownloadManager {
        Self {
            download_items: HashMap::new(),
            client: reqwest::Client::new(),
            poll_every_ms,
            status_history: DashMap::new(),
        }
    }
    pub async fn add_download_segmented(
        &mut self,
        url: String,
        filepath: String,
        number_of_segments: u64,
    ) -> Result<(), SegmentedDownloadAddErrors> {
        let required_download_item = Arc::new(
            SegmentedDownloadItem::new(
                self.client.clone(),
                &url,
                &filepath,
                number_of_segments,
                self.poll_every_ms,
            )
            .await?,
        );
        let required_item_clone = required_download_item.clone();
        tokio::spawn(async move {
            let _ = required_item_clone.start().await;
        });
        let uuid_for_download = Uuid::new_v4();
        let uuid_str = uuid_for_download.as_simple().to_string();
        self.download_items.insert(uuid_str, required_download_item);
        Ok(())
    }
    pub async fn add_download_normal(&mut self, url: String, filepath: String) {
        let required_download_item = Arc::new(
            NormalDownloadItem::new(
                url,
                filepath.into(),
                self.client.clone(),
                self.poll_every_ms,
            )
            .await,
        );
        let required_item_clone = required_download_item.clone();
        tokio::spawn(async move {
            let _ = required_item_clone.start().await;
        });
        let uuid = Uuid::new_v4().as_simple().to_string();
        self.download_items.insert(uuid, required_download_item);
    }
    // Renamed to be more explicit about what it does.
    pub async fn get_download_data_from_item(
        &self,
        item_id: &str,
    ) -> Result<IDownloadData, DownloadStatusCheckError> {
        // --- Phase 1: Fetch all data concurrently (no locks) ---
        // This is the long-running part.
        let item = self.download_items.get(item_id).unwrap();

        let (filepath, filename, url, current_status) = tokio::join!(
            item.filepath(),
            item.filename(),
            async { item.get_url() },
            async { item.get_status() } // Wrap sync function in async block for join
        );
        let current_status = current_status?; // Propagate error from get_status

        // --- Phase 2: Calculate speed and update history atomically ---
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
                let bytes_diff = (*current_bytes - *prev_bytes) as f64;
                if bytes_diff >= 0.0 {
                    speed_mbps = (bytes_diff / 1024.0 / 1024.0) / time_elapsed;
                }
            }
        }

        // Update the history entry with the current values.
        *history_entry = (current_status.clone(), now);

        // The DashMap lock is released when `history_entry` goes out of scope here.

        // --- Phase 3: Translate to the rich DTO
        let rich_status = match current_status {
            statuses::DownloadStatus::Running { downloaded, total } => {
                RichDownloadStatus::Running {
                    downloaded,
                    total,
                    download_speed: speed_mbps,
                    downloaded_portion: total.map(|total| downloaded as f64 / total as f64),
                    eta_secs: total.and_then(|total| {
                        let diff = ((total - downloaded) as f64) / 1024.0 / 1024.0;
                        if speed_mbps <= 0.0 {
                            return None;
                        }
                        Some(diff / speed_mbps)
                    }),
                }
            }
            statuses::DownloadStatus::Completed => RichDownloadStatus::Completed,
            statuses::DownloadStatus::Failed(e) => RichDownloadStatus::Failed(e),
            statuses::DownloadStatus::Pending => RichDownloadStatus::Pending,
            statuses::DownloadStatus::Stopped => RichDownloadStatus::Stopped,
        };

        Ok(IDownloadData {
            status: rich_status,
            filename,
            filepath,
            url,
        })
    }
    pub async fn get_all_items(
        &self,
    ) -> Result<HashMap<String, IDownloadData>, segment::DownloadStatusCheckError> {
        //A side effect of this design is that if the frontend has a large delay in calling this,
        //then the download speed will be more averaged
        let mut download_datas_map = HashMap::<String, IDownloadData>::new();
        let mut download_data_futures: FutureVec<
            Result<IDownloadData, segment::DownloadStatusCheckError>,
        > = FutureVec::new();
        let mut id_list: Vec<&str> = vec![];
        for (id, _) in self.download_items.iter() {
            let download_data = self.get_download_data_from_item(id);
            download_data_futures.push(Box::pin(download_data));
            id_list.push(id);
        }
        let download_datas = join_all(download_data_futures).await;
        for i in 0..download_datas.len() {
            let download_data_maybe = download_datas[i].as_ref();
            if let Err(e) = download_data_maybe {
                return Err(e.clone());
            }
            let download_data = download_data_maybe.unwrap();
            download_datas_map.insert(id_list[i].to_string(), download_data.clone());
        }
        Ok(download_datas_map)
    }
}
