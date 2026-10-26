//! WireMock tests for error handling scenarios.

use dashload_downloader::{
    download_manager::DownloadManager,
    download_manager::ManagerError,
    download_run_error::DownloadRunError,
    download_status::{DownloadState, DownloadStatus},
    request_headers::RequestHeaders,
    verifier::VerifierMaybe,
};
use std::path::PathBuf;
use std::time::Duration;
use wiremock::MockServer;

mod wiremock_helpers;
use wiremock_helpers::*;

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
#[tokio::test]
async fn test_network_timeout() {
    let mock_server = MockServer::start().await;
    // Don't mount any handlers - connection will hang and eventually timeout

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_timeout");

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            1,
            VerifierMaybe::none(),
            default_test_headers(),
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
    let content = test_file_content();

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

/// Test: Server closes connection mid-download
#[tokio::test]
async fn test_connection_reset() {
    ///TODO: Dashload-Downloader currently does not have timeouts. Need to add
    let mock_server = MockServer::start().await;
    let content = test_file_content();

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

    // First range request succeeds
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/test-file.bin"))
        .and(wiremock::matchers::header("Range", "bytes=0-249999"))
        .respond_with(
            wiremock::ResponseTemplate::new(206)
                .insert_header(
                    "Content-Range",
                    format!("bytes 0-249999/{}", TEST_FILE_SIZE),
                )
                .set_body_bytes(&content[0..250000]),
        )
        .mount(&mock_server)
        .await;

    // Second range request - connection reset (no response)
    // WireMock doesn't easily support connection reset, so we'll use a delay that exceeds timeout
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/test-file.bin"))
        .and(wiremock::matchers::header("Range", "bytes=250000-499999"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_delay(Duration::from_secs(10)), // Longer than client timeout
        )
        .mount(&mock_server)
        .await;

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
            TEST_SEGMENTS,
            VerifierMaybe::none(),
            default_test_headers(),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 10_000)
        .await
        .expect("Download timed out");

    // Should fail with network error due to timeout
    assert!(
        matches!(status.state, DownloadState::Failed(e) if matches!(e, DownloadRunError::NetworkError(_)))
    );

    cleanup_file(&path).await;
}
