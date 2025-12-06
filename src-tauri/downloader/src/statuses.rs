use serde::Serialize;

use crate::segment::DownloadRunError;
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadStatus {
    Stopped,
    Running {
        total: Option<u64>,
        downloaded: u64,
        download_speed: u64,
    },
    Completed {
        total: u64,
    },
    Failed(DownloadRunError),
    Paused {
        downloaded: u64,
        total: Option<u64>,
    },
}
