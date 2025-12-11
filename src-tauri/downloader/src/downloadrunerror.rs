use crate::errors::get_err_string_for_multi_errors;
use crate::normal_download_strategies::GetFileError;
use crate::segmented_verifier::SegmentVerificationError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadRunError {
    #[error("Failed to open file")]
    FileOpenError,
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
    #[error("Failed to create or resume from file")]
    FileCreationError(GetFileError),
    #[error("Failed to verify that the download can be segmented")]
    SegmentVerificationError(SegmentVerificationError),
}

#[derive(Error, Debug, Serialize, Clone, PartialEq)]
pub struct DownloadRunErrors {
    pub errors: Vec<DownloadRunError>,
}
impl std::fmt::Display for DownloadRunErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", get_err_string_for_multi_errors(&self.errors))
    }
}

impl From<DownloadRunError> for DownloadRunErrors {
    fn from(value: DownloadRunError) -> Self {
        DownloadRunErrors {
            errors: vec![value],
        }
    }
}
