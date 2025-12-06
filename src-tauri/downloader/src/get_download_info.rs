use serde::Serialize;
use url::Url;

use crate::{filename_getter, length_getter, segmented_verifier};
#[derive(Debug, Serialize)]
pub struct DownloadInfo {
    url: String,
    filename: Option<String>,
    filesize: Option<u64>,
    segmented_possible: bool,
}
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadInfoGetError {
    RequestError(String),
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

pub async fn get_download_info(
    client: reqwest::Client,
    url: &str,
) -> Result<DownloadInfo, DownloadInfoGetError> {
    //Check if the server is even available
    Url::parse(url)?;
    client.get(url).send().await?;
    let length_maybe_future = length_getter::get_length(client.clone(), url);
    let filename_future = filename_getter::get_filename(client.clone(), url);
    let (length_res, filename_res) = tokio::join!(length_maybe_future, filename_future);
    let length_maybe = length_res.ok();
    let filename = filename_res.ok();
    //Check if at least a single segment is possible with accept ranges
    let mut segmented_possible = false;
    if let Some(length) = length_maybe {
        let check =
            segmented_verifier::verify_segmented(client.clone(), url, 0, Some(length - 1)).await;
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
