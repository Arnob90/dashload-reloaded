use serde::Serialize;
use thiserror::Error;

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

pub async fn get_filename(
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
