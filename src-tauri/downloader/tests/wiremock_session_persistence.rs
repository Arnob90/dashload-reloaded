//! WireMock tests for session persistence (save/load).

use dashload_downloader::{
    download_data::DownloadData,
    download_data_factory::DownloadDataFactory,
    download_manager::DownloadManager,
    download_status::{DownloadState, DownloadStatus},
    request_headers::RequestHeaders,
    verifier::{HashAlgo, VerifierMaybe},
};
use std::path::PathBuf;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::fs;
use wiremock::MockServer;

mod wiremock_helpers;
use wiremock_helpers::*;

/// Test: Save session creates valid JSON that can be deserialized
#[tokio::test]
async fn test_save_session_creates_valid_json() {
    let mock_server = setup_dynamic_range_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client.clone(), 100);
    let path = temp_filepath("wiremock_session");

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

    // Let it run briefly
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Pause to get a clean snapshot
    manager.pause(&id).expect("Failed to pause");
    wait_for_state(&manager, &id, DownloadState::Paused, 5_000).await;

    // Save session to temp file
    let session_file = NamedTempFile::new().expect("Failed to create temp file");
    let session_path = session_file.path().to_path_buf();

    manager
        .save_session(&session_path)
        .await
        .expect("Failed to save session");

    // Verify file exists and is valid JSON
    let json_content = fs::read_to_string(&session_path)
        .await
        .expect("Failed to read session file");
    assert!(!json_content.is_empty());

    // Deserialize and verify structure
    let snapshot: std::collections::HashMap<String, DownloadData> =
        serde_json::from_str(&json_content).expect("Failed to deserialize session JSON");

    assert_eq!(snapshot.len(), 1);
    let saved_data = snapshot.values().next().unwrap();
    assert_eq!(
        saved_data.url,
        format!("{}/test-file.bin", mock_server.uri())
    );
    assert_eq!(saved_data.length, Some(TEST_FILE_SIZE));
    assert_eq!(saved_data.segments.len(), TEST_SEGMENTS as usize);

    // Verify segments have progress
    let total_downloaded: u64 = saved_data.segments.iter().map(|s| s.last_downloaded).sum();
    assert!(
        total_downloaded > 0,
        "Snapshot should have recorded progress"
    );

    cleanup_file(&path).await;
}

/// Test: Reconstruct manager from snapshot and resume download
#[tokio::test]
async fn test_reconstruct_resumes_from_snapshot() {
    let mock_server = setup_dynamic_range_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client.clone(), 100);
    let path = temp_filepath("wiremock_reconstruct");

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

    // Let it run
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Pause
    manager.pause(&id).expect("Failed to pause");
    wait_for_state(&manager, &id, DownloadState::Paused, 5_000).await;

    let downloaded_before = manager.get_item_status(&id).unwrap().downloaded;

    // Save session
    let session_file = NamedTempFile::new().expect("Failed to create temp file");
    let session_path = session_file.path().to_path_buf();
    manager
        .save_session(&session_path)
        .await
        .expect("Failed to save session");

    // Get snapshot
    let snapshot = manager.get_full_snapshot();

    // Create new manager and reconstruct
    let new_manager = DownloadManager::reconstruct(client.clone(), 100, snapshot)
        .await
        .expect("Failed to reconstruct manager");

    // Find the download in new manager
    let items = new_manager.get_all_items();
    assert_eq!(items.len(), 1);
    let new_id = items.keys().next().unwrap().clone();

    // Resume in new manager
    new_manager
        .resume(&new_id)
        .await
        .expect("Failed to resume after reconstruct");

    // Wait for running state
    let status = wait_for_state(
        &new_manager,
        &new_id,
        DownloadState::Running {
            download_speed: 0.0,
        },
        5_000,
    )
    .await;

    // Progress should not decrease
    assert!(
        status.downloaded >= downloaded_before,
        "Progress after reconstruct should not decrease (was {}, now {})",
        downloaded_before,
        status.downloaded
    );

    // Wait for completion
    let final_status = poll_until_completed(&new_manager, &new_id, 10_000)
        .await
        .expect("Download timed out");
    assert!(matches!(
        final_status.state,
        DownloadState::Completed { .. }
    ));

    // Verify file content
    let content = fs::read(&path)
        .await
        .expect("Failed to read downloaded file");
    assert_eq!(content.len(), TEST_FILE_SIZE as usize);
    assert_eq!(content, test_file_content());

    cleanup_file(&path).await;
}

