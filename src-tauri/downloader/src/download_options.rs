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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retry_scheme::ExponentialBackoff;
    use crate::verifier::HashAlgo;
    use std::time::Duration;

    fn custom_headers() -> RequestHeaders {
        RequestHeaders {
            cookies: Some("session=abc".to_string()),
            referrer: Some("https://example.com/".to_string()),
            user_agent: Some("custom-agent/1.0".to_string()),
        }
    }

    fn custom_retry() -> RetryScheme {
        RetryScheme {
            max_attempts: 2,
            backoff: ExponentialBackoff::new(
                Duration::from_millis(10),
                Duration::from_millis(100),
                1.5,
                false,
            ),
            stream_timeout: Duration::from_secs(1),
            connection_timeout: Duration::from_secs(2),
        }
    }

    #[test]
    fn new_uses_documented_defaults() {
        let opts = DownloadOptions::new("https://example.com/f.bin", "/tmp/f.bin");

        assert_eq!(opts.url, "https://example.com/f.bin");
        assert_eq!(opts.filepath, PathBuf::from("/tmp/f.bin"));
        assert_eq!(opts.number_of_segments, 4);
        assert!(!opts.verifier.is_some());
        assert_eq!(
            opts.request_headers.user_agent,
            RequestHeaders::default().user_agent
        );
        assert_eq!(
            opts.retry_scheme.max_attempts,
            RetryScheme::default().max_attempts
        );
        assert!(opts.id.is_none());
    }

    #[test]
    fn builders_replace_values_and_chain() {
        let verifier = VerifierMaybe::new(HashAlgo::Sha256, "deadbeef".to_string());
        let opts = DownloadOptions::new("https://example.com/f.bin", "/tmp/f.bin")
            .with_segments(1)
            .with_id("abc")
            .with_verifier(verifier)
            .with_headers(custom_headers())
            .with_retry(custom_retry());

        assert_eq!(opts.number_of_segments, 1);
        assert_eq!(opts.id.as_deref(), Some("abc"));
        assert!(opts.verifier.is_some());
        assert_eq!(opts.verifier.algorithm(), Some(HashAlgo::Sha256));
        assert_eq!(opts.verifier.expected_hash(), Some("deadbeef"));
        assert_eq!(opts.request_headers.cookies.as_deref(), Some("session=abc"));
        assert_eq!(
            opts.request_headers.referrer.as_deref(),
            Some("https://example.com/")
        );
        assert_eq!(
            opts.request_headers.user_agent.as_deref(),
            Some("custom-agent/1.0")
        );
        assert_eq!(opts.retry_scheme.max_attempts, 2);
    }

    #[test]
    fn builder_last_call_wins() {
        let opts = DownloadOptions::new("https://example.com/f.bin", "/tmp/f.bin")
            .with_segments(2)
            .with_segments(8)
            .with_id("first")
            .with_id("second");

        assert_eq!(opts.number_of_segments, 8);
        assert_eq!(opts.id.as_deref(), Some("second"));
    }

    #[test]
    fn serde_uses_snake_case_and_matches_frontend_payload() {
        let opts = DownloadOptions::new("https://example.com/f.bin", "/tmp/f.bin")
            .with_segments(8)
            .with_headers(custom_headers())
            .with_retry(custom_retry());

        let value = serde_json::to_value(&opts).expect("serialize DownloadOptions");
        let object = value.as_object().expect("DownloadOptions is a JSON object");

        let expected_keys = [
            "url",
            "filepath",
            "number_of_segments",
            "verifier",
            "request_headers",
            "retry_scheme",
            "id",
        ];
        assert_eq!(object.len(), expected_keys.len());
        for key in expected_keys {
            assert!(object.contains_key(key), "missing wire field `{key}`");
        }
        assert!(!object.contains_key("numberOfSegments"));
        assert!(!object.contains_key("requestHeaders"));
        assert!(!object.contains_key("retryScheme"));

        // `None` must serialize as JSON `null`, matching the frontend schema.
        assert!(object["id"].is_null());
        assert!(object["verifier"]["verifier"].is_null());

        // Literal payload shaped like the frontend `DownloadOptionsSchema`.
        let frontend_payload = r#"{
            "url": "https://example.com/f.bin",
            "filepath": "/tmp/f.bin",
            "number_of_segments": 8,
            "verifier": { "verifier": null },
            "request_headers": {
                "cookies": null,
                "referrer": null,
                "user_agent": "custom-agent/1.0"
            },
            "retry_scheme": {
                "max_attempts": 2,
                "backoff": {
                    "base_delay": { "secs": 0, "nanos": 10000000 },
                    "max_delay": { "secs": 0, "nanos": 100000000 },
                    "factor": 1.5,
                    "jitter": false
                },
                "stream_timeout": { "secs": 1, "nanos": 0 },
                "connection_timeout": { "secs": 2, "nanos": 0 }
            },
            "id": null
        }"#;

        let round_tripped: DownloadOptions =
            serde_json::from_str(frontend_payload).expect("deserialize frontend payload");

        assert_eq!(round_tripped.url, "https://example.com/f.bin");
        assert_eq!(round_tripped.filepath, PathBuf::from("/tmp/f.bin"));
        assert_eq!(round_tripped.number_of_segments, 8);
        assert!(!round_tripped.verifier.is_some());
        assert_eq!(
            round_tripped.request_headers.user_agent.as_deref(),
            Some("custom-agent/1.0")
        );
        assert_eq!(round_tripped.retry_scheme.max_attempts, 2);
        assert!(round_tripped.id.is_none());
    }

    #[test]
    fn pathbuf_conversion_accepts_path_and_string() {
        let from_path = DownloadOptions::new("u", PathBuf::from("/tmp/a"));
        let from_str = DownloadOptions::new("u", "/tmp/a");

        assert_eq!(from_path.filepath, PathBuf::from("/tmp/a"));
        assert_eq!(from_str.filepath, PathBuf::from("/tmp/a"));
    }
}
