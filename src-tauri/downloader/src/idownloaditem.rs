use crate::{
    segment::{self, DownloadRunError},
    statuses,
};
use async_trait::async_trait;

#[async_trait]
pub trait IDownloadItem: Send + Sync {
    async fn start(&self) -> Result<(), DownloadRunError>;
    fn get_status(&self) -> Result<statuses::DownloadStatus, segment::DownloadStatusCheckError>;
    async fn filename(&self) -> String;
    async fn filepath(&self) -> String;
}