/// Test: Save session with verifier preserves verifier config
#[tokio::test]
async fn test_save_session_preserves_verifier() {
    let mock_server = setup_dynamic_range_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client.clone(), 100);
    let path = temp_filepath("wiremock_session_verifier");

    let verifier = create_sha256_verifier();

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

    tokio::time::sleep(Duration::from_millis(300)).await;
    manager.pause(&id).expect("Failed to pause");
    wait_for_state(&manager, &id, DownloadState::Paused, 5_000).await;

    // Save session
    let session_file = NamedTempFile::new().expect("Failed to create temp file");
    let session_path = session_file.path().to_path_buf();
    manager
        .save_session(&session_path)
        .await
        .expect("Failed to save session");

    // Deserialize and check verifier
    let json_content = fs::read_to_string(&session_path)
        .await
        .expect("Failed to read session file");
    let snapshot: std::collections::HashMap<String, DownloadData> =
        serde_json::from_str(&json_content).expect("Failed to deserialize");

    let saved_data = snapshot.values().next().unwrap();
    assert!(
        saved_data.verifier.is_some(),
        "Verifier should be preserved in session"
    );
    assert_eq!(saved_data.verifier.algorithm(), Some(HashAlgo::Sha256));
    assert_eq!(saved_data.verifier.expected_hash(), Some(TEST_FILE_SHA256));

    cleanup_file(&path).await;
}

/// Test: Save session preserves request headers
#[tokio::test]
async fn test_save_session_preserves_headers() {
    let mock_server = setup_dynamic_range_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client.clone(), 100);
    let path = temp_filepath("wiremock_session_headers");

    let custom_headers = RequestHeaders {
        user_agent: Some("CustomAgent/1.0".to_string()),
        referrer: Some("https://custom.example.com/".to_string()),
        cookies: Some("session=abc123".to_string()),
    };

    let id = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path.clone(),
            TEST_SEGMENTS,
            VerifierMaybe::none(),
            custom_headers.clone(),
            default_retry_scheme(),
        )
        .await
        .expect("Failed to add download");

    tokio::time::sleep(Duration::from_millis(300)).await;
    manager.pause(&id).expect("Failed to pause");
    wait_for_state(&manager, &id, DownloadState::Paused, 5_000).await;

    // Save session
    let session_file = NamedTempFile::new().expect("Failed to create temp file");
    let session_path = session_file.path().to_path_buf();
    manager
        .save_session(&session_path)
        .await
        .expect("Failed to save session");

    // Deserialize and check headers
    let json_content = fs::read_to_string(&session_path)
        .await
        .expect("Failed to read session file");
    let snapshot: std::collections::HashMap<String, DownloadData> =
        serde_json::from_str(&json_content).expect("Failed to deserialize");

    let saved_data = snapshot.values().next().unwrap();
    // Check that all segments have the headers
    for segment in &saved_data.segments {
        assert_eq!(segment.headers.user_agent, custom_headers.user_agent);
        assert_eq!(segment.headers.referrer, custom_headers.referrer);
        assert_eq!(segment.headers.cookies, custom_headers.cookies);
    }

    cleanup_file(&path).await;
}

