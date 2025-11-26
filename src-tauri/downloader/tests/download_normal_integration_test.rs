use log::info;
use std::time::Duration;

use downloader::{self, download_manager};
#[tokio::test]
#[test_log::test]
async fn test_normal_download() {
    let client = reqwest::Client::new();
    let manager = download_manager::DownloadManager::new(client.clone(), 300);
    info!("I was here");
    manager
        .add_download_normal(
            "https://iso.omarchy.org/omarchy-3.1.7.iso".into(),
            "/home/arnob/DownloadsTest/omarchy-3.1.7.iso".into(),
        )
        .await
        .unwrap();
    info!("I was here 2");
    loop {
        let datas = manager.get_all_items().await.unwrap();
        for data in datas.values() {
            let serialized = serde_json::to_value(data).unwrap().to_string();
            info!("Reading status: {}", serialized);
        }
        info!("Number of items: {}", datas.len());

        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}
