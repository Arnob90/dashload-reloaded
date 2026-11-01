//! Shared utilities for wiremock-based integration tests.

use dashload_downloader::{
    download_manager::DownloadManager,
    download_status::{DownloadState, DownloadStatus},
    request_headers::RequestHeaders,
    retry_scheme::{ExponentialBackoff, RetryScheme},
    verifier::{HashAlgo, VerifierMaybe},
};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// Standard test file size: 1 MB
pub const TEST_FILE_SIZE: u64 = 1_000_000;
/// Number of segments for segmented tests
pub const TEST_SEGMENTS: u64 = 4;
/// Segment size (TEST_FILE_SIZE / TEST_SEGMENTS)
pub const SEGMENT_SIZE: u64 = TEST_FILE_SIZE / TEST_SEGMENTS;

/// Deterministic test file content (1 MB of pseudo-random bytes)
pub fn test_file_content() -> Vec<u8> {
    // Use a simple deterministic pattern for reproducibility
    let mut data = Vec::with_capacity(TEST_FILE_SIZE as usize);
    for i in 0..TEST_FILE_SIZE {
        data.push((i.wrapping_mul(31).wrapping_add(7)) as u8);
    }
    data
}

/// Expected SHA256 of test_file_content()
pub const TEST_FILE_SHA256: &str =
    "668f6709eed11666baa9f0fcd94cbae12c6ad4236918792be36e62d03257fc44";

/// Expected SHA512 of test_file_content()
pub const TEST_FILE_SHA512: &str = "d1373ca7e98ef65f247a5bf35fcdc0fc9a9915790e669659f3ccc31ec250c327c2fa26a68e049b9bde8c19ac3b5f6e05c42ea9aab2d3b36d2af7cea522106089";

/// Expected MD5 of test_file_content()
pub const TEST_FILE_MD5: &str = "c9bd641122080114548d4af2a41a7e11";

/// Creates a unique temporary file path for tests
pub fn temp_filepath(prefix: &str) -> PathBuf {
    let filename = format!("{}_{}.bin", prefix, uuid::Uuid::new_v4().as_simple());
    std::env::temp_dir().join(filename)
}

/// Retry scheme for tests that expect a successful transfer.
pub fn default_retry_scheme() -> RetryScheme {
    RetryScheme::default()
}

/// Retry scheme with retries disabled.
///
/// Failure-path tests assert on the *terminal* error produced by a single
/// attempt. The public API now retries transient failures by default, so these
/// tests opt out to keep their original single-attempt semantics and bounded
/// run time.
pub fn no_retry_scheme() -> RetryScheme {
    RetryScheme {
        max_attempts: 0,
        backoff: ExponentialBackoff::new(
            Duration::from_millis(1),
            Duration::from_millis(1),
            1.0,
            false,
        ),
    }
}

/// Default test headers (Tor Browser / Firefox ESR User-Agent)
pub fn default_test_headers() -> RequestHeaders {
    RequestHeaders {
        user_agent: Some(
            "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0".to_string(),
        ),
        referrer: Some("https://example.com/".to_string()),
        cookies: None,
    }
}

pub async fn setup_segmented_mock_server() -> MockServer {
    let mock_server = MockServer::start().await;
    let content = test_file_content();
    Mock::given(method("HEAD"))
        .and(path("/test-file.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Length", TEST_FILE_SIZE.to_string())
                .insert_header("Accept-Ranges", "bytes"),
        )
        .mount(&mock_server)
        .await;

    // --- NEW: Mock for the verification probe (bytes=0-0) ---
    let probe_body = &content[0..=0];
    let probe_content_range = format!("bytes 0-0/{}", TEST_FILE_SIZE);

    Mock::given(method("GET"))
        .and(path("/test-file.bin"))
        .and(header("Range", "bytes=0-0"))
        .respond_with(
            ResponseTemplate::new(206)
                .insert_header("Content-Range", probe_content_range)
                .insert_header("Content-Length", "1")
                .set_body_bytes(probe_body),
        )
        .mount(&mock_server)
        .await;

    // Range requests for each actual segment
    for segment_idx in 0..TEST_SEGMENTS {
        let start = segment_idx * SEGMENT_SIZE;
        let end = start + SEGMENT_SIZE - 1;
        let range_header = format!("bytes={}-{}", start, end);
        let content_range = format!("bytes {}-{}/{}", start, end, TEST_FILE_SIZE);
        let body = &content[start as usize..=end as usize];

        Mock::given(method("GET"))
            .and(path("/test-file.bin"))
            .and(header("Range", range_header.as_str()))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", content_range)
                    .insert_header("Content-Length", SEGMENT_SIZE.to_string())
                    .set_body_bytes(body),
            )
            .mount(&mock_server)
            .await;
    }

    mock_server
}

