use serde::Serialize;

use crate::segment::DownloadRunError;
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadStatus {
    Stopped,
    Running { total: Option<u64>, downloaded: u64 },
    Completed,
    Failed(DownloadRunError),
    Paused,
}
