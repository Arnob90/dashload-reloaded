use serde::{Deserialize, Serialize};

use crate::request_headers::RequestHeaders;

/// Payload received from external sources (e.g., browser extensions) to initiate a download.
#[derive(Deserialize, Debug, Clone, Default, Serialize)]
pub struct ExternalDownloadRequest {
    /// The download URL.
    pub url: String,
    /// Optional suggested filename (may be inferred from headers if absent).
    pub filename: Option<String>,
    /// Optional known file size in bytes.
    pub filesize: Option<u64>,
    /// Custom HTTP headers to use for the download request.
    pub headers: RequestHeaders,
}
