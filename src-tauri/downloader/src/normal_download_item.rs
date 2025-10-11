use async_trait::async_trait;
use tokio::io::AsyncWriteExt;

use crate::idownloaditem::IDownloadItem;
use crate::length_getter::LengthGettingErrors;
use crate::segment::{self, DownloadRunError};
use crate::{length_getter, statuses};
use std::path;
use std::time::Duration;
use tokio::select;
pub struct NormalDownloadItem {
    pub url: String,
    pub filepath: path::PathBuf,
    status_sender: tokio::sync::watch::Sender<statuses::DownloadStatus>,
    pub status_reciever: tokio::sync::watch::Receiver<statuses::DownloadStatus>,
    pub client: reqwest::Client,
    pub filesize: u64,
    pub update_interval: u64
}

impl NormalDownloadItem {
    pub async fn new(
        url: String,
        filepath: path::PathBuf,
        client: reqwest::Client,
        update_interval:u64
    ) -> Result<NormalDownloadItem, LengthGettingErrors> {
        let length = length_getter::get_length(client.clone(), &url).await?;
        let client_clone = client.clone();
        let (sender, reciever) = tokio::sync::watch::channel(statuses::DownloadStatus::Pending);
        Ok(Self {
            url,
            filepath,
            status_sender: sender.into(),
            status_reciever: reciever,
            client: client_clone,
            filesize: length,
            update_interval
        })
    }
}

impl NormalDownloadItem {
    async fn start_helper(&self) -> Result<(),DownloadRunError> {
        let required_sender = &self.status_sender;
        if *required_sender.borrow() != statuses::DownloadStatus::Pending {
            return Err(DownloadRunError::AlreadyRunning);
        }
        let _ = required_sender.send(statuses::DownloadStatus::Running {
            downloaded: 0,
            total: self.filesize,
        });
        let client_to_use = self.client.clone();
        let mut res = client_to_use.get(&self.url).send().await?;
        if !res.status().is_success() {
            return Err(DownloadRunError::ServerNotAvailable);
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .open(&self.filepath)
            .await?;
        let mut downloaded_so_far: u64 = 0;
        let mut interval = tokio::time::interval(Duration::from_millis(self.update_interval));
        loop {
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
}

#[async_trait]
impl IDownloadItem for NormalDownloadItem {
    async fn start(&self) -> Result<(), DownloadRunError> {
        let err_maybe = self.start_helper().await;
        if err_maybe.is_err() {
            let _ = self.status_sender.send(statuses::DownloadStatus::Failed(err_maybe.clone().unwrap_err()));
        }
        return err_maybe;
    }
    fn get_status(
        &self,
    ) -> Result<statuses::DownloadStatus, segment::DownloadStatusCheckError> {
        Ok(self.status_reciever.borrow().clone())
    }
}
