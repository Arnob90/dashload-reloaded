use reqwest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors related to retrieving file metadata (e.g., content-disposition filename).
pub enum GetFilePathError {
    RequestError(reqwest::Error),
    IllBehavedServer,
}

/// Rich failure details attached to a `DownloadState::Failed` variant.
/// Carries the typed error plus diagnostic metadata (timestamp, segment context,
/// retry attempts exhausted) so the frontend can surface actionable context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureDetails {
    /// The underlying typed error.
    pub error: DownloadRunError,
    /// Unix timestamp (seconds) when the failure was recorded.
    pub timestamp_secs: u64,
    /// Zero-based index of the segment that triggered the failure (if known).
    pub failed_segment_index: Option<usize>,
    /// Number of automatic retry attempts that were made before giving up.
    pub attempts_made: u32,
}

impl FailureDetails {
    pub fn new(error: DownloadRunError) -> Self {
        Self {
            error,
            timestamp_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            failed_segment_index: None,
            attempts_made: 0,
        }
    }

    pub fn with_segment(mut self, index: usize) -> Self {
        self.failed_segment_index = Some(index);
        self
    }

    pub fn with_attempts(mut self, attempts: u32) -> Self {
        self.attempts_made = attempts;
        self
    }
}

/// Core error type representing failures during active download execution.
/// Serialized for transport across threads/processes and for state persistence.
#[derive(Debug, Error, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadRunError {
    /// Network-level failure (DNS, TCP, TLS, connection reset).
    /// **Action**: Check internet connection or retry.
    #[error("Network error ({status_code:?}): {message}")]
    NetworkError {
        message: String,
        status_code: Option<u16>,
    },

    /// Server responded with an error status or unsupported feature (e.g., no range support).
    /// **Action**: Server rejected range requests, 404, 500, etc.
    #[error("Server error ({status_code:?}): {message}")]
    ServerError {
        message: String,
        status_code: Option<u16>,
    },

    /// Local filesystem error (permission denied, disk full, path not found).
    /// **Action**: Check disk space, file permissions, or path.
    #[error("Disk I/O error: {message} (path: {path:?})")]
    DiskError {
        message: String,
        path: Option<String>,
    },

    /// Internal state conflict.
    /// **Action**: Internal state error.
    #[error("Download is already running")]
    AlreadyRunning,

    /// Post-download checksum verification failed.
    /// **Action**: File corrupted or hash mismatch.
    #[error("Failed to verify completed download: expected {expected}, got {actual}")]
    VerifyError {
        expected: String,
        actual: String,
    },

    /// Timed out waiting for next chunk after {secs}s
    #[error("Timed out waiting for next chunk after {secs}s")]
    TimeoutError {
        secs: u64,
    },
}

// Automatic conversion for all std::io::Error (open, seek, write, flush, remove)
impl From<std::io::Error> for DownloadRunError {
    fn from(err: std::io::Error) -> Self {
        Self::DiskError {
            message: err.to_string(),
            path: None,
        }
    }
}

// Automatic conversion for reqwest network errors
impl From<reqwest::Error> for DownloadRunError {
    fn from(err: reqwest::Error) -> Self {
        let status_code = err.status().map(|s| s.as_u16());
        Self::NetworkError {
            message: err.to_string(),
            status_code,
        }
    }
}