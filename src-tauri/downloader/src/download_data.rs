use std::path::PathBuf;

use crate::in_place_segment::InPlaceSegmentData;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct DownloadData {
    pub url: String,
    pub filepath: PathBuf,
    pub segments: Vec<InPlaceSegmentData>,
    pub length: Option<u64>,
}
