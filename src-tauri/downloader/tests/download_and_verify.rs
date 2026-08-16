use downloader::download_manager::DownloadManager;
use downloader::download_status::DownloadState;
use log::info;
use sha2::{Digest, Sha256};
use std::time::Duration;

#[test_log::test(tokio::test)]
async fn test_alpine_download_and_verify_sha256() {
    let download_url = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.24.1-x86_64.tar.gz";
    let sha256_url = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.24.1-x86_64.tar.gz.sha256";

    // Store in OS temp directory with a unique filename
    let temp_dir = std::env::temp_dir();
    let target_path = temp_dir.join(format!(
        "alpine_test_{}.tar.gz",
        uuid::Uuid::new_v4().as_simple()
    ));

    let client = reqwest::Client::new();
    let poll_every_ms = 200;
    let download_manager = DownloadManager::new(client.clone(), poll_every_ms);

    info!("Starting Alpine minirootfs download to {:?}", target_path);

    // 1. Add download with 4 concurrent segments
    let id = download_manager
        .add_download(download_url.to_string(), target_path.clone(), 4)
        .await
        .expect("Failed to add download");

    // 2. Poll until completed or failed
    loop {
        let status = download_manager
            .get_item_status(&id)
            .expect("Failed to get item status");

        match status.state {
            DownloadState::Completed { size } => {
                info!("Download completed! Final size: {} bytes", size);
                break;
            }
            DownloadState::Failed(err) => {
                panic!("Download failed with error: {:?}", err);
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

    // 3. Fetch official expected SHA256 checksum from Alpine CDN
    let sha256_response = client
        .get(sha256_url)
        .send()
        .await
        .expect("Failed to fetch official SHA256 file")
        .text()
        .await
        .expect("Failed to read SHA256 response text");

    // The .sha256 file format from Alpine is: "<hash>  <filename>"
    let expected_hash_str = sha256_response
        .split_whitespace()
        .next()
        .expect("Invalid .sha256 file format");

    // 4. Calculate SHA256 of the downloaded file on disk
    let downloaded_bytes = tokio::fs::read(&target_path)
        .await
        .expect("Failed to read downloaded file from disk");

    let mut hasher = Sha256::new();
    hasher.update(&downloaded_bytes);
    let actual_hash = hasher.finalize();
    let actual_hash_str = hex::encode(actual_hash);

    // 5. Verify Checksum Match
    assert_eq!(
        actual_hash_str, expected_hash_str,
        "Downloaded file checksum does not match official Alpine checksum!"
    );

    // 6. Clean up temp file
    let _ = tokio::fs::remove_file(&target_path).await;
    info!("Verification successful! Checksums match and test file cleaned up.");
}
