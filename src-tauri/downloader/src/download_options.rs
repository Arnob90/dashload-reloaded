use crate::request_headers::RequestHeaders;
use crate::retry_scheme::RetryScheme;
use crate::verifier::VerifierMaybe;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// All parameters required to create and start a download.
///
/// Use [`DownloadOptions::new`] and the builder methods to construct this
/// without positional argument explosion:
///
/// ```
/// use dashload_downloader::download_options::DownloadOptions;
///
/// let opts = DownloadOptions::new("https://example.com/file.bin", "/tmp/file.bin")
///     .with_segments(8);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadOptions {
    /// The target download URL.
    pub url: String,
    /// The local destination file path.
    pub filepath: PathBuf,
    /// Number of concurrent segments/chunks to split the download into.
    pub number_of_segments: u64,
    /// Optional checksum verifier.
    pub verifier: VerifierMaybe,
    /// Custom HTTP headers such as User-Agent, cookies, or referrer.
    pub request_headers: RequestHeaders,
    /// Retry/backoff policy.
    pub retry_scheme: RetryScheme,
    /// Optional pre-existing ID. A new UUID is generated when `None`.
    pub id: Option<String>,
}

impl DownloadOptions {
    /// Creates options with sensible defaults: 4 segments, no verifier,
    /// default headers, default retry scheme, and no ID.
    pub fn new(url: impl Into<String>, filepath: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            filepath: filepath.into(),
            number_of_segments: 4,
            verifier: VerifierMaybe::none(),
            request_headers: RequestHeaders::default(),
            retry_scheme: RetryScheme::default(),
            id: None,
        }
    }

    /// Sets the number of segments/chunks.
    pub fn with_segments(mut self, number_of_segments: u64) -> Self {
        self.number_of_segments = number_of_segments;
        self
    }

    /// Sets an explicit download ID.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the checksum verifier.
    pub fn with_verifier(mut self, verifier: VerifierMaybe) -> Self {
        self.verifier = verifier;
        self
    }

    /// Sets the HTTP request headers.
    pub fn with_headers(mut self, request_headers: RequestHeaders) -> Self {
        self.request_headers = request_headers;
        self
    }

    /// Sets the retry scheme.
    pub fn with_retry(mut self, retry_scheme: RetryScheme) -> Self {
        self.retry_scheme = retry_scheme;
        self
    }
}
