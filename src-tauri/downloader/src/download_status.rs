use crate::download_run_error::DownloadRunError;
use serde::{self, Deserialize, Serialize};

/// Represents the current lifecycle state of a download item.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadState {
    /// Download has been stopped (e.g., by user command).
    Stopped,
    /// Download is actively transferring data.
    Running { download_speed: f64 },
    /// Download completed successfully with the given total size.
    Completed { size: u64 },
    /// Download failed with the specified error.
    Failed(DownloadRunError),
    /// Download is paused and can be resumed.
    Paused,
    /// Post-download checksum verification is in progress.
    Verifying,
    Retrying {
        attempt: u32,
        max_attempts: u32,
        wait_secs: u64,
    },
}

/// Aggregated status snapshot for a download item at a point in time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadStatus {
    /// Total file size in bytes (if known).
    pub total: Option<u64>,
    /// Bytes downloaded so far.
    pub downloaded: u64,
    /// Current lifecycle state.
    pub state: DownloadState,
}
