use reqwest::header::HeaderMap;
use serde::Serialize;
use std::path::Path;
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

/// Strips directory traversal (`../`, `/`, `\`) and replaces OS-illegal characters
/// so an untrusted string can be safely joined onto a destination directory.
pub fn sanitize_filename(raw: &str) -> String {
    // 1. Extract only the final filename component (strips all `/`, `\`, `../`, `C:\`)
    let base_name = Path::new(raw.trim())
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // 2. Replace characters forbidden on Windows/macOS/Linux filesystems
    let cleaned: String = base_name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    // 3. Strip leading/trailing dots & spaces (Windows hates trailing dots/spaces)
    let trimmed = cleaned.trim_matches(|c| c == ' ' || c == '.');

    // 4. Fallback if nothing valid remains
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        "download".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Attempts to determine a suitable filename from the remote URL.
/// Tries `HEAD` first, falls back to `GET`, and parses `Content-Disposition` header.
/// Falls back to the last path segment of the URL if no header is present.
pub async fn get_filename_from_url_unsanitized(
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

pub async fn get_filename_from_url(
    client: reqwest::Client,
    url: &str,
) -> Result<String, FilepathGettingError> {
    let unsanitized = get_filename_from_url_unsanitized(client, url).await?;
    Ok(sanitize_filename(&unsanitized))
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
