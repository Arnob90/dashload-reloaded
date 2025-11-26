use crate::download_data::DownloadData;
use crate::statuses;
pub struct TerminalDownloadItem {
    pub data: DownloadData,
    pub status: statuses::DownloadStatus,
}
