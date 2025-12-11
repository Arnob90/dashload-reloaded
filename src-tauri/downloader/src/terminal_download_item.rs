use crate::download_data::DownloadData;
use crate::statuses;
#[derive(Debug, Clone)]
pub struct TerminalDownloadItem {
    pub data: DownloadData,
    pub status: statuses::DownloadStatus,
}
