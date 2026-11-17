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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// `From<std::io::Error>` maps to `DiskError` with the error message and `path: None`.
    #[test]
    fn io_error_to_disk_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err: DownloadRunError = io_err.into();
        assert_eq!(
            err,
            DownloadRunError::DiskError {
                message: "denied".to_string(),
                path: None,
            }
        );
    }

    /// `From<reqwest::Error>` maps to `NetworkError` with status_code None
    /// for a connection-refused error (no HTTP status available).
    #[tokio::test]
    async fn reqwest_connect_refused_to_network_error() {
        // Connect-refused is deterministic on Linux: no status code is set.
        let client = reqwest::Client::new();
        let err = client
            .get("http://127.0.0.1:1/x")
            .send()
            .await
            .expect_err("connection should be refused");
        let err: DownloadRunError = err.into();
        match err {
            DownloadRunError::NetworkError {
                message,
                status_code,
            } => {
                assert_eq!(status_code, None);
                assert!(!message.is_empty());
            }
            other => panic!("expected NetworkError, got {other:?}"),
        }
    }

    /// `FailureDetails::new` records a timestamp near "now", no segment, 0 attempts.
    #[test]
    fn failure_details_new_defaults() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let d = FailureDetails::new(DownloadRunError::AlreadyRunning);
        assert!(d.timestamp_secs >= now - 5 && d.timestamp_secs <= now + 5);
        assert_eq!(d.failed_segment_index, None);
        assert_eq!(d.attempts_made, 0);
        assert_eq!(d.error, DownloadRunError::AlreadyRunning);
    }

    /// Builder chaining sets the right fields.
    #[test]
    fn failure_details_with_segment_and_attempts() {
        let d = FailureDetails::new(DownloadRunError::AlreadyRunning)
            .with_segment(3)
            .with_attempts(7);
        assert_eq!(d.failed_segment_index, Some(3));
        assert_eq!(d.attempts_made, 7);
    }

    /// Serde round-trip for every `DownloadRunError` variant.
    #[test]
    fn all_variants_serde_roundtrip() {
        let cases: Vec<DownloadRunError> = vec![
            DownloadRunError::NetworkError {
                message: "boom".to_string(),
                status_code: Some(503),
            },
            DownloadRunError::ServerError {
                message: "no range".to_string(),
                status_code: Some(416),
            },
            DownloadRunError::DiskError {
                message: "disk full".to_string(),
                path: Some("/tmp/x".to_string()),
            },
            DownloadRunError::AlreadyRunning,
            DownloadRunError::VerifyError {
                expected: "abc".to_string(),
                actual: "def".to_string(),
            },
            DownloadRunError::TimeoutError { secs: 42 },
        ];
        for c in cases {
            let json = serde_json::to_string(&c).unwrap();
            let back: DownloadRunError = serde_json::from_str(&json).unwrap();
            assert_eq!(c, back, "round-trip failed for {json}");
        }
    }

    /// `Display` output regression protection for user-facing strings.
    #[test]
    fn display_strings() {
        assert_eq!(
            format!(
                "{}",
                DownloadRunError::NetworkError {
                    message: "msg".to_string(),
                    status_code: Some(503),
                }
            ),
            "Network error (Some(503)): msg"
        );
        assert_eq!(
            format!("{}", DownloadRunError::AlreadyRunning),
            "Download is already running"
        );
        assert_eq!(
            format!(
                "{}",
                DownloadRunError::ServerError {
                    message: "m".to_string(),
                    status_code: None,
                }
            ),
            "Server error (None): m"
        );
        assert_eq!(
            format!(
                "{}",
                DownloadRunError::DiskError {
                    message: "m".to_string(),
                    path: Some("/p".to_string()),
                }
            ),
            "Disk I/O error: m (path: Some(\"/p\"))"
        );
        assert_eq!(
            format!(
                "{}",
                DownloadRunError::VerifyError {
                    expected: "e".to_string(),
                    actual: "a".to_string(),
                }
            ),
            "Failed to verify completed download: expected e, got a"
        );
        assert_eq!(
            format!("{}", DownloadRunError::TimeoutError { secs: 9 }),
            "Timed out waiting for next chunk after 9s"
        );
    }
}