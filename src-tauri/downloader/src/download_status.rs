use crate::download_run_error::{DownloadRunError, FailureDetails};
use serde::{self, Deserialize, Deserializer, Serialize};

/// Represents the current lifecycle state of a download item.
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadState {
    /// Download has been stopped (e.g., by user command).
    Stopped,
    /// Download is actively transferring data.
    Running { download_speed: f64 },
    /// Download completed successfully with the given total size.
    Completed { size: u64 },
    /// Download failed with the specified rich error details.
    Failed(FailureDetails),
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

impl<'de> Deserialize<'de> for DownloadState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
    D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "kind", content = "payload")]
        enum RawDownloadState {
            Stopped,
            Running { download_speed: f64 },
            Completed { size: u64 },
            Failed(serde_json::Value),
            Paused,
            Verifying,
            Retrying { attempt: u32, max_attempts: u32, wait_secs: u64 },
        }

        let raw = RawDownloadState::deserialize(deserializer)?;
        match raw {
            RawDownloadState::Stopped => Ok(DownloadState::Stopped),
            RawDownloadState::Running { download_speed } => Ok(DownloadState::Running { download_speed }),
            RawDownloadState::Completed { size } => Ok(DownloadState::Completed { size }),
            RawDownloadState::Paused => Ok(DownloadState::Paused),
            RawDownloadState::Verifying => Ok(DownloadState::Verifying),
            RawDownloadState::Retrying { attempt, max_attempts, wait_secs } => {
                Ok(DownloadState::Retrying { attempt, max_attempts, wait_secs })
            }
            RawDownloadState::Failed(val) => {
                // Try parsing as FailureDetails first
                if let Ok(details) = serde_json::from_value::<FailureDetails>(val.clone()) {
                    return Ok(DownloadState::Failed(details));
                }
                // Fallback: legacy format where payload was a bare DownloadRunError
                if let Ok(err) = serde_json::from_value::<DownloadRunError>(val) {
                    return Ok(DownloadState::Failed(FailureDetails::new(err)));
                }
                Err(serde::de::Error::custom("Invalid Failed state payload"))
            }
        }
    }
}

impl DownloadState {
    /// Returns the raw `DownloadRunError` if this state is `Failed`, otherwise `None`.
    pub fn get_error(&self) -> Option<&DownloadRunError> {
        match self {
            DownloadState::Failed(failure) => Some(&failure.error),
            _ => None,
        }
    }
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