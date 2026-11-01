use dashload_downloader::download_manager::DownloadManager;
use dashload_downloader::download_status::DownloadState;
use dashload_downloader::request_headers::RequestHeaders;
use dashload_downloader::retry_scheme::RetryScheme;
use dashload_downloader::verifier::{HashAlgo, VerifierMaybe};
use log::info;
use std::time::Duration;

#[test_log::test(tokio::test)]
#[ignore = "Live Network Test"]
async fn test_alpine_download_and_verify_sha256() {
    let download_url = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.24.1-x86_64.tar.gz";
    let sha256_url = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.24.1-x86_64.tar.gz.sha256";

    // Unique temporary path
    let temp_dir = std::env::temp_dir();
    let target_path = temp_dir.join(format!(
        "alpine_test_{}.tar.gz",
        uuid::Uuid::new_v4().as_simple()
    ));

    let client = reqwest::Client::new();
    let poll_every_ms = 200;
    let download_manager = DownloadManager::new(client.clone(), poll_every_ms);

    // 1. Fetch official expected SHA256 checksum string from Alpine CDN
    let sha256_response = client
        .get(sha256_url)
        .send()
        .await
        .expect("Failed to fetch official SHA256 file")
        .text()
        .await
        .expect("Failed to read SHA256 response text");

    let expected_hash_str = sha256_response
        .split_whitespace()
        .next()
        .expect("Invalid .sha256 file format");

    info!("Official expected SHA256: {}", expected_hash_str);

    // 2. Build the verifier directly for the download engine
    let verifier = VerifierMaybe::new(HashAlgo::Sha256, expected_hash_str.to_string());

    // 3. Configure headers with the Tor Browser / Firefox ESR User-Agent
    let request_headers = RequestHeaders {
        user_agent: Some(
            "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0".to_string(),
        ),
        referrer: Some("https://alpinelinux.org/downloads/".to_string()),
        cookies: None,
    };

    info!("Starting Alpine minirootfs download to {:?}", target_path);

    // 4. Add download with 4 segments, verifier, and custom headers
    let id = download_manager
        .add_download(
            download_url.to_string(),
            target_path.clone(),
            4,
            verifier,
            request_headers, // 👈 Passed request headers here
            RetryScheme::default(),
        )
        .await
        .expect("Failed to add download");

    // 5. Poll until the engine finishes downloading and automatically verifies the checksum
    loop {
        let status = download_manager
            .get_item_status(&id)
            .expect("Failed to get item status");

        match status.state {
            DownloadState::Completed { size } => {
                info!(
                    "Download completed and verified by engine! Final size: {} bytes",
                    size
                );
                break;
            }
            DownloadState::Failed(err) => {
                panic!("Download or checksum verification failed: {:?}", err);
            }
            DownloadState::Running { download_speed } => {
                info!(
                    "Progress: {} / {:?} bytes | Speed: {:.2} B/s",
                    status.downloaded, status.total, download_speed
                );
            }
            _ => {}
        }

        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // 6. Clean up temp file
    let _ = tokio::fs::remove_file(&target_path).await;
    info!("Verification test completed successfully. Test file cleaned up.");
}
