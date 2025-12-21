use crate::downloadrunerror::DownloadRunErrors;
use serde::Serialize;
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(tag = "kind", content = "payload")]
pub enum DownloadStatus {
    Stopped,
    Running {
        total: Option<u64>,
        downloaded: u64,
        download_speed: f64,
        downloaded_portion: Option<f64>,
        eta_ms: Option<u64>,
    },
    Completed {
        total: u64,
    },
    Failed(DownloadRunErrors),
    Paused {
        downloaded: u64,
        total: Option<u64>,
    },
}

pub fn calculate_download_speed(
    previous_downloaded: u64,
    current_downloaded: u64,
    time_passed: std::time::Duration,
) -> f64 {
    let current_downloaded: f64 = current_downloaded as f64;
    let previous_downloaded: f64 = previous_downloaded as f64;
    let time_passed: f64 = time_passed.as_secs_f64();
    if time_passed == 0.0 {
        return 0.0;
    }
    (current_downloaded - previous_downloaded) / time_passed
}

pub fn calculate_eta_in_ms(length: u64, speed: f64) -> Option<u64> {
    let speed_ms = speed / 1000.0;
    if speed_ms == 0.0 {
        return None;
    }
    Some(((length as f64) / speed_ms) as u64)
}
pub fn calculate_downloaded_portion(downloaded: u64, total: u64) -> f64 {
    let total: f64 = total as f64;
    let downloaded: f64 = downloaded as f64;
    if total == 0.0 {
        return 0.0;
    }
    downloaded / total
}
