use downloader::request_headers::RequestHeaders;
use downloader::retry_scheme::RetryScheme;
use std::time::Duration; 
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub auto_add_download: bool,
    #[serde(default)]
    pub default_download_path: PathBuf,
    pub default_number_of_segments: u64,
    /// Retry policy applied to new downloads. Defaults when absent from an
    /// existing config file so older configs keep loading.
    #[serde(default)]
    pub retry_scheme: RetryScheme,
    #[serde(default)]
    pub request_headers: RequestHeaders,
    /// Polling interval for status updates. Serialized as human-friendly duration.
    #[serde(with = "humantime_serde")]
    pub polling_rate: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_add_download: false,
            default_download_path: dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
            default_number_of_segments: 3,
            retry_scheme: RetryScheme::default(),
            request_headers: RequestHeaders::default(),
            polling_rate: Duration::from_millis(300),
        }
    }
}
