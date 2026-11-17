use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, HeaderValue, RANGE};
use reqwest::{self, header::HeaderMap};
use serde::Serialize;
use thiserror::Error;

use crate::length_getter;

/// Errors that can occur during pre-flight verification of segmented download support.
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
    InvalidHeaderValue,
    #[error("Server error. Error code: {0}")]
    ServerFailure(u16),
    #[error("Cannot find metadata for resumption")]
    NoMetadataFile,
    #[error("The given download is not segmented")]
    NoSegments,
    #[error("The server does not have filesize information")]
    NoFilesize,
}

impl From<reqwest::Error> for SegmentVerificationError {
    fn from(err: reqwest::Error) -> Self {
        SegmentVerificationError::RequestError(err.to_string())
    }
}

/// Extracts `(start, end, total)` from a `Content-Range` header value.
///
/// The expected form is `bytes <start>-<end>/<total>` where `<total>` is a
/// concrete byte count (unknown totals like `bytes 0-9/*` are rejected).
/// Leading zeros are accepted (standard `u64::parse` semantics).
pub(crate) fn parse_content_range(value: &str) -> Result<(u64, u64, u64), SegmentVerificationError> {
    let (range_part, total_part) = value
        .strip_prefix("bytes ")
        .and_then(|value| value.split_once('/'))
        .ok_or(SegmentVerificationError::ServerNotRangable)?;

    let (start_part, end_part) = range_part
        .split_once('-')
        .ok_or(SegmentVerificationError::ServerNotRangable)?;

    let start = start_part
        .parse::<u64>()
        .map_err(|_| SegmentVerificationError::ServerNotRangable)?;
    let end = end_part
        .parse::<u64>()
        .map_err(|_| SegmentVerificationError::ServerNotRangable)?;
    let total = total_part
        .parse::<u64>()
        .map_err(|_| SegmentVerificationError::ServerNotRangable)?;

    Ok((start, end, total))
}

/// Verifies that a remote server supports segmented (range-based) downloads.
///
/// Checks that:
/// 1. The requested segment bounds are valid.
/// 2. A GET request with `Range: bytes=0-0` returns 206 Partial Content.
/// 3. Content-Range describes exactly byte 0 and reports the total file size.
/// 4. The probe response contains exactly one byte.
///
/// `Accept-Ranges` is deliberately not required because it is only advisory.
pub async fn verify_segmented(
    client: reqwest::Client,
    url: &str,
    _begin: u64,
    end: u64,
    headers: HeaderMap,
) -> Result<(), SegmentVerificationError> {
    // --- Probe actual range support ---
    // Preserve the caller's headers and override any existing Range header.
    let mut probe_headers = headers;
    probe_headers.insert(RANGE, HeaderValue::from_static("bytes=0-0"));

    let mut response = client.get(url).headers(probe_headers).send().await?;

    let status = response.status();

    if status.as_u16() == 200 {
        // The server ignored our Range request and returned the full resource.
        return Err(SegmentVerificationError::ServerNotRangable);
    }

    if status.as_u16() != 206 {
        return Err(SegmentVerificationError::ServerFailure(status.as_u16()));
    }

    // --- Validate Content-Range ---
    let content_range = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or(SegmentVerificationError::ServerNotRangable)?;

    let (start, returned_end, total_size) = parse_content_range(content_range)?;

    if start != 0 || returned_end != 0 || total_size == 0 {
        return Err(SegmentVerificationError::ServerNotRangable);
    }
    // Validate the requested segment against the confirmed file size.
    if end >= total_size {
        return Err(SegmentVerificationError::RangeError);
    }

    // Validate Content-Length when provided.
    if let Some(value) = response.headers().get(CONTENT_LENGTH) {
        let length = value
            .to_str()
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or(SegmentVerificationError::ServerNotRangable)?;

        if length != 1 {
            return Err(SegmentVerificationError::ServerNotRangable);
        }
    }

    // Validate the actual body without buffering the entire response.
    let mut received = 0usize;

    while let Some(chunk) = response.chunk().await? {
        received += chunk.len();

        if received > 1 {
            return Err(SegmentVerificationError::ServerNotRangable);
        }
    }

    if received != 1 {
        return Err(SegmentVerificationError::ServerNotRangable);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_range_valid() {
        assert_eq!(parse_content_range("bytes 0-0/1000").unwrap(), (0, 0, 1000));
        assert_eq!(parse_content_range("bytes 5-9/100").unwrap(), (5, 9, 100));
        assert_eq!(parse_content_range("bytes 00-00/1000").unwrap(), (0, 0, 1000));
    }

    #[test]
    fn parse_content_range_errors() {
        // Unknown total
        assert!(parse_content_range("bytes 0-99/*").is_err());
        // Missing /total
        assert!(parse_content_range("bytes 0-0").is_err());
        // Wrong unit
        assert!(parse_content_range("items 0-0/10").is_err());
        // Malformed garbage
        assert!(parse_content_range("garbage").is_err());
        assert!(parse_content_range("bytes a-b/10").is_err());
    }
}
