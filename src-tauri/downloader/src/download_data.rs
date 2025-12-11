use crate::segment::SegmentData;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Clone)]
pub struct DownloadData {
    pub url: String,
    pub filepath: String,
    pub segments: Vec<SegmentData>,
    pub length: Option<u64>,
}
