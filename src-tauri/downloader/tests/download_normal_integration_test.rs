use std::time::Duration;
use test_log;
use log::info;

use downloader::{self, download_manager,statuses};
#[tokio::test]
#[test_log::test]
async fn test_normal_download() {
    let mut manager = download_manager::DownloadManager::new(300);
    let _ = manager.add_download_normal("https://mirror.xeonbd.com/ubuntu-releases/24.04.3/ubuntu-24.04.3-desktop-amd64.iso".to_string(), "/home/arnob/Programming/dashload-reloaded/src-tauri/downloader/tests/downloads/".to_string()).await;
    loop {
        let statuses = manager.get_all_status().unwrap();
        for (_,status) in statuses {
            if let statuses::DownloadStatus::Failed(current_err)=status {
                panic!("{}",current_err.to_string());
            }
            info!("Reading status: {:#?}",status)
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}
