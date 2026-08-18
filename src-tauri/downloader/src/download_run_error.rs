use reqwest::{self};
use serde::{Deserialize, Serialize};
use thiserror::Error;
pub enum GetFilePathError {
    RequestError(reqwest::Error),
    IllBehavedServer,
}

#[derive(Debug, Error, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadRunError {
    /// Action: Check internet connection or retry
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Action: Server rejected range requests, 404, 500, etc.
    #[error("Server error: {0}")]
    ServerError(String),

    /// Action: Check disk space, file permissions, or path
    #[error("Disk I/O error: {0}")]
    DiskError(String),

    /// Action: Internal state error
    #[error("Download is already running")]
    AlreadyRunning,
    #[error("Failed to verify completed download")]
    VerifyError,
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
