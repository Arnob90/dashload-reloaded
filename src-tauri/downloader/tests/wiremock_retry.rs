//! WireMock tests for the `DownloadManager::retry` lifecycle.

use dashload_downloader::download_options::DownloadOptions;

use dashload_downloader::{
    download_manager::DownloadManager,
    download_status::DownloadState,
    verifier::VerifierMaybe,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::time::timeout;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

mod wiremock_helpers;
use wiremock_helpers::*;

const SMALL_FILE_SIZE: usize = 64 * 1024;

/// Returns HTTP 500 on the first body request, then serves `content`.
struct FailFirstGetResponder {
    content: Vec<u8>,
    calls: Arc<AtomicUsize>,
}

impl Respond for FailFirstGetResponder {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            ResponseTemplate::new(500)
        } else {
            ResponseTemplate::new(200)
                .insert_header("Content-Length", self.content.len().to_string())
                .set_body_bytes(self.content.clone())
        }
    }
}

async fn setup_fail_first_mock_server() -> MockServer {
    let server = MockServer::start().await;
    let content = vec![0x42u8; SMALL_FILE_SIZE];
    let calls = Arc::new(AtomicUsize::new(0));

    Mock::given(method("HEAD"))
        .and(path("/test-file.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Length", SMALL_FILE_SIZE.to_string()),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/test-file.bin"))
        .respond_with(FailFirstGetResponder { content, calls })
        .mount(&server)
        .await;

    server
}

#[tokio::test]
async fn retry_stopped_download_reuses_id_and_restarts() {
    let server = start_universal_mock_server(SMALL_FILE_SIZE, Duration::from_secs(2)).await;
    let manager = DownloadManager::new(reqwest::Client::new(), 50);
    let path = temp_filepath("retry_stopped");

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

    tokio::time::sleep(Duration::from_millis(100)).await;
    timeout(Duration::from_secs(15), manager.stop(&id))
        .await
        .expect("stop timed out")
        .expect("stop failed");

    assert_eq!(manager.download_items.len(), 1);
    assert!(manager.download_items.contains_key(&id));
    assert!(
        matches!(
            manager.get_item_status(&id).unwrap().state,
            DownloadState::Stopped
        ),
        "expected Stopped after stop"
    );

    let retried = timeout(Duration::from_secs(10), manager.retry(&id))
        .await
        .expect("retry timed out")
        .expect("retry failed");
    assert_eq!(retried, id);
    assert!(manager.download_items.contains_key(&id));
    assert_eq!(manager.download_items.len(), 1);

    let final_status = timeout(Duration::from_secs(15), poll_until_completed(&manager, &id, 14_000))
        .await
        .expect("poll timed out")
        .expect("download timed out");
    assert!(
        matches!(final_status.state, DownloadState::Completed { .. }),
        "unexpected final state: {:?}",
        final_status.state
    );
    assert_eq!(final_status.downloaded, SMALL_FILE_SIZE as u64);

    let content = tokio::fs::read(&path).await.expect("read downloaded file");
    assert_eq!(content, vec![0x42u8; SMALL_FILE_SIZE]);

    cleanup_file(&path).await;
}

#[tokio::test]
async fn retry_after_failure_recreates_file() {
    let server = setup_fail_first_mock_server().await;
    let manager = DownloadManager::new(reqwest::Client::new(), 50);
    let path = temp_filepath("retry_failed");

    let id = manager
        .add_download(
            DownloadOptions::new(format!("{}/test-file.bin", server.uri()), path.clone())
                .with_segments(1)
                .with_verifier(VerifierMaybe::none())
                .with_headers(default_test_headers())
                // Fail on the very first attempt so the item reaches `Failed`.
                .with_retry(no_retry_scheme()),
        )
        .await
        .expect("add_download failed");

    let failed = timeout(Duration::from_secs(15), poll_until_completed(&manager, &id, 14_000))
        .await
        .expect("poll timed out")
        .expect("download timed out");
    assert!(
        matches!(failed.state, DownloadState::Failed(_)),
        "expected Failed, got {:?}",
        failed.state
    );

    let retried = timeout(Duration::from_secs(10), manager.retry(&id))
        .await
        .expect("retry timed out")
        .expect("retry failed");
    assert_eq!(retried, id);

    let final_status = timeout(Duration::from_secs(15), poll_until_completed(&manager, &id, 14_000))
        .await
        .expect("poll timed out")
        .expect("download timed out");
    assert!(
        matches!(final_status.state, DownloadState::Completed { .. }),
        "unexpected final state: {:?}",
        final_status.state
    );

    let content = tokio::fs::read(&path).await.expect("read downloaded file");
    assert_eq!(content, vec![0x42u8; SMALL_FILE_SIZE]);

    cleanup_file(&path).await;
}

#[tokio::test]
async fn retry_does_not_delete_new_file() {
    timeout(Duration::from_secs(20), async {
        let server = start_universal_mock_server(SMALL_FILE_SIZE, Duration::from_secs(2)).await;
        let manager = DownloadManager::new(reqwest::Client::new(), 50);
        let path = temp_filepath("retry_keep_file");

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

        tokio::time::sleep(Duration::from_millis(100)).await;

        // `stop` deletes the partial file and awaits the old runner; a following
        // retry must recreate it and must not have it unlinked by stale cleanup.
        manager.stop(&id).await.expect("stop failed");
        manager.retry(&id).await.expect("retry failed");
        assert!(
            path.exists(),
            "the retried download should have recreated its file"
        );

        let final_status = poll_until_completed(&manager, &id, 15_000)
            .await
            .expect("download timed out");
        assert!(matches!(final_status.state, DownloadState::Completed { .. }));
        assert!(path.exists(), "file disappeared after retry completed");
        assert_eq!(
            tokio::fs::read(&path).await.expect("read file"),
            vec![0x42u8; SMALL_FILE_SIZE]
        );

        cleanup_file(&path).await;
    })
    .await
    .expect("test timed out (possible deadlock)");
}
