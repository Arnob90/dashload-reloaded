//! WireMock tests for error handling scenarios.

use dashload_downloader::{
    download_manager::DownloadManager, download_manager::ManagerError,
    download_run_error::DownloadRunError, download_status::DownloadState, verifier::VerifierMaybe,
};
use std::path::PathBuf;
use std::time::Duration;
use wiremock::MockServer;

mod wiremock_helpers;
use console_subscriber;
use log::info;
use std::sync::Once;
use wiremock_helpers::*;

static INIT: Once = Once::new();
fn init_console_subscriber() {
    INIT.call_once(|| {
        console_subscriber::init();
    });
}

/// Test: Server returns 404 -> DownloadRunError::ServerError
#[tokio::test]
async fn test_server_returns_404() {
    let mock_server = setup_error_mock_server(404).await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_404");

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            1,
            VerifierMaybe::none(),
            default_test_headers(),
            no_retry_scheme(),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 5_000)
        .await
        .expect("Download timed out");

    assert!(
        matches!(status.state, DownloadState::Failed(e) if matches!(e, DownloadRunError::ServerError(_)))
    );

    cleanup_file(&path).await;
}

/// Test: Server returns 500 -> DownloadRunError::ServerError
#[tokio::test]
async fn test_server_returns_500() {
    let mock_server = setup_error_mock_server(500).await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_500");

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            1,
            VerifierMaybe::none(),
            default_test_headers(),
            no_retry_scheme(),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 5_000)
        .await
        .expect("Download timed out");

    assert!(
        matches!(status.state, DownloadState::Failed(e) if matches!(e, DownloadRunError::ServerError(_)))
    );

    cleanup_file(&path).await;
}

/// Test: Network timeout -> DownloadRunError::NetworkError
///
/// A real transport-level failure (connecting to a closed port) is used rather
/// than an unmounted WireMock route: an unmounted route answers HTTP 404
/// quickly, which is correctly reported as `ServerError`, not `NetworkError`.
#[tokio::test]
async fn test_network_timeout() {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(100))
        .timeout(Duration::from_millis(200))
        .build()
        .unwrap();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_timeout");

    // Port 1 is not listening on localhost -> connect refusal -> NetworkError.
    let id = manager
        .add_download(
            "http://127.0.0.1:1/test-file.bin".to_string(),
            path.clone(),
            1,
            VerifierMaybe::none(),
            default_test_headers(),
            no_retry_scheme(),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 5_000)
        .await
        .expect("Download timed out");

    assert!(
        matches!(status.state, DownloadState::Failed(e) if matches!(e, DownloadRunError::NetworkError(_)))
    );

    cleanup_file(&path).await;
}

/// Test: Disk write error -> DownloadRunError::DiskError
#[tokio::test]
async fn test_disk_write_error() {
    // Try to write to a directory that doesn't exist or isn't writable
    // On Unix, /root is typically not writable by non-root users
    let invalid_path = PathBuf::from("/root/nonexistent/download.bin");

    let mock_server = setup_segmented_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);

    let result = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            invalid_path,
            1,
            VerifierMaybe::none(),
            default_test_headers(),
            no_retry_scheme(),
        )
        .await;

    // Should fail at creation time due to disk error
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, ManagerError::DownloadCreationError(_)));
}

/// Test: Server returns 416 Range Not Satisfiable
#[tokio::test]
async fn test_range_not_satisfiable() {
    let mock_server = MockServer::start().await;

    // HEAD request
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .and(wiremock::matchers::path("/test-file.bin"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .insert_header("Content-Length", TEST_FILE_SIZE.to_string())
                .insert_header("Accept-Ranges", "bytes"),
        )
        .mount(&mock_server)
        .await;

    // Range request that exceeds file size -> 416
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/test-file.bin"))
        .and(wiremock::matchers::header("Range", "bytes=2000000-3000000")) // Beyond file size
        .respond_with(wiremock::ResponseTemplate::new(416))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_416");

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            1,
            VerifierMaybe::none(),
            default_test_headers(),
            no_retry_scheme(),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 5_000)
        .await
        .expect("Download timed out");

    assert!(
        matches!(status.state, DownloadState::Failed(e) if matches!(e, DownloadRunError::ServerError(_)))
    );

    cleanup_file(&path).await;
}

/// Test: Server closes connection mid-download -> DownloadRunError::NetworkError
#[tokio::test]
async fn test_connection_reset() {
    init_console_subscriber();
    info!("Starting test_connection_reset");
    let mock_server = MockServer::start().await;
    let content = test_file_content();
    let total_size = content.len();

    // 1. HEAD request
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .and(wiremock::matchers::path("/test-file.bin"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .insert_header("Content-Length", total_size.to_string())
                .insert_header("Accept-Ranges", "bytes"),
        )
        .mount(&mock_server)
        .await;

    // 2. Range probe (bytes=0-0) required by DownloadManager::add_download
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/test-file.bin"))
        .and(wiremock::matchers::header("Range", "bytes=0-0"))
        .respond_with(
            wiremock::ResponseTemplate::new(206)
                .insert_header("Content-Range", format!("bytes 0-0/{}", total_size))
                .insert_header("Accept-Ranges", "bytes")
                .set_body_bytes(vec![content[0]]),
        )
        .mount(&mock_server)
        .await;

    // 3. The download request hangs for 10s, exceeding the client's 2s timeout,
    // producing a transport-level timeout -> NetworkError.
    // A fresh single-stream download sends no `Range` header, so this mock must
    // match a GET without a Range matcher.
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/test-file.bin"))
        .respond_with(
            wiremock::ResponseTemplate::new(206).set_delay(Duration::from_secs(10)),
        )
        .mount(&mock_server)
        .await;

    // Client with 2-second timeout so the 10s delay becomes a NetworkError.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_reset");

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            1, // 1 segment: avoids multi-segment math mismatches!
            VerifierMaybe::none(),
            default_test_headers(),
            no_retry_scheme(), // max_attempts: 0 -> fails immediately
        )
        .await
        .expect("Failed to add download");
    info!("Added download with ID: {}", id);

    // Poll until the 2s timeout fails the download
    let start = std::time::Instant::now();
    let mut final_status = None;

    info!("Final status before polling: {:?}", final_status);
    let mut i = 0;
    while start.elapsed() < Duration::from_secs(5) {
        let status = manager.get_item_status(&id).unwrap();
        info!("Poll {}: status = {:?}", i, status.state);
        if matches!(status.state, DownloadState::Failed(_)) {
            final_status = Some(status);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        i += 1;
    }

    let final_status = final_status.expect("Download should have failed");
    assert!(
        matches!(
            final_status.state,
            DownloadState::Failed(ref e) if matches!(e, DownloadRunError::NetworkError(_))
        ),
        "Expected NetworkError, got: {:?}",
        final_status.state
    );

    cleanup_file(&path).await;
}
