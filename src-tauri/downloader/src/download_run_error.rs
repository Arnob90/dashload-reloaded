use reqwest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors related to retrieving file metadata (e.g., content-disposition filename).
pub enum GetFilePathError {
    RequestError(reqwest::Error),
    IllBehavedServer,
}

/// Core error type representing failures during active download execution.
/// Serialized for transport across threads/processes and for state persistence.
#[derive(Debug, Error, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadRunError {
    /// Network-level failure (DNS, TCP, TLS, connection reset).
    /// **Action**: Check internet connection or retry.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Server responded with an error status or unsupported feature (e.g., no range support).
    /// **Action**: Server rejected range requests, 404, 500, etc.
    #[error("Server error: {0}")]
    ServerError(String),

    /// Local filesystem error (permission denied, disk full, path not found).
    /// **Action**: Check disk space, file permissions, or path.
    #[error("Disk I/O error: {0}")]
    DiskError(String),

    /// Internal state conflict.
    /// **Action**: Internal state error.
    #[error("Download is already running")]
    AlreadyRunning,

    /// Post-download checksum verification failed.
    /// **Action**: File corrupted or hash mismatch.
    #[error("Failed to verify completed download")]
    VerifyError,

    ///Timed out waiting for next chunk
    #[error("Timed out waiting for next chunk")]
    TimeoutError,
}

// Automatic conversion for all std::io::Error (open, seek, write, flush, remove)
impl From<std::io::Error> for DownloadRunError {
    fn from(err: std::io::Error) -> Self {
        Self::DiskError(err.to_string())
    }
}

// Automatic conversion for reqwest network errors
impl From<reqwest::Error> for DownloadRunError {
    fn from(err: reqwest::Error) -> Self {
        Self::NetworkError(err.to_string())
    }
}
