use reqwest::{self, header::HeaderMap};
use serde::Serialize;
use thiserror::Error;

use crate::length_getter;
#[derive(Error, Debug, Serialize, Clone)]
#[serde(tag = "kind", content = "payload")]
pub enum SegmentVerificationError {
    #[error("Request failed!")]
    RequestError(String),
    #[error("The server does not have a concept of filesize")]
    LengthGettingError(#[from] length_getter::LengthGettingErrors),
    #[error("Given range is out of range")]
    RangeError,
    #[error("The server violates basic protocols")]
    IllBehavedServer,
    #[error("The server does not support range requests")]
    ServerNotRangable,
    #[error("Invalid header value")]
    InvalidHeaderValue, // New variant for the parse error
    #[error("Server error. Error code: {0}")]
    ServerFailure(u16),
    #[error("Cannot find metadata for resumption")]
    NoMetadataFile,
    #[error("The given download is not segmented")]
    NoSegments,
}
impl From<reqwest::Error> for SegmentVerificationError {
    fn from(err: reqwest::Error) -> Self {
        SegmentVerificationError::RequestError(err.to_string())
    }
}

pub async fn verify_segmented(
    client: reqwest::Client,
    url: &str,
    _begin: u64,
    end: u64,
) -> Result<(), SegmentVerificationError> {
    // --- Metadata Check with HEAD ---
    let response = client.head(url).send().await?;

    if !response.status().is_success() {
        return Err(SegmentVerificationError::ServerFailure(
            response.status().as_u16(),
        ));
    }

    if let Some(header_value) = response.headers().get("Accept-Ranges") {
        if header_value.to_str().map_or(true, |s| s != "bytes") {
            return Err(SegmentVerificationError::ServerNotRangable);
        }
    } else {
        return Err(SegmentVerificationError::ServerNotRangable);
    }
    let client_to_get_length = client.clone();
    let length_num: u64 = length_getter::get_length(client_to_get_length, url).await?;
    if end >= length_num {
        return Err(SegmentVerificationError::RangeError);
    }

    // --- Prepare and return the GET request Future ---
    let mut headers = HeaderMap::new();
    //Stream 0 bytes as test
    let range_header = format!("bytes={}-{}", 0, 0)
        .parse()
        .map_err(|_| SegmentVerificationError::InvalidHeaderValue)?;
    headers.insert("Range", range_header);

    let download_future = client.get(url).headers(headers).send().await?;
    let status_code = download_future.status().as_u16();
    if status_code == 206 {
        return Ok(());
    } else if status_code == 200 {
        return Err(SegmentVerificationError::ServerNotRangable);
    }
    Err(SegmentVerificationError::ServerFailure(status_code))
}
