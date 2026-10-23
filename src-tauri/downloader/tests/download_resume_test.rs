use downloader::download_manager::DownloadManager;
use downloader::download_status::DownloadState;
use downloader::request_headers::RequestHeaders;
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

/// Helper for sensible default headers (Tor Browser / Mozilla User-Agent)
fn default_test_headers() -> RequestHeaders {
    RequestHeaders {
        user_agent: Some(
            "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0".to_string(),
        ),
        referrer: Some("https://linuxmint.com/".to_string()),
        cookies: None,
    }
}

/// Test 1: Segmented Download Progress (N = 4)
#[test_log::test(tokio::test)]
async fn test_segmented_download_progress() {
    let client = reqwest::Client::new();
    let manager = DownloadManager::new(client, 200);
    let path = temp_filepath("mint_segmented");

    // Pass VerifierMaybe::none() and default request headers
    let id = manager
        .add_download(
            TEST_URL.to_string(),
            path.clone(),
            4,
            VerifierMaybe::none(),
            default_test_headers(), // 👈 5th argument added
        )
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

    let id = manager
        .add_download(
            TEST_URL.to_string(),
            path.clone(),
            4,
            VerifierMaybe::none(),
            default_test_headers(),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(1500)).await;

    // 1. First pause
    let first_pause_started = tokio::time::Instant::now();
    manager.pause(&id).unwrap();
    while manager.get_item_status(&id).unwrap().state != DownloadState::Paused {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let downloaded_at_pause = manager.get_item_status(&id).unwrap().downloaded;
    let first_pause_elapsed = first_pause_started.elapsed();
    log::info!("First pause took {:?}", first_pause_elapsed);
    // 2. Resume
    manager.resume(&id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let status_after_resume = manager.get_item_status(&id).unwrap();
    log::info!("Status after resume: {:?}", status_after_resume.state);

    assert!(matches!(
        status_after_resume.state,
        DownloadState::Running { .. }
    ));
    assert!(
        status_after_resume.downloaded >= downloaded_at_pause,
        "Downloaded bytes should resume from paused progress"
    );

    // 3. Second pause — measure the time spent inside pause()
    let second_pause_started = tokio::time::Instant::now();
    manager.pause(&id).unwrap();
    while manager.get_item_status(&id).unwrap().state != DownloadState::Paused {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let second_pause_elapsed = second_pause_started.elapsed();

    log::info!("Second pause took {:?}", second_pause_elapsed);
    let status_after_second_pause = manager.get_item_status(&id).unwrap();
    log::info!(
        "Status after second pause: {:?}",
        status_after_second_pause.state
    );
    assert_eq!(status_after_second_pause.state, DownloadState::Paused);

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

    // 1 segment (N = 1) with VerifierMaybe::none() and request headers
    let id = manager
        .add_download(
            TEST_URL.to_string(),
            path.clone(),
            1,
            VerifierMaybe::none(),
            default_test_headers(),
        )
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
