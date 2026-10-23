use std::path::PathBuf;

use crate::{
    download_status::DownloadState, in_place_segment::InPlaceSegmentData, verifier::VerifierMaybe,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Clone)]
pub struct DownloadData {
    pub url: String,
    pub filepath: PathBuf,
    pub segments: Vec<InPlaceSegmentData>,
    pub length: Option<u64>,
    pub state: DownloadState,
    pub verifier: VerifierMaybe,
}
