use std::path::PathBuf;

use crate::{
    download_status::DownloadState, in_place_segment::InPlaceSegmentData, verifier::VerifierMaybe,
};
use serde::{Deserialize, Serialize};

/// Complete persistent state for a single download, used for session serialization and reconstruction.
#[derive(Serialize, Deserialize, Clone)]
pub struct DownloadData {
    /// The source download URL.
    pub url: String,
    /// Local destination file path.
    pub filepath: PathBuf,
    /// Individual segment data definitions for this download.
    pub segments: Vec<InPlaceSegmentData>,
    /// Total file size in bytes (if known).
    pub length: Option<u64>,
    /// Current download state (running, paused, completed, etc.).
    pub state: DownloadState,
    /// Optional hash verifier for post-download integrity checking.
    pub verifier: VerifierMaybe,
    /// Retry scheme for download
    pub retry_scheme: crate::retry_scheme::RetryScheme,
}
