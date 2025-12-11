use std::any::Any;

use crate::{
    download_data::DownloadData,
    downloadrunerror::DownloadRunError,
    segment::{self},
    statuses::{self, DownloadStatus},
};
use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DeleteFileError {
    #[error("Failed to delete file. Reason: {0}")]
    IoError(String),
}
#[async_trait]
pub trait IRunningDownloadItem: Send + Sync + Any {
    fn get_status(&self) -> Result<statuses::DownloadStatus, segment::DownloadStatusCheckError>;
    fn filename(&self) -> String;
    fn filepath(&self) -> String;
    fn get_url(&self) -> String;
    fn get_update_interval_ms(&self) -> u64;
    fn pause(&self);
    fn get_download_data(&self) -> DownloadData;
    async fn cancel(&self) -> Result<(), DeleteFileError>;
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
    Completed {
        total: u64,
    },
    Stopped,
    Paused,
}
#[derive(Serialize, Clone)]
pub struct IDownloadData {
    pub filepath: String,
    pub filename: String,
    pub url: String,
    pub status: DownloadStatus,
}
