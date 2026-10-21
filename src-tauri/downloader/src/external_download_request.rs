use serde::{Deserialize, Serialize};

use crate::request_headers::RequestHeaders;

#[derive(Deserialize, Debug, Clone, Default, Serialize)]
pub struct ExternalDownloadRequest {
    pub url: String,
    pub filename: Option<String>,
    pub filesize: Option<u64>,
    #[serde(flatten)]
    pub headers: RequestHeaders,
}
