use async_trait::async_trait;
use log::info;
use serde::Serialize;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs::{self};
use tokio::io::AsyncWriteExt;

use crate::download_data::DownloadData;
use crate::idownloaditem::IRunningDownloadItem;
use crate::normal_download_strategies::{
    DownloadStrategy, FullDownloadStrategy, ResumedDownloadStrategy,
};
use crate::segment::{self, DownloadRunError};
use crate::{length_getter, statuses};
use std::path::{self, Path};
use std::time::Duration;
use tokio::select;
#[derive(Error, Debug, Serialize, Clone)]
#[serde(tag = "kind", content = "payload")]
pub enum NormalDownloadItemCreationError {
    #[error("The given filepath is that of a directory. Filepath: {0}")]
    DirectoryGivenError(String),
    #[error("Cannot create the metadata file for download!")]
    MetadataFileCreationError,
    #[error("Cannot resume a download which has no metadata!")]
    NoMetadataFile,
    #[error("Given metadata file is invalid")]
    InvalidMetadataFile,
    #[error("No file to resume from!")]
    NoFileToResume,
    #[error("The file must've changed on the server side")]
    FileChangedOnServer,
    #[error("Failed to get filesize of file to resume")]
    FileSizeGettingError,
}
pub struct NormalRunningDownloadItem {
    pub url: String,
    pub filepath: path::PathBuf,
    status_sender: tokio::sync::watch::Sender<statuses::DownloadStatus>,
    pub status_reciever: tokio::sync::watch::Receiver<statuses::DownloadStatus>,
    pub client: reqwest::Client,
    pub filesize: Option<u64>,
    pub update_interval: u64,
    pub start: u64,
    pub download_strategy: Box<dyn DownloadStrategy>,
}

#[derive(Debug, Serialize)]
pub enum NormalDownloadItemErrors {
    DownloadRunError(DownloadRunError),
    NormalDownloadItemCreationError(NormalDownloadItemCreationError),
}

impl NormalRunningDownloadItem {
    async fn create_helper(
        url: String,
        filepath: path::PathBuf,
        client: reqwest::Client,
        update_interval: u64,
    ) -> Result<NormalRunningDownloadItem, NormalDownloadItemCreationError> {
        let length = length_getter::get_length(client.clone(), &url).await.ok();
        if filepath.is_dir() {
            return Err(NormalDownloadItemCreationError::DirectoryGivenError(
                filepath.to_str().unwrap().to_string(),
            ));
        }
        filepath
            .file_name()
            .ok_or(NormalDownloadItemCreationError::DirectoryGivenError(
                filepath.to_str().unwrap().to_string(),
            ))?;
        let (sender, reciever) = tokio::sync::watch::channel(statuses::DownloadStatus::Running {
            total: length,
            downloaded: 0,
        });
        let required_item = Self {
            url,
            filepath,
            status_sender: sender,
            status_reciever: reciever,
            client,
            filesize: length,
            update_interval,
            start: 0,
            download_strategy: Box::new(FullDownloadStrategy {}),
        };
        required_item
            .serialize()
            .await
            .map_err(|_| NormalDownloadItemCreationError::MetadataFileCreationError)?;
        Ok(required_item)
    }

    pub async fn new(
        url: String,
        filepath: path::PathBuf,
        client: reqwest::Client,
        update_interval: u64,
    ) -> Result<Arc<NormalRunningDownloadItem>, NormalDownloadItemCreationError> {
        let required_item =
            Arc::new(Self::create_helper(url, filepath, client, update_interval).await?);
        let required_item_clone = required_item.clone();
        let handle = tokio::spawn(async move {
            let _ = required_item_clone.start_internal().await;
        });
        std::mem::drop(handle);
        Ok(required_item)
    }