/// Test: Reconstruct with multiple downloads
#[tokio::test]
async fn test_reconstruct_multiple_downloads() {
    let mock_server = setup_dynamic_range_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client.clone(), 100);

    let path1 = temp_filepath("wiremock_multi_1");
    let path2 = temp_filepath("wiremock_multi_2");

    let id1 = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path1.clone(),
            TEST_SEGMENTS,
            VerifierMaybe::none(),
            default_test_headers(),
            default_retry_scheme(),
        )
        .await
        .expect("Failed to add download 1");

    let id2 = manager
        .add_download(
            format!("{}/test-file.bin", mock_server.uri()),
            path2.clone(),
            TEST_SEGMENTS,
            VerifierMaybe::none(),
            default_test_headers(),
            default_retry_scheme(),
        )
        .await
        .expect("Failed to add download 2");

    // Let them run
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Pause both
    manager.pause(&id1).expect("Failed to pause 1");
    manager.pause(&id2).expect("Failed to pause 2");
    wait_for_state(&manager, &id1, DownloadState::Paused, 5_000).await;
    wait_for_state(&manager, &id2, DownloadState::Paused, 5_000).await;

    // Save session
    let session_file = NamedTempFile::new().expect("Failed to create temp file");
    let session_path = session_file.path().to_path_buf();
    manager
        .save_session(&session_path)
        .await
        .expect("Failed to save session");

    // Reconstruct
    let snapshot = manager.get_full_snapshot();
    let new_manager = DownloadManager::reconstruct(client.clone(), 100, snapshot)
        .await
        .expect("Failed to reconstruct");

    // Both downloads should be present
    let items = new_manager.get_all_items();
    assert_eq!(items.len(), 2);

    // Collect IDs first to avoid move issues
    let ids: Vec<String> = items.keys().cloned().collect();

    // Resume both
    for new_id in &ids {
        new_manager.resume(new_id).await.expect("Failed to resume");
    }

    // Wait for both to complete
    for new_id in &ids {
        let status = poll_until_completed(&new_manager, new_id, 15_000)
            .await
            .expect("Download timed out");
        assert!(matches!(status.state, DownloadState::Completed { .. }));
    }

    cleanup_file(&path1).await;
    cleanup_file(&path2).await;
}

/// Test: Round-trip with DownloadDataFactory deserialize_datas
#[tokio::test]
async fn test_deserialize_datas_roundtrip() {
    let mock_server = setup_dynamic_range_mock_server().await;
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 100);
    let path = temp_filepath("wiremock_deserialize");

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

    tokio::time::sleep(Duration::from_millis(300)).await;
    manager.pause(&id).expect("Failed to pause");
    wait_for_state(&manager, &id, DownloadState::Paused, 5_000).await;

    // Save session using manager's save_session
    let session_file = NamedTempFile::new().expect("Failed to create temp file");
    let session_path = session_file.path().to_path_buf();
    manager
        .save_session(&session_path)
        .await
        .expect("Failed to save session");

    // Now use DownloadDataFactory::deserialize_datas to load it
    let datas = DownloadDataFactory::deserialize_datas(&session_path)
        .await
        .expect("Failed to deserialize datas");

    assert_eq!(datas.len(), 1);
    let data = datas.values().next().unwrap();
    assert_eq!(data.verifier.algorithm(), Some(HashAlgo::Sha256));
    assert_eq!(data.segments.len(), TEST_SEGMENTS as usize);

    // Reconstruct from deserialized datas
    let client2 = reqwest::Client::new();
    let new_manager = DownloadManager::reconstruct(client2, 100, datas)
        .await
        .expect("Failed to reconstruct from deserialized");

    let items = new_manager.get_all_items();
    assert_eq!(items.len(), 1);
    let new_id = items.keys().next().unwrap().clone();

    new_manager.resume(&new_id).await.expect("Failed to resume");
    let final_status = poll_until_completed(&new_manager, &new_id, 10_000)
        .await
        .expect("Download timed out");
    assert!(matches!(
        final_status.state,
        DownloadState::Completed { .. }
    ));

    cleanup_file(&path).await;
}

/// Helper to wait for a specific download state
async fn wait_for_state(
    manager: &DownloadManager,
    id: &str,
    target_state: DownloadState,
    timeout_ms: u64,
) -> DownloadStatus {
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    loop {
        if start.elapsed() > timeout {
            panic!("Timeout waiting for state {:?}", target_state);
        }

        let status = manager.get_item_status(id).expect("Failed to get status");

        let matches = match (&status.state, &target_state) {
            (DownloadState::Paused, DownloadState::Paused) => true,
            (DownloadState::Stopped, DownloadState::Stopped) => true,
            (DownloadState::Completed { .. }, DownloadState::Completed { .. }) => true,
            (DownloadState::Running { .. }, DownloadState::Running { .. }) => true,
            (DownloadState::Failed(_), DownloadState::Failed(_)) => true,
            (DownloadState::Verifying, DownloadState::Verifying) => true,
            _ => false,
        };

        if matches {
            return status;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

