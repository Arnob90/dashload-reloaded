use std::time::Duration;

use downloader::{self};
use log::info;
#[tokio::test]
#[test_log::test]
async fn test_download() {
    console_subscriber::init();
    let update_rate_ms: u64 = 400;
    let client = reqwest::Client::new();
    let download_manager =
        downloader::download_manager::DownloadManager::new(client.clone(), update_rate_ms);
    let download_link =
        "https://mirror.xeonbd.com/linuxmint.com/stable/22.2/linuxmint-22.2-xfce-64bit.iso";
    let filepath = "/home/arnob/Programming/dashload-reloaded/src-tauri/downloader/tests/downloads/linux-mint.iso";
    download_manager
        .add_download_segmented(download_link.to_owned(), filepath.into(), 4)
        .await
        .unwrap();
    loop {
        let datas = download_manager.get_all_items().await.unwrap();
        for (_, data) in datas {
            let serialized = serde_json::to_value(data).unwrap().to_string();
            info!("Reading status: {}", serialized);
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}