/// Creates a mock server that serves a 1MB file WITHOUT range support (single stream)
pub async fn setup_single_stream_mock_server() -> MockServer {
    let mock_server = MockServer::start().await;
    let content = test_file_content();

    // HEAD request - returns file size but NO Accept-Ranges
    Mock::given(method("HEAD"))
        .and(path("/test-file.bin"))
        .respond_with(
            ResponseTemplate::new(200).insert_header("Content-Length", TEST_FILE_SIZE.to_string()),
            // No Accept-Ranges header
        )
        .mount(&mock_server)
        .await;

    // GET request - returns full file (no range support)
    Mock::given(method("GET"))
        .and(path("/test-file.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Length", TEST_FILE_SIZE.to_string())
                .set_body_bytes(content),
        )
        .mount(&mock_server)
        .await;

    mock_server
}

/// Creates a mock server that returns an error status
pub async fn setup_error_mock_server(status: u16) -> MockServer {
    let mock_server = MockServer::start().await;

    Mock::given(method("HEAD"))
        .and(path("/test-file.bin"))
        .respond_with(ResponseTemplate::new(status))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/test-file.bin"))
        .respond_with(ResponseTemplate::new(status))
        .mount(&mock_server)
        .await;

    mock_server
}

/// Creates a mock server that supports dynamic range responses (for resume testing)
/// Uses static range responses for each segment - tests will request specific ranges
pub async fn setup_dynamic_range_mock_server() -> MockServer {
    let mock_server = MockServer::start().await;
    let content = test_file_content();

    // HEAD request
    Mock::given(method("HEAD"))
        .and(path("/test-file.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Length", TEST_FILE_SIZE.to_string())
                .insert_header("Accept-Ranges", "bytes"),
        )
        .mount(&mock_server)
        .await;

    // Range-support probe issued by `DownloadManager::add_download` before
    // segmenting (always requests the single byte at offset 0).
    Mock::given(method("GET"))
        .and(path("/test-file.bin"))
        .and(header("Range", "bytes=0-0"))
        .respond_with(
            ResponseTemplate::new(206)
                .insert_header("Content-Range", format!("bytes 0-0/{}", TEST_FILE_SIZE))
                .insert_header("Content-Length", "1")
                .set_body_bytes(&content[0..=0]),
        )
        .mount(&mock_server)
        .await;

    // Range requests for each segment
    for segment_idx in 0..TEST_SEGMENTS {
        let start = segment_idx * SEGMENT_SIZE;
        let end = start + SEGMENT_SIZE - 1;
        let range_header = format!("bytes={}-{}", start, end);
        let content_range = format!("bytes {}-{}/{}", start, end, TEST_FILE_SIZE);
        let body = &content[start as usize..=end as usize];

        // Keep the final segment in flight long enough for session tests to
        // pause a genuinely in-progress download while earlier segments have
        // already recorded progress.
        let mut response = ResponseTemplate::new(206)
            .insert_header("Content-Range", content_range)
            .insert_header("Content-Length", SEGMENT_SIZE.to_string())
            .set_body_bytes(body);
        if segment_idx == TEST_SEGMENTS - 1 {
            response = response.set_delay(Duration::from_secs(2));
        }

        Mock::given(method("GET"))
            .and(path("/test-file.bin"))
            .and(header("Range", range_header.as_str()))
            .respond_with(response)
            .mount(&mock_server)
            .await;
    }

    // Also support full file GET (for single stream fallback)
    Mock::given(method("GET"))
        .and(path("/test-file.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Length", TEST_FILE_SIZE.to_string())
                .set_body_bytes(content),
        )
        .mount(&mock_server)
        .await;

    mock_server
}

