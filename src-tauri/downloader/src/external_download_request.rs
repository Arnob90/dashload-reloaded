use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Clone, Default, Serialize)]
pub struct ExternalDownloadRequest {
    pub url: String,
    pub filename: Option<String>,
    pub referrer: Option<String>,
    pub cookies: Option<String>,
    pub user_agent: Option<String>,
    pub filesize: Option<u64>,
}
