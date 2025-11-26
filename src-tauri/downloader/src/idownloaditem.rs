use std::any::Any;

use crate::{
    download_data::DownloadData,
    segment::{self, DownloadRunError},
    statuses,
};
use async_trait::async_trait;
use serde::Serialize;

#[async_trait]
pub trait IRunningDownloadItem: Send + Sync + Any {
    fn get_status(&self) -> Result<statuses::DownloadStatus, segment::DownloadStatusCheckError>;
    fn filename(&self) -> String;
    fn filepath(&self) -> String;
    fn get_url(&self) -> String;
    fn get_update_interval_ms(&self) -> u64;
    fn pause(&self);
    fn get_status_reciever(&self) -> tokio::sync::watch::Receiver<statuses::DownloadStatus>;
    fn get_download_data(&self) -> DownloadData;
}

#[derive(Serialize, Clone, PartialEq)]
#[serde(tag = "kind", content = "payload")]
pub enum RichDownloadStatus {
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
    Paused,
}
#[derive(Serialize, Clone)]
pub struct IDownloadData {
    pub filepath: String,
    pub filename: String,
    pub url: String,
    pub status: RichDownloadStatus,
}
