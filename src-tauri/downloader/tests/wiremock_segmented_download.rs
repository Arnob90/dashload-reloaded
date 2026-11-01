//! WireMock tests for segmented (multi-chunk) downloads.

use wiremock::MockServer;
use dashload_downloader::{
    download_manager::DownloadManager,
    download_status::{DownloadState, DownloadStatus},
    download_run_error::DownloadRunError,
    request_headers::RequestHeaders,
    verifier::{VerifierMaybe, HashAlgo},
};
use std::path::PathBuf;
use std::time::Duration;

mod wiremock_helpers;
use wiremock_helpers::*;

/// Test: Segmented download completes successfully with 4 segments
#[tokio::test]
async fn test_segmented_download_completes() {
    let mock_server = setup_segmented_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_segmented");

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            TEST_SEGMENTS,
            VerifierMaybe::none(),
            default_test_headers(),
            default_retry_scheme(),
        )
        .await
        .expect("Failed to add download");

    // Poll for completion
    let status = poll_until_completed(&manager, &id, 10_000).await.expect("Download timed out");

    assert_eq!(status.downloaded, TEST_FILE_SIZE);
    assert_eq!(status.total, Some(TEST_FILE_SIZE));
    assert!(matches!(status.state, DownloadState::Completed { size } if size == TEST_FILE_SIZE));

    // Verify file content
    let content = tokio::fs::read(&path).await.expect("Failed to read downloaded file");
    assert_eq!(content.len(), TEST_FILE_SIZE as usize);
    assert_eq!(content, test_file_content());

    cleanup_file(&path).await;
}

/// Test: Segmented download with SHA256 verification passes
#[tokio::test]
async fn test_segmented_download_with_verifier() {
    let mock_server = setup_segmented_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_verified");

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            TEST_SEGMENTS,
            create_sha256_verifier(),
            default_test_headers(),
            default_retry_scheme(),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 10_000).await.expect("Download timed out");

    assert!(matches!(status.state, DownloadState::Completed { size } if size == TEST_FILE_SIZE));

    // Verify file content
    let content = tokio::fs::read(&path).await.expect("Failed to read downloaded file");
    assert_eq!(content, test_file_content());

    cleanup_file(&path).await;
}

/// Test: Segmented download with wrong hash fails verification
#[tokio::test]
async fn test_segmented_download_verification_fails() {
    let mock_server = setup_segmented_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_wrong_hash");

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            TEST_SEGMENTS,
            create_wrong_verifier(),
            default_test_headers(),
            default_retry_scheme(),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 10_000).await.expect("Download timed out");

    // Should fail verification
    assert!(matches!(status.state, DownloadState::Failed(e) if matches!(e, dashload_downloader::download_run_error::DownloadRunError::VerifyError)));

    cleanup_file(&path).await;
}

/// Test: Segmented download with SHA512 verification
#[tokio::test]
async fn test_segmented_download_sha512_verifier() {
    let mock_server = setup_segmented_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_sha512");

    let verifier = VerifierMaybe::new(HashAlgo::Sha512, TEST_FILE_SHA512.to_string());

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            TEST_SEGMENTS,
            verifier,
            default_test_headers(),
            default_retry_scheme(),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 10_000).await.expect("Download timed out");

    assert!(matches!(status.state, DownloadState::Completed { size } if size == TEST_FILE_SIZE));

    cleanup_file(&path).await;
}

/// Test: Segmented download with MD5 verification
#[tokio::test]
async fn test_segmented_download_md5_verifier() {
    let mock_server = setup_segmented_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_md5");

    let verifier = VerifierMaybe::new(HashAlgo::Md5, TEST_FILE_MD5.to_string());

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            TEST_SEGMENTS,
            verifier,
            default_test_headers(),
            default_retry_scheme(),
        )
        .await
        .expect("Failed to add download");

    let status = poll_until_completed(&manager, &id, 10_000).await.expect("Download timed out");

    assert!(matches!(status.state, DownloadState::Completed { size } if size == TEST_FILE_SIZE));

    cleanup_file(&path).await;
}