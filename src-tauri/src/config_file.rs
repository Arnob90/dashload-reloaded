use std::path::PathBuf;

use downloader::retry_scheme::RetryScheme;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub auto_add_download: bool,
    pub default_download_path: PathBuf,
    pub default_number_of_segments: u64,
    /// Retry policy applied to new downloads. Defaults when absent from an
    /// existing config file so older configs keep loading.
    #[serde(default)]
    pub retry_scheme: RetryScheme,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_add_download: false,
            default_download_path: dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
            default_number_of_segments: 3,
            retry_scheme: RetryScheme::default(),
        }
    }
}
