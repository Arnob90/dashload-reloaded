//! Shared utilities for wiremock-based integration tests.

use dashload_downloader::{
    download_manager::DownloadManager,
    download_run_error::DownloadRunError,
    download_status::{DownloadState, DownloadStatus},
    request_headers::RequestHeaders,
    verifier::{HashAlgo, VerifierMaybe},
};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

    // Range requests for each segment
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
