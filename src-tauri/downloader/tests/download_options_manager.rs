//! Manager-level tests for `DownloadOptions` IDs and the retry/resume guards.

use dashload_downloader::download_options::DownloadOptions;

use dashload_downloader::{
    download_manager::{DownloadManager, ManagerError},
    download_status::DownloadState,
    verifier::VerifierMaybe,
};
use std::time::Duration;
use tokio::time::timeout;

mod wiremock_helpers;
use wiremock_helpers::*;

async fn wait_for_running(manager: &DownloadManager, id: &str) -> DownloadState {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let status = manager.get_item_status(id).expect("status");
        if matches!(status.state, DownloadState::Running { .. }) {
            return status.state;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for Running, last state: {:?}",
            status.state
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn add_download_preserves_explicit_id() {
    let server = start_universal_mock_server(1024, Duration::ZERO).await;
    let manager = DownloadManager::new(reqwest::Client::new(), 50);
    let path = temp_filepath("explicit_id");

    let id = timeout(
        Duration::from_secs(10),
        manager.add_download(
            DownloadOptions::new(format!("{}/test-file.bin", server.uri()), path.clone())
                .with_segments(1)
                .with_verifier(VerifierMaybe::none())
                .with_headers(default_test_headers())
                .with_retry(default_retry_scheme())
                .with_id("chosen-id"),
        ),
    )
    .await
    .expect("add_download timed out")
    .expect("add_download failed");

    assert_eq!(id, "chosen-id");
    assert!(manager.download_items.contains_key("chosen-id"));
    assert!(manager.runner_handles.contains_key("chosen-id"));

    let _ = timeout(Duration::from_secs(10), manager.stop(&id)).await;
    cleanup_file(&path).await;
}

#[tokio::test]
async fn add_download_generates_uuid_when_id_is_none() {
    let server = start_universal_mock_server(1024, Duration::ZERO).await;
    let manager = DownloadManager::new(reqwest::Client::new(), 50);
    let path_a = temp_filepath("generated_id_a");
    let path_b = temp_filepath("generated_id_b");

    let make = |path: std::path::PathBuf| {
        DownloadOptions::new(format!("{}/test-file.bin", server.uri()), path)
            .with_segments(1)
            .with_verifier(VerifierMaybe::none())
            .with_headers(default_test_headers())
            .with_retry(default_retry_scheme())
    };

    let id_a = timeout(Duration::from_secs(10), manager.add_download(make(path_a.clone())))
        .await
        .expect("first add timed out")
        .expect("first add failed");
    let id_b = timeout(Duration::from_secs(10), manager.add_download(make(path_b.clone())))
        .await
        .expect("second add timed out")
        .expect("second add failed");

    assert!(!id_a.is_empty());
    assert!(!id_b.is_empty());
    assert_ne!(id_a, id_b);
    assert!(manager.download_items.contains_key(&id_a));
    assert!(manager.download_items.contains_key(&id_b));

    let _ = timeout(Duration::from_secs(10), manager.stop(&id_a)).await;
    let _ = timeout(Duration::from_secs(10), manager.stop(&id_b)).await;
    cleanup_file(&path_a).await;
    cleanup_file(&path_b).await;
}

#[tokio::test]
async fn retry_unknown_id_returns_id_does_not_exist() {
    let manager = DownloadManager::new(reqwest::Client::new(), 50);

    let err = timeout(Duration::from_secs(5), manager.retry("missing"))
        .await
        .expect("retry timed out")
        .expect_err("retry on unknown id should fail");

    assert!(matches!(err, ManagerError::IdDoesNotExist));
}

#[tokio::test]
async fn retry_rejects_running_state() {
    let server = start_universal_mock_server(1024 * 1024, Duration::from_secs(2)).await;
    let manager = DownloadManager::new(reqwest::Client::new(), 50);
    let path = temp_filepath("retry_running");

    let id = manager
        .add_download(
            DownloadOptions::new(format!("{}/test-file.bin", server.uri()), path.clone())
                .with_segments(1)
                .with_verifier(VerifierMaybe::none())
                .with_headers(default_test_headers())
                .with_retry(default_retry_scheme()),
        )
        .await
        .expect("add_download failed");

    wait_for_running(&manager, &id).await;

    let err = timeout(Duration::from_secs(5), manager.retry(&id))
        .await
        .expect("retry timed out")
        .expect_err("retry on a running item should fail");
    assert!(matches!(err, ManagerError::RetryError(DownloadState::Running { .. })));

    let _ = timeout(Duration::from_secs(15), manager.stop(&id)).await;
    cleanup_file(&path).await;
}

#[tokio::test]
async fn retry_rejects_completed_state() {
    let server = start_universal_mock_server(1024, Duration::ZERO).await;
    let manager = DownloadManager::new(reqwest::Client::new(), 50);
    let path = temp_filepath("retry_completed");

    let id = manager
        .add_download(
            DownloadOptions::new(format!("{}/test-file.bin", server.uri()), path.clone())
                .with_segments(1)
                .with_verifier(VerifierMaybe::none())
                .with_headers(default_test_headers())
                .with_retry(default_retry_scheme()),
        )
        .await
        .expect("add_download failed");

    let status = timeout(Duration::from_secs(10), poll_until_completed(&manager, &id, 9_000))
        .await
        .expect("poll timed out")
        .expect("download timed out");
    assert!(matches!(status.state, DownloadState::Completed { .. }));

    let err = timeout(Duration::from_secs(5), manager.retry(&id))
        .await
        .expect("retry timed out")
        .expect_err("retry on a completed item should fail");
    assert!(matches!(err, ManagerError::RetryError(DownloadState::Completed { .. })));

    cleanup_file(&path).await;
}

#[tokio::test]
async fn resume_running_is_noop() {
    let server = start_universal_mock_server(1024 * 1024, Duration::from_secs(2)).await;
    let manager = DownloadManager::new(reqwest::Client::new(), 50);
    let path = temp_filepath("resume_running");

    let id = manager
        .add_download(
            DownloadOptions::new(format!("{}/test-file.bin", server.uri()), path.clone())
                .with_segments(1)
                .with_verifier(VerifierMaybe::none())
                .with_headers(default_test_headers())
                .with_retry(default_retry_scheme()),
        )
        .await
        .expect("add_download failed");

    wait_for_running(&manager, &id).await;

    timeout(Duration::from_secs(5), manager.resume(&id))
        .await
        .expect("first resume timed out")
        .expect("resume on running item should be Ok");
    timeout(Duration::from_secs(5), manager.resume(&id))
        .await
        .expect("second resume timed out")
        .expect("second resume on running item should be Ok");

    assert!(matches!(
        manager.get_item_status(&id).unwrap().state,
        DownloadState::Running { .. }
    ));
    assert_eq!(manager.runner_handles.len(), 1);
    assert!(manager.runner_handles.contains_key(&id));

    let _ = timeout(Duration::from_secs(15), manager.stop(&id)).await;
    cleanup_file(&path).await;
}

#[tokio::test]
async fn resume_rejects_completed() {
    let server = start_universal_mock_server(1024, Duration::ZERO).await;
    let manager = DownloadManager::new(reqwest::Client::new(), 50);
    let path = temp_filepath("resume_completed");

    let id = manager
        .add_download(
            DownloadOptions::new(format!("{}/test-file.bin", server.uri()), path.clone())
                .with_segments(1)
                .with_verifier(VerifierMaybe::none())
                .with_headers(default_test_headers())
                .with_retry(default_retry_scheme()),
        )
        .await
        .expect("add_download failed");

    let status = timeout(Duration::from_secs(10), poll_until_completed(&manager, &id, 9_000))
        .await
        .expect("poll timed out")
        .expect("download timed out");
    assert!(matches!(status.state, DownloadState::Completed { .. }));

    let err = timeout(Duration::from_secs(5), manager.resume(&id))
        .await
        .expect("resume timed out")
        .expect_err("resume on a completed item should fail");
    assert!(matches!(err, ManagerError::ResumeError(DownloadState::Completed { .. })));

    cleanup_file(&path).await;
}
