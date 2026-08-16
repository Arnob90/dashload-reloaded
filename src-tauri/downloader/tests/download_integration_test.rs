use downloader::download_manager::DownloadManager;
use downloader::download_status::DownloadState;
use std::path::PathBuf;
use std::time::Duration;

#[test_log::test(tokio::test)]
async fn test_download() {
    let update_rate_ms: u64 = 400;
    let client = reqwest::Client::new();
    let download_manager = DownloadManager::new(client.clone(), update_rate_ms);
    log::info!("I am here");
    let download_link =
        "https://mirror.xeonbd.com/linuxmint.com/stable/22.2/linuxmint-22.2-xfce-64bit.iso";
    let filepath = PathBuf::from("./tests/downloads/linux-mint.iso");

    // Unified add_download method
    let _id = download_manager
        .add_download(download_link.to_string(), filepath.clone(), 4)
        .await
        .unwrap();

    loop {
        // get_all_items is now non-async (instantaneous readout)
        let items = download_manager.get_all_items();

        for (id, status) in items {
            let serialized = serde_json::to_string(&status).unwrap();
            log::info!("ID: {}, Status: {}", id, serialized);

            // Exit loop when completed
            if matches!(status.state, DownloadState::Completed { .. }) {
                log::info!("Download finished successfully!");
                return;
            }

            // Exit loop if failed
            if let DownloadState::Failed(err) = status.state {
                panic!("Download failed with error: {:?}", err);
            }
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}