    async fn resume_from_file(
        client: reqwest::Client,
        filepath: path::PathBuf,
        update_interval: u64,
    ) -> Result<NormalRunningDownloadItem, NormalDownloadItemCreationError> {
        if filepath.is_dir() {
            return Err(NormalDownloadItemCreationError::DirectoryGivenError(
                filepath.to_str().unwrap().to_string(),
            ));
        }
        let required_metadata_filename =
            format!("{}.json", filepath.file_name().unwrap().to_str().unwrap());
        let metadata_path =
            path::Path::join(filepath.parent().unwrap(), required_metadata_filename);
        let deserialized_data_txt = fs::read_to_string(metadata_path)
            .await
            .map_err(|_| NormalDownloadItemCreationError::NoMetadataFile)?;
        let deserialized_data: DownloadData = serde_json::from_str(&deserialized_data_txt)
            .map_err(|_| NormalDownloadItemCreationError::InvalidMetadataFile)?;
        let required_filepath_str = deserialized_data.filepath;
        let len = length_getter::get_length(client.clone(), &deserialized_data.url)
            .await
            .ok();
        let required_filepath = Path::new(&required_filepath_str);
        if !required_filepath.exists() {
            return Err(NormalDownloadItemCreationError::NoFileToResume);
        }
        if len != deserialized_data.length {
            return Err(NormalDownloadItemCreationError::FileChangedOnServer);
        }
        let file_metadata = fs::metadata(&required_filepath_str)
            .await
            .map_err(|_| NormalDownloadItemCreationError::FileSizeGettingError)?;
        let start = file_metadata.len();
        let required_item = Self {
            url: deserialized_data.url,
            filepath: required_filepath.to_path_buf(),
            status_sender: tokio::sync::watch::channel(statuses::DownloadStatus::Running {
                total: None,
                downloaded: 0,
            })
            .0,
            status_reciever: tokio::sync::watch::channel(statuses::DownloadStatus::Running {
                total: None,
                downloaded: 0,
            })
            .1,
            client,
            filesize: len,
            update_interval,
            start,
            download_strategy: Box::new(ResumedDownloadStrategy {
                start_offset: start,
                end: None,
                segment_start_offset: 0,
            }),
        };
        Ok(required_item)
    }
    pub async fn create_with_resume(
        filepath: path::PathBuf,
        client: reqwest::Client,
        update_interval: u64,
    ) -> Result<Arc<NormalRunningDownloadItem>, NormalDownloadItemCreationError> {
        let required_item =
            Arc::new(Self::resume_from_file(client, filepath, update_interval).await?);
        let required_item_clone = required_item.clone();
        let handle = tokio::spawn(async move {
            let _ = required_item_clone.start_internal().await;
        });
        std::mem::drop(handle);
        Ok(required_item)
    }
}
#[derive(Error, Debug)]
pub enum FileGettingError {}

impl NormalRunningDownloadItem {
    async fn start_helper(&self) -> Result<(), DownloadRunError> {
        let required_sender = &self.status_sender;
        let _ = required_sender.send(statuses::DownloadStatus::Running {
            downloaded: 0,
            total: self.filesize,
        });
        let client_to_use = self.client.clone();
        let mut res = self.download_strategy.run(client_to_use, &self.url).await?;
        if !res.status().is_success() {
            info!("{}", res.status());
            return Err(DownloadRunError::ServerNotAvailable);
        }
        let mut file = self
            .download_strategy
            .get_file(&self.filepath)
            .await
            .map_err(|_| DownloadRunError::FileOpenError)?;
        let mut downloaded_so_far = self.download_strategy.get_start();
        let mut interval = tokio::time::interval(Duration::from_millis(self.update_interval));
        loop {
            if *required_sender.borrow() == statuses::DownloadStatus::Paused {
                return Ok(());
            }
            select! {
                next_chunk_maybe_err = res.chunk() => {
                    let next_chunk_maybe = next_chunk_maybe_err?;
                    if next_chunk_maybe.is_none() {
                        break;
                    }
                    let next_chunk = next_chunk_maybe.unwrap();
                    let chunk_size = next_chunk.len() as u64;
                    downloaded_so_far += chunk_size;
                    file.write_all(&next_chunk).await?;
                }
                _ = interval.tick() => {
                    let _ = required_sender.send(statuses::DownloadStatus::Running { total: self.filesize, downloaded: downloaded_so_far });
                }
            }
        }
        let _ = required_sender.send(statuses::DownloadStatus::Completed);
        Ok(())
    }
    async fn serialize(&self) -> Result<(), std::io::Error> {
        let required_data = self.get_download_data();
        let filename = self.filepath.file_name().unwrap();
        let to_serialize_to = format!("{}.json", filename.to_str().unwrap());
        let serialized = serde_json::to_string_pretty(&required_data).unwrap();
        fs::write(
            path::Path::join(self.filepath.parent().unwrap(), to_serialize_to),
            serialized,
        )
        .await
    }
    async fn start_internal(&self) -> Result<(), DownloadRunError> {
        let err_maybe = self.start_helper().await;
        if err_maybe.is_err() {
            let _ = self.status_sender.send(statuses::DownloadStatus::Failed(
                err_maybe.clone().unwrap_err(),
            ));
        }
        err_maybe
    }
}

#[async_trait]
impl IRunningDownloadItem for NormalRunningDownloadItem {
    fn get_status(&self) -> Result<statuses::DownloadStatus, segment::DownloadStatusCheckError> {
        Ok(self.status_reciever.borrow().clone())
    }
    fn filename(&self) -> String {
        self.filepath
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    }
    fn filepath(&self) -> String {
        self.filepath.to_str().unwrap().to_string()
    }
    fn get_url(&self) -> String {
        self.url.clone()
    }
    fn get_update_interval_ms(&self) -> u64 {
        self.update_interval
    }
    fn pause(&self) {
        let _ = self.status_sender.send(statuses::DownloadStatus::Paused);
    }
    fn get_status_reciever(&self) -> tokio::sync::watch::Receiver<statuses::DownloadStatus> {
        self.status_reciever.clone()
    }
    fn get_download_data(&self) -> DownloadData {
        DownloadData {
            url: self.url.clone(),
            filepath: self.filepath.to_str().unwrap().to_string(),
            segments: None,
            length: self.filesize,
        }
    }
}
