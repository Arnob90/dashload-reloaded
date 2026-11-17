use std::path::PathBuf;

use crate::{
    download_status::DownloadState, in_place_segment::InPlaceSegmentData,
    verifier::VerifierMaybe,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Complete persistent state for a single download, used for session serialization and reconstruction.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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
    /// Time elapsed since the download started
    pub elapsed: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full `DownloadData` serde round-trip (the persistence unit).
    #[test]
    fn download_data_serde_roundtrip() {
        let data = DownloadDataFactory::new_segmented(
            "https://example.com/file.bin".to_string(),
            PathBuf::from("/tmp/file.bin"),
            1_000_000,
            2,
            VerifierMaybe::none(),
            HeaderMap::new(),
            RetryScheme::default(),
        );
        let json = serde_json::to_string(&data).unwrap();
        let back: DownloadData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, back);
    }
}
