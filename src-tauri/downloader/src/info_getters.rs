use reqwest::header::HeaderMap;
use serde::Serialize;
use thiserror::Error;

/// Errors that can occur when extracting a filename from a remote URL.
#[derive(Error, Debug, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum FilepathGettingError {
    #[error("Unreachable Server")]
    UnreachableServer,
    #[error("Request failed")]
    RequestError(String),
    #[error("No content disposition")]
    ContentDispositionLess,
    #[error("No filename")]
    Filenameless,
    #[error("Given url is invalid. Url: {0}")]
    InvalidUrl(String),
}

impl From<reqwest::Error> for FilepathGettingError {
    fn from(e: reqwest::Error) -> Self {
        FilepathGettingError::RequestError(e.to_string())
    }
}

/// Attempts to determine a suitable filename from the remote URL.
/// Tries `HEAD` first, falls back to `GET`, and parses `Content-Disposition` header.
/// Falls back to the last path segment of the URL if no header is present.
pub async fn get_filename_from_url(
    client: reqwest::Client,
    url: &str,
) -> Result<String, FilepathGettingError> {
    let mut res = client.head(url).send().await?;
    if !res.status().is_success() {
        res = client.get(url).send().await?;
        if !res.status().is_success() {
            return Err(FilepathGettingError::UnreachableServer);
        }
    }
    let content_disposition_maybe = res.headers().get("Content-Disposition");
    if let Some(content_disposition_content) = content_disposition_maybe
        && let Ok(content_disposition_str) = content_disposition_content.to_str()
    {
        let parsed = content_disposition::parse_content_disposition(content_disposition_str);
        if let Some(filename) = parsed.filename_full() {
            return Ok(filename);
        }
    }
    let trimmed_url = url.trim();
    if trimmed_url.ends_with("/") {
        return Err(FilepathGettingError::Filenameless);
    }
    let url_parsed =
        url::Url::parse(url).map_err(|_| FilepathGettingError::InvalidUrl(url.to_owned()))?;
    let url_segments_maybe = url_parsed.path_segments();
    if let Some(mut url_segments) = url_segments_maybe {
        let result_or_err = url_segments
            .next_back()
            .map(|seg| seg.to_owned())
            .ok_or(FilepathGettingError::Filenameless);
        return result_or_err;
    }
    Err(FilepathGettingError::Filenameless)
}

/// Fetches the `Content-Length` header via `HEAD` request to determine file size.
/// Returns `None` if the request fails or the header is missing/malformed.
pub async fn get_filesize(
    client: reqwest::Client,
    url: &str,
    header_map: HeaderMap,
) -> Option<u64> {
    let req = client.head(url).headers(header_map);
    req.send()
        .await
        .ok()?
        .headers()
        .get("Content-Length")
        .and_then(|len| len.to_str().ok())
        .and_then(|len_str| len_str.parse::<u64>().ok())
}
