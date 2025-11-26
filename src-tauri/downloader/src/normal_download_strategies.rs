use crate::segment::DownloadRunError;
use async_trait::async_trait;
use fs2::FileExt;
use reqwest::{Response, StatusCode};
use std::path::Path;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncSeekExt;
#[derive(Error, Debug)]
pub enum GetFileError {
    #[error("Cannot create the file!")]
    Creation,
    #[error("Failed to lock file")]
    Lock,
    #[error("Seek file error")]
    Seek,
}
#[async_trait]
pub trait DownloadStrategy: Send + Sync {
    async fn run(&self, client: reqwest::Client, url: &str) -> Result<Response, DownloadRunError>;
    fn get_start(&self) -> u64;
    async fn get_file(&self, filepath: &Path) -> Result<File, GetFileError>;
}
pub struct FullDownloadStrategy;

#[async_trait]
impl DownloadStrategy for FullDownloadStrategy {
    async fn run(&self, client: reqwest::Client, url: &str) -> Result<Response, DownloadRunError> {
        let res = client.get(url).send().await?;
        if !res.status().is_success() {
            return Err(DownloadRunError::ServerNotAvailable);
        }
        Ok(res)
    }
    async fn get_file(&self, filepath: &Path) -> Result<File, GetFileError> {
        let filepath_clone = filepath.to_path_buf();
        let file = tokio::task::spawn_blocking(move || {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(filepath_clone)
                .map_err(|_| GetFileError::Creation)?;
            let block = file.try_lock_exclusive();
            match block {
                Ok(_) => Ok(tokio::fs::File::from_std(file)),
                Err(_) => Err(GetFileError::Lock),
            }
        })
        .await
        .map_err(|_| GetFileError::Lock)??;
        Ok(file)
    }
    fn get_start(&self) -> u64 {
        0
    }
}

pub struct ResumedDownloadStrategy {
    pub start_offset: u64,
    pub end: Option<u64>,
    pub segment_start_offset: u64,
}

#[async_trait]
impl DownloadStrategy for ResumedDownloadStrategy {
    async fn run(&self, client: reqwest::Client, url: &str) -> Result<Response, DownloadRunError> {
        let range_header = format!(
            "bytes={}-{}",
            self.start_offset,
            self.end
                .map(|end| end.to_string())
                .unwrap_or("".to_string())
        );
        let res = client.get(url).header("Range", range_header).send().await?;
        match res.status() {
            StatusCode::PARTIAL_CONTENT => Ok(res),
            _ => Err(DownloadRunError::ServerNotRangable),
        }
    }
    fn get_start(&self) -> u64 {
        self.start_offset
    }
    async fn get_file(&self, filepath: &Path) -> Result<File, GetFileError> {
        let filepath_clone = filepath.to_path_buf();
        let mut file = tokio::task::spawn_blocking(move || {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(filepath_clone)
                .map_err(|_| GetFileError::Creation)?;
            let block = file.try_lock_exclusive();
            match block {
                Ok(_) => Ok(tokio::fs::File::from_std(file)),
                Err(_) => Err(GetFileError::Lock),
            }
        })
        .await
        .map_err(|_| GetFileError::Lock)??;
        file.seek(std::io::SeekFrom::Start(
            self.get_start() - self.segment_start_offset,
        ))
        .await
        .map_err(|_| GetFileError::Seek)?;
        Ok(file)
    }
}
