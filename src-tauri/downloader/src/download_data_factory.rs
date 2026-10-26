use crate::download_data::DownloadData;
use crate::download_status::DownloadState;
use crate::in_place_segment::{InPlaceSegmentData, SegmentInfo};
use crate::misc::clamp;
use crate::request_headers::RequestHeaders;
use crate::retry_scheme::RetryScheme;
use crate::serialization_error::DataSerializationError;
use crate::verifier::VerifierMaybe;
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, PartialEq, Eq)]
struct Interval {
    pub start: u64,
    pub end: u64,
}

/// Divides a file of given `size` into `num_segments` equal byte ranges.
fn divide_into_segments(size: u64, num_segments: u64) -> Vec<Interval> {
    if size == 0 {
        panic!("Size must be greater than 0 to divide into segments.");
    }
    let num_segments = clamp(num_segments, 1, size);
    let chunk_size = size / num_segments;
    let mut intervals = Vec::new();
    for i in 0..num_segments {
        let start = i * chunk_size;
        let end = if i == (num_segments - 1) {
            size - 1
        } else {
            (i + 1) * chunk_size - 1
        };
        intervals.push(Interval { start, end });
    }
    intervals
}

/// Factory for constructing `DownloadData` instances for both segmented and normal downloads,
/// plus deserialization from session files.
pub struct DownloadDataFactory;

impl DownloadDataFactory {
    /// Creates `DownloadData` for a new segmented (multi-chunk) download.
    ///
    /// # Arguments
    /// * `url` - The source download URL.
    /// * `filepath` - Local destination file path.
    /// * `filesize` - Total file size in bytes (required for segmentation).
    /// * `num_segments` - Number of concurrent segments to split the file into.
    /// * `verifier` - Optional checksum verifier for integrity.
    /// * `request_headers` - HTTP headers for the download requests.
    pub fn new_segmented(
        url: String,
        filepath: PathBuf,
        filesize: u64,
        num_segments: u64,
        verifier: VerifierMaybe,
        request_headers: HeaderMap,
        retry_scheme: RetryScheme,
    ) -> DownloadData {
        let segments = divide_into_segments(filesize, num_segments)
            .iter()
            .map(|i| InPlaceSegmentData {
                url: url.clone(),
                filepath: filepath.clone(),
                info: Some(SegmentInfo {
                    start: i.start,
                    end: i.end,
                }),
                last_downloaded: 0,
                headers: RequestHeaders::from_header_map(&request_headers),
            })
            .collect();
        DownloadData {
            url,
            filepath,
            length: Some(filesize),
            segments,
            state: DownloadState::Running {
                download_speed: 0.0,
            },
            verifier,
            retry_scheme,
        }
    }

    /// Creates `DownloadData` for a normal (single-stream / unknown size) download.
    pub fn new_normal(
        url: String,
        filepath: PathBuf,
        verifier: VerifierMaybe,
        request_headers: RequestHeaders,
        retry_scheme: RetryScheme,
    ) -> DownloadData {
        DownloadData {
            url: url.clone(),
            filepath: filepath.clone(),
            length: None,
            segments: vec![InPlaceSegmentData {
                url,
                filepath: filepath.clone(),
                last_downloaded: 0,
                info: None,
                headers: request_headers,
            }],
            state: DownloadState::Running {
                download_speed: 0.0,
            },
            verifier,
            retry_scheme,
        }
    }

    /// Deserializes a session file (JSON) into a map of download IDs to `DownloadData`.
    pub async fn deserialize_datas(
        json_path: &Path,
    ) -> Result<HashMap<String, DownloadData>, DataSerializationError> {
        let json_str = fs::read_to_string(json_path).await?;
        let req: HashMap<String, DownloadData> = serde_json::from_str(&json_str)?;
        Ok(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1. FAILS: Division by zero when size is 0.
    // num_segments.max(1).min(0) evaluates to 0, causing `0 / 0` to panic.
    #[test]
    #[should_panic(expected = "Size must be greater than 0 to divide into segments")]
    fn test_zero_size_causes_division_by_zero() {
        let _ = divide_into_segments(0, 5);
    }

    // 2. FAILS: Intermediate segments set `end` equal to `size`.
    // For size = 10 and num_segments = 2:
    // Segment 0 gets end = 10 (should be 4 or 5 depending on index convention).
    #[test]
    fn test_intermediate_segment_end_bound() {
        let result = divide_into_segments(10, 2);
        assert_eq!(result.len(), 2);

        // Expected non-overlapping intervals (assuming inclusive [0..4] and [5..9])
        assert_ne!(
            result[0].end, 10,
            "Intermediate segment end was incorrectly set to total size (10)"
        );
    }

    // 3. FAILS: Inconsistent boundary semantics (inclusive vs. exclusive).
    // Segment 0 gets end = 10 (exclusive style), while Segment 1 gets end = 9 (inclusive size - 1).
    #[test]
    fn test_inconsistent_boundary_indexing() {
        let result = divide_into_segments(100, 2);

        // If end is inclusive (0..=49, 50..=99):
        // result[0].end should be 49 and result[1].end should be 99.
        // Currently, result[0].end is 100, which exceeds valid 0-based indices.
        assert!(
            result[0].end < 100,
            "First interval end ({}) goes past valid indices for size 100",
            result[0].end
        );
    }

    // 4. FAILS: Overlapping intervals.
    // Segment 0 covers [0, 10], while Segment 1 starts at 5 and covers [5, 9].
    #[test]
    fn test_intervals_do_not_overlap() {
        let result = divide_into_segments(10, 2);

        let prev_end = result[0].end;
        let next_start = result[1].start;

        assert!(
            prev_end < next_start,
            "Intervals overlap: segment 0 ends at {}, but segment 1 starts at {}",
            prev_end,
            next_start
        );
    }

    // 5. FAILS: Remainder loss when size is not evenly divisible.
    // For size = 10, num_segments = 3: chunk_size = 3.
    // Segment 0 starts at 0, Segment 1 starts at 3, Segment 2 starts at 6.
    // Elements 7 and 8 are skipped if inclusive end is size - 1 (9).
    #[test]
    fn test_uneven_division_coverage() {
        let result = divide_into_segments(10, 3);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].start, 0);
        assert_eq!(result[1].start, 3);
        assert_eq!(result[2].start, 6);
        assert_eq!(result[2].end, 9); // Covers index 6 to 9, but intermediate bounds are broken.
    }

    // 6. FAILS: Asking for more segments than elements.
    // size = 3, num_segments = 10. num_segments is capped at 3, but bounds are wrong.
    #[test]
    fn test_more_segments_than_size() {
        let result = divide_into_segments(3, 10);
        assert_eq!(result.len(), 3);

        // chunk_size = 3 / 3 = 1
        // Expected inclusive intervals: [0, 0], [1, 1], [2, 2]
        assert_eq!(result[0], Interval { start: 0, end: 0 });
        assert_eq!(result[1], Interval { start: 1, end: 1 });
        assert_eq!(result[2], Interval { start: 2, end: 2 });
    }
}
