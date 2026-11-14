use crate::download_data::DownloadData;
use crate::download_data_factory::DownloadDataFactory;
use crate::download_item::DownloadItem;
use crate::download_options::DownloadOptions;
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
use atomic_time::{AtomicDuration, Ordering};
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
    #[error("Only paused (or already running) items can be resumed. Current state: {0:?}")]
    ResumeError(DownloadState),
    #[error("Only stopped or failed items can be retried. Current state: {0:?}")]
    RetryError(DownloadState),
}

/// The core manager for handling multiple concurrent downloads, supporting pause,
/// resume, status polling, verification, and state serialization.
pub struct DownloadManager {
    /// Thread-safe map storing active download items keyed by their unique string IDs.
    pub download_items: DashMap<String, Arc<DownloadItem>>,
    /// Shared HTTP client (`reqwest::Client`) used for making download requests.
    pub client: reqwest::Client,
    /// Shared, atomically-updatable polling interval.
    pub poll_every: Arc<AtomicDuration>,
    /// Background runner tasks keyed by download ID, used to await termination on
    /// stop/retry before reusing a file path.
    pub runner_handles: DashMap<String, tokio::task::JoinHandle<()>>,
    /// Per-download operation locks serializing stop/resume/retry transitions.
    op_locks: DashMap<String, Arc<tokio::sync::Mutex<()>>>,
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
            poll_every: Arc::new(AtomicDuration::from(Duration::from_millis(
                poll_every_ms,
            ))),
            client,
            runner_handles: DashMap::new(),
            op_locks: DashMap::new(),
        }
    }

    /// Sets the polling interval atomically. All active segments pick up the
    /// new rate on their next progress tick.
    pub fn set_poll_every(&self, poll_every_ms: u64) {
        self.poll_every
            .store(Duration::from_millis(poll_every_ms), Ordering::SeqCst);
    }

    /// Returns the per-download operation lock, creating it on first use.
    fn op_lock(&self, id: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.op_locks
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
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
            let item = DownloadItem::create_from_data(
                client.clone(),
                data,
                manager.poll_every.clone(),
            )
            .await
            .map_err(|e| ManagerError::DownloadCreationError(e.to_string()))?;
            manager.download_items.insert(id.clone(), item.clone());
            let handle = tokio::spawn(async move {
                let _ = item.start().await;
            });
            manager.runner_handles.insert(id, handle);
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
    /// initializes the download item, generates a unique ID (unless one is supplied via
    /// [`DownloadOptions::id`]), and starts it in a background task.
    ///
    /// # Errors
    /// - `ManagerError::SegmentedNotSupported` if the server does not support segmented
    ///   downloads. Providing a fallback is the caller's responsibility (e.g. retrying
    ///   with a single segment).
    pub async fn add_download(&self, opts: DownloadOptions) -> Result<String, ManagerError> {
        let DownloadOptions {
            url,
            filepath,
            number_of_segments,
            verifier,
            request_headers,
            retry_scheme,
            id,
        } = opts;
        let id = id.unwrap_or_else(|| Uuid::new_v4().to_string());
        println!(
            "Adding download: {} -> {:?} ({} segments)",
            url, filepath, number_of_segments
        );
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
        let item = DownloadItem::create_from_data(
            self.client.clone(),
            data,
            self.poll_every.clone(),
        )
        .await
        .map_err(|e| ManagerError::DownloadCreationError(e.to_string()))?;

        self.download_items.insert(id.clone(), item.clone());

        // 3. Spawn background runner task
        let item_clone = item.clone();
        let handle = tokio::spawn(async move {
            let _ = item_clone.start().await;
        });
        self.runner_handles.insert(id.clone(), handle);
        eprintln!("Download added with ID: {}", id);

        Ok(id)
    }

    /// Resumes a paused download from its saved state.
    pub async fn resume(&self, id: &str) -> Result<(), ManagerError> {
        let lock = self.op_lock(id);
        let _op_guard = lock.lock().await;
        let mut item_data = {
            self.download_items
                .get(id)
                .ok_or(ManagerError::IdDoesNotExist)?
                .snapshot_data()
        };
        if matches!(item_data.state, DownloadState::Running { .. }) {
            // Already running (e.g. reconstructed and auto-started): resuming is a no-op.
            return Ok(());
        }
        if !matches!(item_data.state, DownloadState::Paused) {
            return Err(ManagerError::ResumeError(item_data.state));
        }
        // Ensure the previous runner has finished before starting a new one.
        if let Some((_, old_handle)) = self.runner_handles.remove(id) {
            let _ = old_handle.await;
        }
        item_data.state = DownloadState::Running {
            download_speed: 0.0,
        };
        let new_item = DownloadItem::create_from_data(
            self.client.clone(),
            item_data,
            self.poll_every.clone(),
        )
        .await
        .map_err(|e| ManagerError::OperationFailed(e.to_string()))?;
        let item_clone = new_item.clone();
        let handle = tokio::spawn(async move {
            let _ = item_clone.start().await;
        });
        self.runner_handles.insert(id.to_string(), handle);
        self.download_items.insert(id.to_string(), new_item);
        Ok(())
    }
    pub async fn retry(&self, id: &str) -> Result<String, ManagerError> {
        let lock = self.op_lock(id);
        let _op_guard = lock.lock().await;
        // Collect everything inside a block so the DashMap read guard is dropped
        // before `add_download` awaits and re-inserts into the same shard.
        let (state, url, filepath, number_of_segments, verifier, request_headers, retry_scheme) = {
            let item = self
                .download_items
                .get(id)
                .ok_or(ManagerError::IdDoesNotExist)?;
            (
                item.get_status().state,
                item.url.clone(),
                item.filepath.clone(),
                item.segments.len(),
                item.verifier.clone(),
                item.segments[0].snapshot().headers,
                item.retry_scheme.clone(),
            )
        };
        if !matches!(state, DownloadState::Stopped | DownloadState::Failed(_)) {
            return Err(ManagerError::RetryError(state));
        }
        // Wait for the previous runner to fully terminate before reusing the
        // same file path, otherwise its cleanup could unlink the new file.
        if let Some((_, old_handle)) = self.runner_handles.remove(id) {
            let _ = old_handle.await;
        }
        self.add_download(
            DownloadOptions::new(url, filepath)
                .with_segments(number_of_segments as u64)
                .with_verifier(verifier)
                .with_headers(request_headers)
                .with_retry(retry_scheme)
                .with_id(id),
        )
        .await
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

    pub async fn stop(&self, id: &str) -> Result<(), ManagerError> {
        let lock = self.op_lock(id);
        let _op_guard = lock.lock().await;
        // Clone the Arc so the DashMap guard is dropped before awaiting.
        let item = self
            .download_items
            .get(id)
            .map(|entry| entry.value().clone())
            .ok_or(ManagerError::IdDoesNotExist)?;
        item.stop().await;
        if let Some((_, handle)) = self.runner_handles.remove(id) {
            let _ = handle.await;
        }
        Ok(())
    }

    /// Deletes a download: stops it, removes it from the manager, and cleans up
    /// its background runner task under the operation lock to prevent concurrent races.
    pub async fn delete(&self, id: &str) -> Result<(), ManagerError> {
        let lock = self.op_lock(id);
        let _op_guard = lock.lock().await;
        let item = self
            .download_items
            .get(id)
            .map(|entry| entry.value().clone())
            .ok_or(ManagerError::IdDoesNotExist)?;
        item.stop().await;
        if let Some((_, handle)) = self.runner_handles.remove(id) {
            let _ = handle.await;
        }
        self.download_items.remove(id);
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
