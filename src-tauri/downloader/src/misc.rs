use std::pin::Pin;

/// **Deprecated**: `FutureVec` is obsolete. Use standard unboxed `Vec<impl Future>` with `join_all` instead.
#[deprecated(
    note = "FutureVec is obsolete. Use standard unboxed Vec<impl Future> with join_all instead."
)]
pub type FutureVec<'a, OutputT> = Vec<Pin<Box<dyn Future<Output = OutputT> + 'a + Send>>>;

/// Clamps `value` to the inclusive range [`min`, `max`].
pub fn clamp(value: u64, min: u64, max: u64) -> u64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_below_range_returns_min() {
        assert_eq!(clamp(0, 10, 100), 10);
        assert_eq!(clamp(5, 10, 100), 10);
    }

    #[test]
    fn clamp_above_range_returns_max() {
        assert_eq!(clamp(1000, 10, 100), 100);
        assert_eq!(clamp(101, 10, 100), 100);
    }

    #[test]
    fn clamp_in_range_is_identity() {
        assert_eq!(clamp(50, 10, 100), 50);
        assert_eq!(clamp(10, 10, 100), 10);
        assert_eq!(clamp(100, 10, 100), 100);
    }
}
