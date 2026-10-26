use reqwest::header::HeaderMap;
use serde::Serialize;
use thiserror::Error;
use url::Url;

use crate::{info_getters, length_getter, segmented_verifier};

/// Aggregated metadata about a remote download resource.
#[derive(Debug, Serialize)]
pub struct DownloadInfo {
    /// The source URL.
    pub url: String,
    /// Suggested filename (if determinable).
    pub filename: Option<String>,
    /// File size in bytes (if available).
    pub filesize: Option<u64>,
    /// Whether segmented (multi-chunk) download is supported by the server.
    pub segmented_possible: bool,
}

/// Errors that can occur when fetching download metadata.
#[derive(Debug, Serialize, Error)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadInfoGetError {
    #[error("Request Failed: {0}")]
    RequestError(String),
    #[error("Invalid Url: {0}")]
    UrlError(String),
}

impl From<reqwest::Error> for DownloadInfoGetError {
    fn from(value: reqwest::Error) -> Self {
        DownloadInfoGetError::RequestError(format!("Request error: {}", value))
    }
}

impl From<url::ParseError> for DownloadInfoGetError {
    fn from(_: url::ParseError) -> Self {
        DownloadInfoGetError::UrlError("The provided url is malformed".to_string())
    }
}

/// Fetches comprehensive metadata for a download URL in parallel:
/// - Validates URL and server reachability.
/// - Determines file size (via `length_getter`).
/// - Determines suggested filename (via `info_getters`).
/// - Checks if segmented download is supported (via `segmented_verifier`).
pub async fn get_download_info(
    client: reqwest::Client,
    url: &str,
    headers: HeaderMap,
) -> Result<DownloadInfo, DownloadInfoGetError> {
    // Check if the server is even available
    Url::parse(url)?;
    client.get(url).send().await?;
    let length_maybe_future = length_getter::get_length(client.clone(), url);
    let filename_future = info_getters::get_filename_from_url(client.clone(), url);
    let (length_res, filename_res) = tokio::join!(length_maybe_future, filename_future);
    let length_maybe = length_res.ok();
    let filename = filename_res.ok();
    // Check if at least a single segment is possible with accept ranges
    let mut segmented_possible = false;
    if let Some(length) = length_maybe {
        let check =
            segmented_verifier::verify_segmented(client.clone(), url, 0, length - 1, headers).await;
        if check.is_ok() {
            segmented_possible = true;
        }
    }

    Ok(DownloadInfo {
        url: url.to_string(),
        filename,
        filesize: length_maybe,
        segmented_possible,
    })
}
