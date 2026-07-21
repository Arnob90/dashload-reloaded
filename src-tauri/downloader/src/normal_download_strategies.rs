use crate::segment::DownloadRunError;
use async_trait::async_trait;
use fs2::FileExt;
use reqwest::{Response, StatusCode};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::Duration;
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
    #[error("Failed to acquire lock. Timed out")]
    Timeout,
}

async fn get_file_lock_within_timeout_idiomatic(
    filepath: PathBuf,
    options: OpenOptions,
    timeout: Duration,
) -> Result<tokio::fs::File, GetFileError> {
    let action = async {
        let mut polling_rate = tokio::time::interval(Duration::from_millis(50));
        loop {
            let options_clone = options.clone();
            let filepath_clone = filepath.clone();

            let file_result = tokio::task::spawn_blocking(move || {
                let opened_file = options_clone
                    .open(filepath_clone)
                    .map_err(|_| GetFileError::Creation)?;
                match opened_file.try_lock_exclusive() {
                    Ok(_) => Ok(tokio::fs::File::from_std(opened_file)),
                    Err(_) => Err(GetFileError::Lock),
                }
            })
            .await
            .map_err(|_| GetFileError::Lock)?;

            match file_result {
                Ok(file) => return Ok(file),
                Err(GetFileError::Lock) => {
                    polling_rate.tick().await;
                }
                Err(e) => return Err(e),
            }
        }
    };

    // Automatically aborts the internal async block if it exceeds the duration
    tokio::time::timeout(timeout, action)
        .await
        .map_err(|_| GetFileError::Timeout)?
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
        let file = get_file_lock_within_timeout_idiomatic(
            filepath_clone,
            std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(true)
                .clone(),
            Duration::from_millis(500),
        )
        .await?;
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
        let mut file = get_file_lock_within_timeout_idiomatic(
            filepath_clone,
            OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .clone(),
            Duration::from_millis(400),
        )
        .await?;
        let diff_maybe = self.get_start().checked_sub(self.segment_start_offset);
        if let Some(diff) = diff_maybe {
            file.seek(std::io::SeekFrom::Start(diff))
                .await
                .map_err(|_| GetFileError::Seek)?;
        } else {
            return Err(GetFileError::Seek);
        }
        Ok(file)
    }
}
