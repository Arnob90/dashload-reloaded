use serde::Serialize;
use thiserror::Error;
pub struct SegmentData {
    pub start: u64,
    pub end: u64,
}

#[derive(Error, Debug, Serialize)]
pub enum MakingSegmentError {
    #[error(
        "Cannot segment when the length is less than number of segments. \nLength: {0}\nNumber_of_segments: {1}"
    )]
    SegmentError(u64, u64),
}

pub fn segment_into_portions(
    length: u64,
    number_of_segments: u64,
) -> Result<Vec<SegmentData>, MakingSegmentError> {
    if length < number_of_segments {
        return Err(MakingSegmentError::SegmentError(length, number_of_segments));
    }
    if number_of_segments == 1 {
        return Ok(vec![SegmentData {
            start: 0,
            end: length - 1,
        }]);
    }
    let unit_length = length / number_of_segments;
    let mut segments: Vec<SegmentData> = vec![];
    for i in 0..(number_of_segments - 1) {
        segments.push(SegmentData {
            start: i * unit_length,
            end: ((i + 1) * unit_length) - 1,
        });
    }
    //Handle separately for uneven divisions
    segments.push(SegmentData {
        start: (number_of_segments - 1) * unit_length,
        end: length - 1,
    });
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_segment_into_portions() {
        let segments = segment_into_portions(10, 3).unwrap();
        assert_eq!(segments[0].start, 0);
        assert_eq!(segments[0].end, 2);
        assert_eq!(segments[1].start, 3);
        assert_eq!(segments[1].end, 5);
        assert_eq!(segments[2].start, 6);
        assert_eq!(segments[2].end, 9);
    }
    #[test]
    fn test_single_segment() {
        let segments = segment_into_portions(10, 1).unwrap();
        assert_eq!(segments[0].start, 0);
        assert_eq!(segments[0].end, 9);
    }
}
