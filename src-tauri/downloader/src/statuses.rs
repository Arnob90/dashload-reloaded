#[derive(Clone)]
pub enum DownloadStatus {
    Stopped,
    Running { total: u64, downloaded: u64 },
    Completed,
}
