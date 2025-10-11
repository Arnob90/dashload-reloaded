use std::time::Duration;
use std::{collections::HashMap};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use crate::length_getter::LengthGettingErrors;
use crate::{
    idownloaditem::IDownloadItem,
    segment,
    segmented_download_item::{DownloadItemCreationError, SegmentedDownloadItem},
    normal_download_item::NormalDownloadItem,
    statuses::{self, DownloadStatus},
};

pub struct DownloadManager {
    pub download_items: HashMap<String, Arc<dyn IDownloadItem>>,
    pub client: reqwest::Client,
    pub poll_every_ms: u64
}
#[derive(Error, Debug)]
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
    pub async fn add_download_normal(&mut self, url:String, filepath:String) -> Result<(),LengthGettingErrors> {
        let required_download_item = Arc::new(NormalDownloadItem::new(url, filepath.into(), self.client.clone(), self.poll_every_ms).await?);
        let required_item_clone = required_download_item.clone();
        tokio::spawn(async move {
            let _ = required_item_clone.start().await;
        });
        let uuid = Uuid::new_v4().as_simple().to_string();
        self.download_items.insert(uuid, required_download_item);
        Ok(())
    }
    pub fn get_all_status(
        &self,
    ) -> Result<HashMap<String, statuses::DownloadStatus>, segment::DownloadStatusCheckError> {
        let statuses_map = self
            .download_items
            .iter()
            .map(|(id, download)| (id, download.get_status()));
        let mut required_map: HashMap<String, DownloadStatus> = HashMap::new();
        for (id, status_maybe) in statuses_map {
            required_map.insert(id.clone(), status_maybe?);
        }
        Ok(required_map)
    }
}
