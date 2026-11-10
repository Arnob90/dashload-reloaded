//! WireMock tests for single-stream (non-segmented) downloads.
use dashload_downloader::download_options::DownloadOptions;

use dashload_downloader::{
    download_manager::DownloadManager,
    download_run_error::DownloadRunError,
    download_status::{DownloadState, DownloadStatus},
    request_headers::RequestHeaders,
    verifier::{HashAlgo, VerifierMaybe},
};
use log::info;
use std::path::PathBuf;
use std::time::Duration;
use wiremock::MockServer;
mod wiremock_helpers;
use wiremock_helpers::*;

/// Test: Single stream download fails if segments are requested but server doesn't support ranges
#[test_log::test(tokio::test)]
async fn test_single_stream_download_completes() {
    info!("Test started");
    let mock_server = setup_single_stream_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_single");
    info!("Reached this point");
    // Request 4 segments but server doesn't support ranges -> should fail
    let id = manager
        .add_download(
            DownloadOptions::new(format!("{}/test-file.bin", mock_server.uri()), path.clone())
                .with_segments(TEST_SEGMENTS)
                .with_verifier(VerifierMaybe::none())
                .with_headers(default_test_headers())
                .with_retry(default_retry_scheme()),
        )
        .await;
    let id_err = id.expect_err("Expected error due to unsupported segmented download");
    assert!(
        matches!(
            id_err,
            dashload_downloader::download_manager::ManagerError::SegmentedNotSupported(_)
        ),
        "Expected SegmentedNotSupported error, got: {:?}",
        id_err
    );
}

/// Test: Single stream download with SHA256 verification
#[tokio::test]
async fn test_single_stream_with_verifier() {
    let mock_server = setup_single_stream_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_single_verified");

    let id = manager
        .add_download(
            DownloadOptions::new(format!("{}/test-file.bin", mock_server.uri()), path.clone())
                .with_segments(1) // Explicitly request 1 segment
                .with_verifier(create_sha256_verifier())
                .with_headers(default_test_headers())
                .with_retry(default_retry_scheme()),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 10_000)
        .await
        .expect("Download timed out");

    assert!(matches!(status.state, DownloadState::Completed { size } if size == TEST_FILE_SIZE));

    // Verify file content
    let content = tokio::fs::read(&path)
        .await
        .expect("Failed to read downloaded file");
    assert_eq!(content, test_file_content());

    cleanup_file(&path).await;
}

/// Test: Single stream with SHA512 verification
#[tokio::test]
async fn test_single_stream_sha512_verifier() {
    let mock_server = setup_single_stream_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_single_sha512");

    let verifier = VerifierMaybe::new(HashAlgo::Sha512, TEST_FILE_SHA512.to_string());

    let id = manager
        .add_download(
            DownloadOptions::new(format!("{}/test-file.bin", mock_server.uri()), path.clone())
                .with_segments(1)
                .with_verifier(verifier)
                .with_headers(default_test_headers())
                .with_retry(default_retry_scheme()),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 10_000)
        .await
        .expect("Download timed out");

    assert!(matches!(status.state, DownloadState::Completed { size } if size == TEST_FILE_SIZE));

    cleanup_file(&path).await;
}

/// Test: Single stream with wrong hash fails verification
#[tokio::test]
async fn test_single_stream_verification_fails() {
    let mock_server = setup_single_stream_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_single_wrong");

    let id = manager
        .add_download(
            DownloadOptions::new(format!("{}/test-file.bin", mock_server.uri()), path.clone())
                .with_segments(1)
                .with_verifier(create_wrong_verifier())
                .with_headers(default_test_headers())
                .with_retry(default_retry_scheme()),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 10_000)
        .await
        .expect("Download timed out");

    // Should fail verification
    assert!(
        matches!(status.state, DownloadState::Failed(f) if matches!(f.error, DownloadRunError::VerifyError { .. }))
    );

    cleanup_file(&path).await;
}
