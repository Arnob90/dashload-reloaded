use std::path::PathBuf;

use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub auto_add_download: bool,
    pub default_download_path: PathBuf,
    pub default_number_of_segments: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_add_download: false,
            default_download_path: dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
            default_number_of_segments: 3,
        }
    }
}
