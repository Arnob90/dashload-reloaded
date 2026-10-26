use dashload_downloader::download_manager::DownloadManager;
use dashload_downloader::download_status::DownloadState;
use dashload_downloader::request_headers::RequestHeaders;
use dashload_downloader::verifier::{HashAlgo, VerifierMaybe};
use std::path::PathBuf;
use std::time::Duration;

#[test_log::test(tokio::test)]
#[ignore = "Live Network Test"]
async fn test_download() {
    let update_rate_ms: u64 = 400;
    let client = reqwest::Client::new();
    let download_manager = DownloadManager::new(client.clone(), update_rate_ms);

    let download_link =
        "https://mirror.xeonbd.com/linuxmint.com/stable/22.2/linuxmint-22.2-xfce-64bit.iso";
    let sha256_url = "https://mirror.xeonbd.com/linuxmint.com/stable/22.2/sha256sum.txt";

    let filepath = PathBuf::from("./tests/downloads/linux-mint.iso");

    // 1. Fetch official sha256sum.txt from the mirror
    let sha_text = client
        .get(sha256_url)
        .send()
        .await
        .expect("Failed to fetch sha256sum.txt")
        .text()
        .await
        .expect("Failed to read sha256 text");

    // 2. Extract the specific hash for the Xfce ISO edition
    let expected_hash = sha_text
        .lines()
        .find(|line| line.contains("linuxmint-22.2-xfce-64bit.iso"))
        .and_then(|line| line.split_whitespace().next())
        .expect("Failed to find hash for linuxmint-22.2-xfce-64bit.iso in sha256sum.txt");

    log::info!("Official expected SHA256: {}", expected_hash);

    // 3. Build VerifierMaybe for the engine
    let verifier = VerifierMaybe::new(HashAlgo::Sha256, expected_hash.to_string());

    // 4. Configure headers with Tor Browser / Mozilla User-Agent
    let request_headers = RequestHeaders {
        user_agent: Some(
            "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0".to_string(),
        ),
        referrer: Some("https://linuxmint.com/".to_string()),
        cookies: None,
    };

    // 5. Start download with 4 segments, verifier, and request headers
    let id = download_manager
        .add_download(
            download_link.to_string(),
            filepath.clone(),
            4,
            verifier,
            request_headers, // 👈 Passed RequestHeaders here
        )
        .await
        .unwrap();

    // 6. Poll until completed or failed
    loop {
        let status = download_manager
            .get_item_status(&id)
            .expect("Failed to get item status");

        let serialized = serde_json::to_string(&status).unwrap();
        log::info!("ID: {}, Status: {}", id, serialized);

        match status.state {
            DownloadState::Completed { size } => {
                log::info!(
                    "Download and SHA256 verification finished successfully! Size: {} bytes",
                    size
                );
                break;
            }
            DownloadState::Failed(err) => {
                // Fails if either downloading or SHA256 verification in spawn_blocking failed
                panic!("Download or verification failed with error: {:?}", err);
            }
            _ => {}
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    // 7. Clean up test file
    let _ = tokio::fs::remove_file(&filepath).await;
}
