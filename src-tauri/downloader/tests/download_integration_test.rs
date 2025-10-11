use std::time::Duration;

use downloader::{self, statuses};
use test_log;
use log::{Level, debug, error, info, log_enabled};
#[tokio::test]
#[test_log::test]
async fn test_download() {
    let update_rate_ms: u64 = 400;
    let mut download_manager = downloader::download_manager::DownloadManager::new(update_rate_ms);
    let download_link = "https://mirror.xeonbd.com/ubuntu-releases/24.04.3/ubuntu-24.04.3-desktop-amd64.iso";
    let filepath = "/home/arnob/Programming/dashload-reloaded/src-tauri/downloader/tests/downloads/somefile";
    download_manager.add_download_segmented(download_link.to_owned(), filepath.to_owned(), 4).await.unwrap();
    loop {
        let statuses = download_manager.get_all_status().unwrap();
        for (_,status) in statuses {
            if let statuses::DownloadStatus::Failed(current_err)=status {
                panic!("{}",current_err.to_string());
            }
            info!("Reading status: {:#?}",status)
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}
