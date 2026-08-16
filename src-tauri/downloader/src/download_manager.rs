use crate::download_data::DownloadData;
use crate::download_data_factory::DownloadDataFactory;
use crate::download_item::DownloadItem;
use crate::download_status::DownloadState;
use crate::download_status::DownloadStatus;
use crate::info_getters;
use crate::serialization_error::DataSerializationError;
use dashmap::DashMap;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, Error, Clone, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum ManagerError {
    #[error("Given ID does not exist")]
    IdDoesNotExist,
    #[error("Failed to create or start download: {0}")]
    DownloadCreationError(String),
    #[error("Download operation failed: {0}")]
    OperationFailed(String),
    #[error("Failed to add download: No filesize")]
    NoFilesizeError,
}

pub struct DownloadManager {
    pub download_items: DashMap<String, Arc<DownloadItem>>,
    pub client: reqwest::Client,
    pub poll_every_ms: u64,
}

impl DownloadManager {
    pub fn new(client: reqwest::Client, poll_every_ms: u64) -> Self {
        Self {
            download_items: DashMap::new(),
            poll_every_ms,
            client,
        }
    }

    pub async fn reconstruct(
        client: reqwest::Client,
        poll_every_ms: u64,
        datas: HashMap<String, DownloadData>,
    ) -> Result<DownloadManager, ManagerError> {
        let manager = DownloadManager::new(client.clone(), poll_every_ms);
        for (id, data) in datas {
            let item = DownloadItem::create_from_data(client.clone(), data, poll_every_ms)
                .await
                .map_err(|e| ManagerError::DownloadCreationError(e.to_string()))?;
            manager.download_items.insert(id, item.clone());
            tokio::spawn(async move {
                let _ = item.start().await;
            });
        }
        Ok(manager)
    }

    pub async fn add_normal_download(
        &self,
        url: String,
        filepath: PathBuf,
    ) -> Result<String, ManagerError> {
        let data = DownloadDataFactory::new_normal(url, filepath);
        let item = DownloadItem::create_from_data(self.client.clone(), data, self.poll_every_ms)
            .await
            .map_err(|e| ManagerError::DownloadCreationError(e.to_string()))?;
        let id = Uuid::new_v4().as_simple().to_string();
        self.download_items.insert(id.clone(), item);
        Ok(id)
    }

    /// Adds a new download (handles both 1 segment and multi-segment)
    pub async fn add_download(
        &self,
        url: String,
        filepath: PathBuf,
        number_of_segments: u64,
    ) -> Result<String, ManagerError> {
        // 1. Fetch filesize. Fallback to `new_normal` (N=1) if unknown!
        let data = match info_getters::get_filesize(self.client.clone(), &url).await {
            Some(filesize) => {
                DownloadDataFactory::new_segmented(url, filepath, filesize, number_of_segments)
            }
            None => DownloadDataFactory::new_normal(url, filepath),
        };

        // 2. Construct the DownloadItem
        let item = DownloadItem::create_from_data(self.client.clone(), data, self.poll_every_ms)
            .await
            .map_err(|e| ManagerError::DownloadCreationError(e.to_string()))?;

        let id = Uuid::new_v4().as_simple().to_string();
        self.download_items.insert(id.clone(), item.clone());

        // 3. Spawn background runner task (.await added!)
        let item_clone = item.clone();
        tokio::spawn(async move {
            let _ = item_clone.start().await;
        });

        Ok(id)
    }

    /// Resumes a paused download from its saved state
    pub async fn resume(&self, id: &str) -> Result<(), ManagerError> {
        let mut item_data = {
            self.download_items
                .get(id)
                .ok_or(ManagerError::IdDoesNotExist)?
                .snapshot_data()
        };
        item_data.state = DownloadState::Running {
            download_speed: 0.0,
        };
        let new_item =
            DownloadItem::create_from_data(self.client.clone(), item_data, self.poll_every_ms)
                .await
                .map_err(|e| ManagerError::OperationFailed(e.to_string()))?;
        let item_clone = new_item.clone();
        tokio::spawn(async move {
            let _ = item_clone.start().await;
        });
        self.download_items.insert(id.to_string(), new_item);
        Ok(())
    }

    /// Pauses an active download
    pub fn pause(&self, id: &str) -> Result<(), ManagerError> {
        let item = self
            .download_items
            .get(id)
            .ok_or(ManagerError::IdDoesNotExist)?;

        item.pause();
        Ok(())
    }

    /// Gets status for a single download item (instantaneous, no async needed)
    pub fn get_item_status(&self, id: &str) -> Result<DownloadStatus, ManagerError> {
        let item = self
            .download_items
            .get(id)
            .ok_or(ManagerError::IdDoesNotExist)?;

        Ok(item.get_status())
    }

    /// Gets status for ALL download items (instantaneous, no async needed)
    pub fn get_all_items(&self) -> HashMap<String, DownloadStatus> {
        let mut status_map = HashMap::new();
        for entry in self.download_items.iter() {
            let id = entry.key().clone();
            let item = entry.value();
            status_map.insert(id, item.get_status());
        }
        status_map
    }
    pub fn get_full_snapshot(&self) -> HashMap<String, DownloadData> {
        let mut req = HashMap::new();
        for entry in self.download_items.iter() {
            let id = entry.key();
            let data = entry.value().snapshot_data();
            req.insert(id.clone(), data);
        }
        req
    }
    pub async fn save_session(&self, to_save_to: &Path) -> Result<(), DataSerializationError> {
        let snapshot = self.get_full_snapshot();
        let json_str = serde_json::to_string(&snapshot)?;
        fs::write(&to_save_to, json_str).await?;
        Ok(())
    }
}
