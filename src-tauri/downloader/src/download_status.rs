use crate::download_run_error::DownloadRunError;
use serde::{self, Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadState {
    Stopped,
    Running { download_speed: f64 },
    Completed { size: u64 },
    Failed(DownloadRunError),
    Paused,
    Verifying,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadStatus {
    pub total: Option<u64>,
    pub downloaded: u64,
    pub state: DownloadState,
}
