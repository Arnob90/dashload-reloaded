use downloader::download_manager::DownloadManager;
use downloader::download_status::DownloadState;
use downloader::verifier::VerifierMaybe;
use std::path::PathBuf;
use std::time::Duration;

const TEST_URL: &str =
    "https://mirror.xeonbd.com/linuxmint.com/stable/22.2/linuxmint-22.2-xfce-64bit.iso";

/// Helper to generate unique filepaths so test runs don't collide
fn temp_filepath(prefix: &str) -> PathBuf {
    let filename = format!("{}_{}.iso", prefix, uuid::Uuid::new_v4().as_simple());
    std::env::temp_dir().join(filename)
}

/// Test 1: Segmented Download Progress (N = 4)
#[test_log::test(tokio::test)]
async fn test_segmented_download_progress() {
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 200);
    let path = temp_filepath("mint_segmented");

    // Pass VerifierMaybe::none() (or VerifierMaybe { verifier: None })
    let id = manager
        .add_download(TEST_URL.to_string(), path.clone(), 4, VerifierMaybe::none())
        .await
        .unwrap();

    // Monitor for 2 seconds to confirm progress
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let status = manager.get_item_status(&id).unwrap();
    log::info!(
        "Segmented Progress: {} / {:?}",
        status.downloaded,
        status.total
    );

    assert!(
        status.downloaded > 0,
        "Downloaded bytes should be greater than 0"
    );
    assert!(matches!(status.state, DownloadState::Running { .. }));

    // Clean up
    let _ = manager.pause(&id);
    let _ = tokio::fs::remove_file(&path).await;
}

/// Test 2: Pause and Resume Lifecycle
#[test_log::test(tokio::test)]
async fn test_pause_and_resume_lifecycle() {
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 200);
    let path = temp_filepath("mint_resume");

    // 1. Start download with VerifierMaybe::none()
    let id = manager
        .add_download(TEST_URL.to_string(), path.clone(), 4, VerifierMaybe::none())
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(1500)).await;

    // 2. Pause download (Synchronous call)
    manager.pause(&id).unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let status_after_pause = manager.get_item_status(&id).unwrap();
    log::info!("Status after pause: {:?}", status_after_pause.state);
    assert_eq!(status_after_pause.state, DownloadState::Paused);

    let downloaded_at_pause = status_after_pause.downloaded;

    // 3. Resume download (Async call)
    manager.resume(&id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let status_after_resume = manager.get_item_status(&id).unwrap();
    log::info!("Status after resume: {:?}", status_after_resume.state);

    // Verify state is Running again and downloaded bytes continued from offset
    assert!(matches!(
        status_after_resume.state,
        DownloadState::Running { .. }
    ));
    assert!(
        status_after_resume.downloaded >= downloaded_at_pause,
        "Downloaded bytes should resume from paused progress"
    );

    // Clean up
    let _ = manager.pause(&id);
    let _ = tokio::fs::remove_file(&path).await;
}

/// Test 3: Single Stream Fallback (N = 1)
#[test_log::test(tokio::test)]
async fn test_single_stream_download() {
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 200);
    let path = temp_filepath("mint_single");

    // 1 segment (N = 1) with VerifierMaybe::none()
    let id = manager
        .add_download(TEST_URL.to_string(), path.clone(), 1, VerifierMaybe::none())
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(1500)).await;

    let status = manager.get_item_status(&id).unwrap();
    log::info!("Single stream progress: {} bytes", status.downloaded);
    assert!(status.downloaded > 0);

    // Clean up
    let _ = manager.pause(&id);
    let _ = tokio::fs::remove_file(&path).await;
}
