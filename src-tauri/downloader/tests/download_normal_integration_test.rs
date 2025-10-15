use log::info;
use std::time::Duration;

use downloader::{self, download_manager};
#[tokio::test]
#[test_log::test]
async fn test_normal_download() {
    let mut manager = download_manager::DownloadManager::new(300);
    let _ = manager
        .add_download_normal(
            "https://file-examples.com/wp-content/storage/2017/04/file_example_MP4_1920_18MG.mp4"
                .to_string(),
            "/home/arnob/Programming/dashload-reloaded/src-tauri/downloader/tests/downloads/"
                .to_string(),
        )
        .await;
    loop {
        let datas = manager.get_all_items().await.unwrap();
        for (_, data) in datas {
            let serialized = serde_json::to_value(data).unwrap().to_string();
            info!("Reading status: {}", serialized);
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}