/// Polls until download reaches a terminal state or timeout
pub async fn poll_until_completed(
    manager: &DownloadManager,
    id: &str,
    timeout_ms: u64,
) -> Result<DownloadStatus, String> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    loop {
        if start.elapsed() > timeout {
            return Err("Timeout waiting for download to complete".into());
        }

        let status = manager
            .get_item_status(id)
            .map_err(|e| format!("Failed to get status: {}", e))?;

        match status.state {
            DownloadState::Completed { .. } | DownloadState::Failed(_) | DownloadState::Stopped => {
                return Ok(status);
            }
            _ => {}
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub struct DynamicRangeResponder {
    pub content: Vec<u8>,
    pub delay: Duration,
}

impl Respond for DynamicRangeResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let total_size = self.content.len();

        // 1. Handle HEAD requests
        if request.method.as_str() == "HEAD" {
            return ResponseTemplate::new(200)
                .insert_header("Content-Length", total_size.to_string())
                .insert_header("Accept-Ranges", "bytes");
        }

        // 2. Handle GET with Range header
        if let Some(range_header) = request.headers.get("range").and_then(|v| v.to_str().ok())
            && let Some(range_spec) = range_header.strip_prefix("bytes=")
        {
            let parts: Vec<&str> = range_spec.split('-').collect();
            let start: usize = parts[0].parse().unwrap_or(0);
            let end: usize = if parts.len() > 1 && !parts[1].is_empty() {
                parts[1].parse().unwrap_or(total_size - 1)
            } else {
                total_size - 1
            };

            let end = end.min(total_size - 1);
            let slice = &self.content[start..=end];
            let delay = if range_spec == "0-0" {
                Duration::ZERO // No delay for the probe
            } else {
                self.delay // Apply configured delay for other ranges
            };

            return ResponseTemplate::new(206)
                .insert_header(
                    "Content-Range",
                    format!("bytes {}-{}/{}", start, end, total_size),
                )
                .insert_header("Content-Length", slice.len().to_string())
                .set_body_bytes(slice.to_vec())
                .set_delay(delay);
        }

        // 3. Handle standard GET without Range (returns full content)
        ResponseTemplate::new(200)
            .insert_header("Content-Length", total_size.to_string())
            .insert_header("Accept-Ranges", "bytes")
            .set_body_bytes(self.content.clone())
            .set_delay(self.delay)
    }
}

/// Creates a universal mock server serving dummy data of any size
pub async fn start_universal_mock_server(size_bytes: usize, delay: Duration) -> MockServer {
    let mock_server = MockServer::start().await;
    let dummy_content = vec![0x42; size_bytes]; // Fill with dummy byte (e.g. 0x42)

    Mock::given(method("HEAD"))
        .and(path("/test-file.bin"))
        .respond_with(DynamicRangeResponder {
            content: dummy_content.clone(),
            delay: Duration::ZERO,
        })
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/test-file.bin"))
        .respond_with(DynamicRangeResponder {
            content: dummy_content,
            delay,
        })
        .mount(&mock_server)
        .await;

    mock_server
}

/// Cleans up a test file
pub async fn cleanup_file(path: &PathBuf) {
    let _ = fs::remove_file(path).await;
}

/// Creates a verifier with the correct hash for test_file_content()
pub fn create_sha256_verifier() -> VerifierMaybe {
    VerifierMaybe::new(HashAlgo::Sha256, TEST_FILE_SHA256.to_string())
}

/// Creates a verifier with an incorrect hash (for testing verification failure)
pub fn create_wrong_verifier() -> VerifierMaybe {
    VerifierMaybe::new(HashAlgo::Sha256, "wrong_hash".to_string())
}
