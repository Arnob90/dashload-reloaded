use crate::download_data::DownloadData;
use crate::in_place_segment::{InPlaceSegmentData, SegmentInfo};
use std::path::{Path, PathBuf};
use tokio::fs;
pub struct DownloadDataFactory;

impl DownloadDataFactory {
    /// Creates DownloadData for a new segmented download
    pub fn new_segmented(
        url: String,
        filepath: PathBuf,
        filesize: u64,
        num_segments: u64,
    ) -> DownloadData {
        let num_segments = num_segments.max(1);
        let chunk_size = filesize / num_segments;
        let mut segments = Vec::new();

        for i in 0..num_segments {
            let start = i * chunk_size;
            let end = if i == num_segments - 1 {
                filesize - 1
            } else {
                (i + 1) * chunk_size - 1
            };

            segments.push(InPlaceSegmentData {
                url: url.clone(),
                filepath: filepath.clone(),
                last_downloaded: 0,
                info: Some(SegmentInfo { start, end }),
            });
        }

        DownloadData {
            url,
            filepath,
            length: Some(filesize),
            segments,
        }
    }

    /// Creates DownloadData for a normal (single-stream / unknown size) download
    pub fn new_normal(url: String, filepath: PathBuf) -> DownloadData {
        DownloadData {
            url: url.clone(),
            filepath: filepath.clone(),
            length: None,
            segments: vec![InPlaceSegmentData {
                url,
                filepath: filepath.clone(),
                last_downloaded: 0,
                info: None,
            }],
        }
    }

    /// Loads DownloadData from a saved JSON metadata file on disk
    pub async fn from_json_file(
        json_path: &Path,
    ) -> Result<DownloadData, Box<dyn std::error::Error>> {
        let json_str = fs::read_to_string(json_path).await?;
        let data: DownloadData = serde_json::from_str(&json_str)?;
        Ok(data)
    }
}
