use log::info;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

use crate::{
    download_data::DownloadData, idownloaditem::IRunningDownloadItem,
    segmented_download_item::SegmentedDownloadItemCreationError,
};

pub struct DownloadResumeFactory {}
#[derive(Debug, Error, Clone, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum ResumeFactoryErr {
    #[error("Metadata file cannot be opened or does not exist")]
    MetadataFileError,
    #[error("Metadata cannot be deserialized")]
    MetadataDeserializationError,
    #[error("Segmented download cannot be created")]
    SegmentedDownloadCreationError(SegmentedDownloadItemCreationError),
}
impl DownloadResumeFactory {
    pub async fn resume_from_filepath(
        client: reqwest::Client,
        filepath: &Path,
        polling_rate_ms: u64,
    ) -> Result<Arc<dyn IRunningDownloadItem>, ResumeFactoryErr> {
        let required_metadata_filename =
            format!("{}.json", filepath.file_name().unwrap().to_str().unwrap());
        let metadata_path = filepath
            .parent()
            .expect("The given item must be a file")
            .join(required_metadata_filename);
        info!("Resuming from {}", metadata_path.display());
        let metadata_text = tokio::fs::read_to_string(metadata_path)
            .await
            .map_err(|_| ResumeFactoryErr::MetadataFileError)?;
        let metadata_read: DownloadData = serde_json::from_str(&metadata_text)
            .map_err(|_| ResumeFactoryErr::MetadataDeserializationError)?;
        let required_filepath = Path::new(&metadata_read.filepath);
        let download_item =
            crate::segmented_download_item::SegmentedRunningDownloadItem::create_with_resume(
                client.clone(),
                required_filepath.to_path_buf(),
                polling_rate_ms,
            )
            .await
            .map_err(ResumeFactoryErr::SegmentedDownloadCreationError)?;
        Ok(download_item)
    }
}
