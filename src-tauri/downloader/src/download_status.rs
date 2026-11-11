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
                // Try parsing as FailureDetails first (current format).
                if let Ok(details) = serde_json::from_value::<FailureDetails>(val.clone()) {
                    return Ok(DownloadState::Failed(details));
                }
                // Fallback: intermediate format where payload was a bare
                // struct-variant DownloadRunError.
                if let Ok(err) = serde_json::from_value::<DownloadRunError>(val.clone()) {
                    return Ok(DownloadState::Failed(FailureDetails::new(err)));
                }
                // Fallback: true legacy format where DownloadRunError variants
                // carried string/unit payloads (pre-error-management rewrite).
                if let Some(err) = legacy_run_error_from_value(&val) {
                    return Ok(DownloadState::Failed(FailureDetails::new(err)));
                }
                Err(serde::de::Error::custom("Invalid Failed state payload"))
            }
        }
    }
}

/// Reconstructs a legacy `DownloadRunError` from a JSON value that predates the
/// struct-variant wire format (e.g. `{"kind":"NetworkError","payload":"connection reset"}`).
/// Returns `None` if the value does not look like a legacy tagged error.
fn legacy_run_error_from_value(val: &serde_json::Value) -> Option<DownloadRunError> {
    let obj = val.as_object()?;
    let kind = obj.get("kind")?.as_str()?;
    let payload = obj.get("payload");
    match kind {
        "NetworkError" => {
            let message = payload
                .and_then(|p| p.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown network error".to_string());
            Some(DownloadRunError::NetworkError {
                message,
                status_code: None,
            })
        }
        "ServerError" => {
            let message = payload
                .and_then(|p| p.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown server error".to_string());
            Some(DownloadRunError::ServerError {
                message,
                status_code: None,
            })
        }
        "DiskError" => {
            let message = payload
                .and_then(|p| p.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown disk error".to_string());
            Some(DownloadRunError::DiskError {
                message,
                path: None,
            })
        }
        "VerifyError" => Some(DownloadRunError::VerifyError {
            expected: "unknown".to_string(),
            actual: "unknown".to_string(),
        }),
        "TimeoutError" => Some(DownloadRunError::TimeoutError { secs: 0 }),
        "AlreadyRunning" => Some(DownloadRunError::AlreadyRunning),
        _ => None,
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