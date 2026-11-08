//! Concurrency regressions for the per-download operation lock and runner lifecycle.
//!
//! Every test is wrapped in `tokio::time::timeout` so that a regression to a
//! deadlock fails instead of hanging CI.

use dashload_downloader::download_options::DownloadOptions;

use dashload_downloader::{
    download_manager::{DownloadManager, ManagerError},
    download_status::DownloadState,
    verifier::VerifierMaybe,
};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;
use wiremock::MockServer;

mod wiremock_helpers;
use wiremock_helpers::*;

const SMALL_FILE_SIZE: usize = 64 * 1024;

async fn add_slow_download(manager: &DownloadManager, server: &MockServer, path: &PathBuf) -> String {
    manager
        .add_download(
            DownloadOptions::new(format!("{}/test-file.bin", server.uri()), path.clone())
                .with_segments(1)
                .with_verifier(VerifierMaybe::none())
                .with_headers(default_test_headers())
                .with_retry(default_retry_scheme()),
        )
        .await
        .expect("add_download failed")
}

async fn add_and_stop(
    manager: &DownloadManager,
    server: &MockServer,
    path: &PathBuf,
) -> String {
    let id = add_slow_download(manager, server, path).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    timeout(Duration::from_secs(15), manager.stop(&id))
        .await
        .expect("stop timed out")
        .expect("stop failed");
    assert!(matches!(
        manager.get_item_status(&id).unwrap().state,
        DownloadState::Stopped
    ));
    id
}

#[tokio::test]
async fn concurrent_retry_calls_spawn_one_runner() {
    timeout(Duration::from_secs(25), async {
        let server = start_universal_mock_server(SMALL_FILE_SIZE, Duration::from_secs(2)).await;
        let manager = DownloadManager::new(reqwest::Client::new(), 50);
        let path = temp_filepath("concurrent_retry");

        let id = add_and_stop(&manager, &server, &path).await;

        let (first, second) = tokio::join!(
            timeout(Duration::from_secs(10), manager.retry(&id)),
            timeout(Duration::from_secs(10), manager.retry(&id)),
        );
        let first = first.expect("first retry timed out");
        let second = second.expect("second retry timed out");

        let successes = [&first, &second]
            .iter()
            .filter(|result| matches!(result, Ok(retried) if retried == &id))
            .count();
        let failures = [&first, &second]
            .iter()
            .filter(|result| result.is_err())
            .count();
        assert_eq!(successes, 1, "expected exactly one successful retry");
        assert_eq!(failures, 1, "expected exactly one rejected retry");

        // The loser should observe the item having already been replaced with a
        // running one, and must not have spawned a second runner.
        let loser = if first.is_err() { &first } else { &second };
        assert!(
            matches!(
                loser,
                Err(ManagerError::RetryError(DownloadState::Running { .. }))
            ),
            "unexpected losing retry result: {loser:?}"
        );

        assert_eq!(manager.runner_handles.len(), 1);
        assert!(manager.runner_handles.contains_key(&id));

        let final_status = timeout(
            Duration::from_secs(15),
            poll_until_completed(&manager, &id, 14_000),
        )
        .await
        .expect("poll timed out")
        .expect("download timed out");
        assert!(matches!(final_status.state, DownloadState::Completed { .. }));

        cleanup_file(&path).await;
    })
    .await
    .expect("test timed out (possible deadlock)");
}

#[tokio::test]
async fn stop_then_concurrent_retry_keeps_file() {
    timeout(Duration::from_secs(25), async {
        let server = start_universal_mock_server(SMALL_FILE_SIZE, Duration::from_secs(2)).await;
        let manager = DownloadManager::new(reqwest::Client::new(), 50);
        let path = temp_filepath("stop_retry_concurrent");

        let id = add_slow_download(&manager, &server, &path).await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        let (stop_result, retry_result) = tokio::join!(
            timeout(Duration::from_secs(15), manager.stop(&id)),
            timeout(Duration::from_secs(15), manager.retry(&id)),
        );
        stop_result
            .expect("stop timed out")
            .expect("stop failed");
        let retry_result = retry_result.expect("retry timed out");

        match retry_result {
            Ok(retried) => {
                assert_eq!(retried, id);
                let final_status = timeout(
                    Duration::from_secs(15),
                    poll_until_completed(&manager, &id, 14_000),
                )
                .await
                .expect("poll timed out")
                .expect("download timed out");
                assert!(
                    matches!(final_status.state, DownloadState::Completed { .. }),
                    "unexpected final state: {:?}",
                    final_status.state
                );
                assert_eq!(
                    tokio::fs::read(&path).await.expect("read file"),
                    vec![0x42u8; SMALL_FILE_SIZE]
                );
            }
            Err(ManagerError::RetryError(DownloadState::Running { .. })) => {
                // The retry lost the race and observed the still-running item;
                // the concurrent stop then left a clean Stopped item and no file.
                assert!(matches!(
                    manager.get_item_status(&id).unwrap().state,
                    DownloadState::Stopped
                ));
                assert!(!path.exists(), "stopped download should have removed its file");
            }
            other => panic!("unexpected retry result: {other:?}"),
        }

        cleanup_file(&path).await;
    })
    .await
    .expect("test timed out (possible deadlock)");
}

#[tokio::test]
async fn stop_and_resume_do_not_deadlock() {
    timeout(Duration::from_secs(25), async {
        let server = start_universal_mock_server(SMALL_FILE_SIZE, Duration::from_secs(2)).await;
        let manager = DownloadManager::new(reqwest::Client::new(), 50);
        let path = temp_filepath("stop_resume_concurrent");

        let id = add_slow_download(&manager, &server, &path).await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        let (stop_result, resume_result) = tokio::join!(
            timeout(Duration::from_secs(15), manager.stop(&id)),
            timeout(Duration::from_secs(15), manager.resume(&id)),
        );
        stop_result
            .expect("stop timed out")
            .expect("stop failed");
        let resume_result = resume_result.expect("resume timed out");

        // Either ordering is acceptable: resume may be a no-op on the running
        // item, or it may reject the already-stopped item.
        match resume_result {
            Ok(()) => {}
            Err(ManagerError::ResumeError(DownloadState::Stopped)) => {}
            other => panic!("unexpected resume result: {other:?}"),
        }

        assert!(matches!(
            manager.get_item_status(&id).unwrap().state,
            DownloadState::Stopped
        ));
        assert!(!manager.runner_handles.contains_key(&id));

        cleanup_file(&path).await;
    })
    .await
    .expect("test timed out (possible deadlock)");
}
