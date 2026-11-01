use crate::download_data::DownloadData;
use crate::download_data_factory::DownloadDataFactory;
use crate::download_item::DownloadItem;
use crate::download_status::DownloadState;
use crate::download_status::DownloadStatus;
use crate::info_getters;
use crate::request_headers::RequestHeaders;
use crate::retry_scheme::RetryScheme;
use crate::segmented_verifier::SegmentVerificationError;
use crate::segmented_verifier::verify_segmented;
use crate::serialization_error::DataSerializationError;
use crate::verifier::VerifierMaybe;
use dashmap::DashMap;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::fs;
use uuid::Uuid;

/// Errors that can occur when interacting with the [`DownloadManager`].
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
    #[error("Invalid headers provided: {0}")]
    InvalidHeaders(String),
    #[error("Segmented download not supported by server. Reason: {0}")]
    SegmentedNotSupported(String),
}

/// The core manager for handling multiple concurrent downloads, supporting pause,
/// resume, status polling, verification, and state serialization.
pub struct DownloadManager {
    /// Thread-safe map storing active download items keyed by their unique string IDs.
    pub download_items: DashMap<String, Arc<DownloadItem>>,
    /// Shared HTTP client (`reqwest::Client`) used for making download requests.
    pub client: reqwest::Client,
    /// Polling interval in milliseconds for status updates and progress tracking.
    pub poll_every_ms: u64,
}

impl DownloadManager {
    /// Builds a Reqwest client with production-ready timeouts and defaults
    pub fn default_client() -> reqwest::Client {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10)) //  Prevents hanging on dead servers
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build default reqwest client")
    }
    /// Creates a new, empty `DownloadManager`.
    ///
    /// # Arguments
    /// * `client` - The `reqwest::Client` instance to use for network operations.
    /// * `poll_every_ms` - How often in milliseconds progress and status are updated/polled.
    pub fn new(client: reqwest::Client, poll_every_ms: u64) -> Self {
        Self {
            download_items: DashMap::new(),
            poll_every_ms,
            client,
        }
    }

    /// Reconstructs a `DownloadManager` from a previously saved session snapshot of download data.
    ///
    /// This recreates all download items and spawns background tasks to resume them.
    pub async fn reconstruct(
        client: reqwest::Client,
        poll_every_ms: u64,
        datas: HashMap<String, DownloadData>,
    ) -> Result<DownloadManager, ManagerError> {
        let manager = DownloadManager::new(client.clone(), poll_every_ms);
        for (id, mut data) in datas.into_iter() {
            data.state = match data.state {
                DownloadState::Retrying { .. } => DownloadState::Paused,
                other => other,
            };
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

    async fn get_download_data(
        &self,
        url: String,
        filepath: PathBuf,
        number_of_segments: u64,
        verifier: VerifierMaybe,
        request_headers: RequestHeaders,
        retry_scheme: RetryScheme,
    ) -> Result<DownloadData, ManagerError> {
        if number_of_segments == 1 {
            return Ok(DownloadDataFactory::new_normal(
                url,
                filepath,
                verifier,
                request_headers,
                retry_scheme,
            ));
        }
        let header_map = request_headers
            .to_header_map()
            .map_err(|e| ManagerError::InvalidHeaders(e.to_string()))?;
        let filesize_maybe =
            info_getters::get_filesize(self.client.clone(), &url, header_map.clone())
                .await
                .ok_or(SegmentVerificationError::NoFilesize);
        let mut segmented_possible = Err(SegmentVerificationError::NoFilesize);
        if let Ok(filesize) = filesize_maybe {
            segmented_possible = verify_segmented(
                self.client.clone(),
                &url,
                0,
                filesize.saturating_sub(1),
                header_map.clone(),
            )
            .await;
        }
        if segmented_possible.is_err() && number_of_segments > 1 {
            return Err(ManagerError::SegmentedNotSupported(
                segmented_possible.err().unwrap().to_string(),
            ));
        }
        Ok(DownloadDataFactory::new_segmented(
            url,
            filepath,
            filesize_maybe.unwrap(),
            number_of_segments,
            verifier,
            header_map,
            retry_scheme,
        ))
    }
    /// Adds a new download (handles both single-segment and multi-segment downloads).
    ///
    /// Fetches the file size first to determine if multi-segment chunking can be used,
    /// initializes the download item, generates a unique ID, and starts it in a background task.
    ///
    /// # Arguments
    /// * `url` - The target download URL.
    /// * `filepath` - The local destination file path.
    /// * `number_of_segments` - The number of concurrent chunks/segments to split the download into.
    /// * `verifier` - Optional checksum verifier (`VerifierMaybe`).
    /// * `request_headers` - Custom HTTP headers (`RequestHeaders`) such as User-Agent or cookies.
    /// # Errors
    /// - ManagerError::SegmentedNotSupported if the server does not support segmented downloads.
    ///
    /// Providing a fallback is user responsibility (e.g., retrying with a single segment).
    pub async fn add_download(
        &self,
        url: String,
        filepath: PathBuf,
        number_of_segments: u64,
        verifier: VerifierMaybe,
        request_headers: RequestHeaders,
        retry_scheme: RetryScheme,
    ) -> Result<String, ManagerError> {
        let data = self
            .get_download_data(
                url,
                filepath,
                number_of_segments,
                verifier,
                request_headers,
                retry_scheme,
            )
            .await?;
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

    /// Resumes a paused download from its saved state.
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

    /// Pauses an active download by its unique ID.
    pub fn pause(&self, id: &str) -> Result<(), ManagerError> {
        let item = self
            .download_items
            .get(id)
            .ok_or(ManagerError::IdDoesNotExist)?;

        item.pause();
        Ok(())
    }

    /// Gets instantaneous status for a single download item (no async needed).
    pub fn get_item_status(&self, id: &str) -> Result<DownloadStatus, ManagerError> {
        let item = self
            .download_items
            .get(id)
            .ok_or(ManagerError::IdDoesNotExist)?;

        Ok(item.get_status())
    }

    /// Gets instantaneous status for all active download items (no async needed).
    pub fn get_all_items(&self) -> HashMap<String, DownloadStatus> {
        let mut status_map = HashMap::new();
        for entry in self.download_items.iter() {
            let id = entry.key().clone();
            let item = entry.value();
            status_map.insert(id, item.get_status());
        }
        status_map
    }

    /// Retrieves a full data snapshot of all download items for serialization and session saving.
    pub fn get_full_snapshot(&self) -> HashMap<String, DownloadData> {
        let mut req = HashMap::new();
        for entry in self.download_items.iter() {
            let id = entry.key();
            let data = entry.value().snapshot_data();
            req.insert(id.clone(), data);
        }
        req
    }

    /// Serializes the current download manager session state to a JSON file at the given path.
    pub async fn save_session(&self, to_save_to: &Path) -> Result<(), DataSerializationError> {
        let snapshot = self.get_full_snapshot();
        let json_str = serde_json::to_string(&snapshot)?;
        fs::write(&to_save_to, json_str).await?;
        Ok(())
    }
}
