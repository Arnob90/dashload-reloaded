use async_trait::async_trait;
use tokio::io::AsyncWriteExt;

use crate::filename_getter::get_filename;
use crate::idownloaditem::IDownloadItem;
use crate::lazy::AsyncLazy;
use crate::segment::{self, DownloadRunError};
use crate::{length_getter, statuses};
use std::path::{self, Path};
use std::time::Duration;
use tokio::select;
pub struct NormalDownloadItem {
    pub url: String,
    pub filepath: path::PathBuf,
    status_sender: tokio::sync::watch::Sender<statuses::DownloadStatus>,
    pub status_reciever: tokio::sync::watch::Receiver<statuses::DownloadStatus>,
    pub client: reqwest::Client,
    pub filesize: Option<u64>,
    pub update_interval: u64,
    pub filename: AsyncLazy<String>,
}

impl NormalDownloadItem {
    pub async fn new(
        url: String,
        filepath: path::PathBuf,
        client: reqwest::Client,
        update_interval: u64,
    ) -> NormalDownloadItem {
        let url_copy = url.clone();
        let length = length_getter::get_length(client.clone(), &url).await.ok();
        let client_clone = client.clone();
        let (sender, reciever) = tokio::sync::watch::channel(statuses::DownloadStatus::Pending);
        //I wish we could take a closure directly.
        //BUT THAT REQUIRES FN ONCE.
        //We need the factory pattern
        let filepath_clone = filepath.clone();
        let filename = AsyncLazy::new(move || {
            let client = client_clone.clone();
            let url = url_copy.clone();
            let filepath = filepath_clone.clone();
            async move {
                if filepath.is_file() {
                    return filepath.file_name().unwrap().to_str().unwrap().to_string();
                }
                get_filename(client, &url)
                    .await
                    .unwrap_or("untitled".to_string())
            }
        });
        Self {
            url,
            filepath,
            status_sender: sender,
            status_reciever: reciever,
            client,
            filesize: length,
            update_interval,
            filename,
        }
    }
}

impl NormalDownloadItem {
    async fn start_helper(&self) -> Result<(), DownloadRunError> {
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
        let mut filepath = self.filepath.as_path().to_path_buf();
        if filepath.is_dir() {
            let filename = get_filename(self.client.clone(), &self.url)
                .await
                .unwrap_or("untitled".to_string());
            let filename_path = Path::new(&filename);
            filepath = filepath.join(filename_path);
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(filepath)
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
            let _ = self.status_sender.send(statuses::DownloadStatus::Failed(
                err_maybe.clone().unwrap_err(),
            ));
        }
        return err_maybe;
    }
    fn get_status(&self) -> Result<statuses::DownloadStatus, segment::DownloadStatusCheckError> {
        Ok(self.status_reciever.borrow().clone())
    }
    async fn filename(&self) -> String {
        self.filename.get().await
    }
    async fn filepath(&self) -> String {
        if self.filepath.is_dir() {
            return self
                .filepath
                .join(self.filename.get().await)
                .to_str()
                .unwrap()
                .to_string();
        }
        self.filepath.to_str().unwrap().to_string()
    }
    fn get_url(&self) -> String {
        self.url.clone()
    }
    fn get_update_interval_ms(&self) -> u64 {
        self.update_interval
    }
}
