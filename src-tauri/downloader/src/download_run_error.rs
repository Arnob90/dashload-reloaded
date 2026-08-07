use std::path::PathBuf;

use crate::normal_download_strategies::GetFileError;
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
    #[error("Request failed! Reason: {0}")]
    RequestError(String),
    #[error("The server does not have a concept of filesize")]
    LengthlessServerError,
    #[error("Given range is out of range")]
    RangeError,
    #[error("The server violates basic protocols")]
    IllBehavedServer,
    #[error("The server does not support range requests")]
    ServerNotRangable,
    #[error("Invalid header value")]
    InvalidHeaderValue, // New variant for the parse error
    #[error("Failed to open file {0:?}")]
    FileOpenError(PathBuf),
    #[error("Failed to seek file")]
    ServerNotAvailable,
    #[error("Failed to seek file")]
    FileSeekError,
    #[error("Cannot open file to write error, reason: {0}")]
    FileWriteError(String),
    #[error("Download cannot be run twice")]
    AlreadyRunning,
    #[error("The file does not match the one kept before resumption")]
    FileMismatch,
    #[error("Given path is one to a dir")]
    DirectoryGivenError,
    #[error("Failed to open file")]
    GetFileError(GetFileError),
    #[error("Failed to merge download segments")]
    DownloadMergeError,
}
