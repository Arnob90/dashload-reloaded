use crate::{
    segment::{self, DownloadRunError},
    statuses,
};
use async_trait::async_trait;
use serde::Serialize;

#[async_trait]
pub trait IDownloadItem: Send + Sync {
    async fn start(&self) -> Result<(), DownloadRunError>;
    fn get_status(&self) -> Result<statuses::DownloadStatus, segment::DownloadStatusCheckError>;
    async fn filename(&self) -> String;
    async fn filepath(&self) -> String;
    fn get_url(&self) -> String;
    fn get_update_interval_ms(&self) -> u64;
}
#[derive(Serialize, Clone)]
#[serde(tag = "kind", content = "payload")]
pub enum RichDownloadStatus {
    Pending,
    Running {
        downloaded: u64,
        total: Option<u64>,
        download_speed: f64,
        downloaded_portion: Option<f64>,
        eta_secs: Option<f64>,
    },
    Failed(DownloadRunError),
    Completed,
    Stopped,
}
#[derive(Serialize, Clone)]
pub struct IDownloadData {
    pub filepath: String,
    pub filename: String,
    pub url: String,
    pub status: RichDownloadStatus,
}
