//TODO: Fix remaining problems
use crate::downloaditem::{DownloadHandle, DownloadItem};
use crate::idownloaditem::IDownloadData;
use crate::misc::FutureVec;
use crate::resume_download_factory::{DownloadResumeFactory, ResumeFactoryErr};
use crate::segment::DownloadStatusCheckError;
use crate::segmented_download_item::{
    SegmentedDownloadItemCreationError, SegmentedRunningDownloadItem,
};
use crate::statuses::{self, DownloadStatus};
use dashmap::DashMap;
use futures::future::join_all;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::time::Instant;
use uuid::Uuid;
#[derive(Debug, Error, Clone, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadStatusGetError {
    #[error("Failed to check status")]
    StatusCheckError(#[from] DownloadStatusCheckError),
    #[error("Given id does not exist")]
    IdDoesNotExist,
}

#[derive(Debug, Error, Clone, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum ResumeError {
    #[error("Given id does not exist")]
    IdDoesNotExist,
    #[error("Download is already running!")]
    NotPausedError,
    #[error("Error creating download item")]
    DownloadItemCreationError(ResumeFactoryErr),
    #[error("Something went wrong!")]
    MiscError,
}

#[derive(Debug, Clone, Error, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum PauseError {
    #[error("Given id does not exist")]
    IdDoesNotExist,
    #[error("Download is not running")]
    NotRunningError,
}
pub struct DownloadManager {
    pub download_items: DashMap<String, Arc<DownloadItem>>,
    pub client: reqwest::Client,
    pub poll_every_ms: u64,
    pub status_history: DashMap<String, (statuses::DownloadStatus, Instant)>,
}
impl DownloadManager {
    pub fn new(client: reqwest::Client, poll_every_ms: u64) -> DownloadManager {
        Self {
            download_items: DashMap::new(),
            poll_every_ms,
            client,
            status_history: DashMap::new(),
        }
    }
    pub async fn add_download_segmented(
        &self,
        url: String,
        filepath: PathBuf,
        number_of_segments: u64,
    ) -> Result<(), SegmentedDownloadItemCreationError> {
        let required_download_item = DownloadItem::new(
            DownloadHandle::Running(
                SegmentedRunningDownloadItem::new(
                    self.client.clone(),
                    &url,
                    filepath,
                    number_of_segments,
                    self.poll_every_ms,
                )
                .await?,
            ),
            self.poll_every_ms,
        )
        .await;
        let uuid = Uuid::new_v4().as_simple().to_string();
        self.download_items.insert(uuid, required_download_item);
        Ok(())
    }
    // Renamed to be more explicit about what it does.
    // We usually catch this in batch, so we might as well own the string
    pub async fn get_download_data_from_item(
        &self,
        item_id: String,
    ) -> Result<IDownloadData, DownloadStatusGetError> {
        let required_item = self
            .download_items
            .get(&item_id)
            .ok_or(DownloadStatusGetError::IdDoesNotExist)?;
        let (data, status) = {
            let lock = required_item.handle.read().await;
            match *lock {
                DownloadHandle::Running(ref item) => (
                    item.get_download_data(),
                    item.get_status()
                        .map_err(DownloadStatusGetError::StatusCheckError)?,
                ),
                DownloadHandle::Terminal(ref item) => (item.data.clone(), item.status.clone()),
            }
        };
        let full_filepath = Path::new(&data.filepath);
        let filename = full_filepath
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        Ok(IDownloadData {
            status,
            filepath: data.filepath,
            url: data.url,
            filename,
        })
    }
    pub async fn get_all_items(
        &self,
    ) -> Result<HashMap<String, IDownloadData>, DownloadStatusGetError> {
        //A side effect of this design is that if the frontend has a large delay in calling this,
        //then the download speed will be more averaged
        let mut download_datas_map = HashMap::<String, IDownloadData>::new();
        let mut download_data_futures: FutureVec<Result<IDownloadData, DownloadStatusGetError>> =
            FutureVec::new();
        let mut id_list: Vec<String> = vec![];
        for entry in self.download_items.iter() {
            let id = entry.key().clone();
            let download_data = self.get_download_data_from_item(id.clone());
            download_data_futures.push(Box::pin(download_data));
            id_list.push(id);
        }
        let _download_datas = join_all(download_data_futures).await;
        for (i, download_data_maybe) in _download_datas.into_iter().enumerate() {
            let download_data = download_data_maybe?;
            download_datas_map.insert(id_list[i].to_string(), download_data.clone());
        }
        Ok(download_datas_map)
    }
    pub async fn resume(&self, item_id: &str) -> Result<(), ResumeError> {
        let to_resume = self.get_download_data_from_item(item_id.to_string()).await;
        if let Err(DownloadStatusGetError::IdDoesNotExist) = to_resume {
            return Err(ResumeError::IdDoesNotExist);
        }
        let to_resume_data = to_resume.map_err(|_| ResumeError::MiscError)?;
        let resume_status = to_resume_data.status;
        if let DownloadStatus::Paused { .. } = resume_status {
            let filepath_to_resume = Path::new(&to_resume_data.filepath);
            let handle = DownloadResumeFactory::resume_from_filepath(
                self.client.clone(),
                filepath_to_resume,
                self.poll_every_ms,
            )
            .await
            .map_err(ResumeError::DownloadItemCreationError)?;
            self.status_history.remove(item_id);
            self.download_items.insert(
                item_id.to_string(),
                DownloadItem::new(DownloadHandle::Running(handle), self.poll_every_ms).await,
            );
            Ok(())
        } else {
            Err(ResumeError::NotPausedError)
        }
    }
    pub async fn pause(&self, item_id: &str) -> Result<(), PauseError> {
        let to_pause = self
            .download_items
            .get(item_id)
            .ok_or(PauseError::IdDoesNotExist)?;
        let handle_read_lock = to_pause.handle.read().await;
        let handle = &*handle_read_lock;
        if let DownloadHandle::Running(item) = handle {
            item.pause();
            Ok(())
        } else {
            Err(PauseError::NotRunningError)
        }
    }
}
