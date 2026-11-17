use std::time::Duration;

use rand;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExponentialBackoff {
    pub base_delay: std::time::Duration,
    pub max_delay: std::time::Duration,
    pub factor: f64,
    pub jitter: bool,
}
impl ExponentialBackoff {
    pub fn new(
        base_delay: std::time::Duration,
        max_delay: std::time::Duration,
        factor: f64,
        jitter: bool,
    ) -> Self {
        Self {
            base_delay,
            max_delay,
            factor,
            jitter,
        }
    }

    pub fn next_delay(&self, attempt: u32) -> std::time::Duration {
        // `factor.powi(attempt)` can overflow f64 to +inf for large attempts,
        // and `base_delay * inf` would panic in `mul_f64`. Compute the raw
        // nanos as f64 and cap at max_delay before constructing the Duration
        // so the call never panics.
        let factor = self.factor.powi(attempt as i32);
        let max_nanos = self.max_delay.as_nanos() as f64;
        let mut delay = if factor.is_finite() {
            let nanos = self.base_delay.as_nanos() as f64 * factor;
            if nanos.is_finite() && nanos <= max_nanos {
                std::time::Duration::from_nanos(nanos as u64)
            } else {
                self.max_delay
            }
        } else {
            self.max_delay
        };
        if delay > self.max_delay {
            delay = self.max_delay;
        }
        if self.jitter {
            let jitter_range = (delay.as_millis() as u64 / 2).max(1);
            let jitter = rand::random_range(0..jitter_range);
            delay += std::time::Duration::from_millis(jitter);
        }
        delay
    }
}
///Max attempt counts retries after the initial attempt, so a max_attempts of 3 means 1 initial attempt + 3 retries = 4 total attempts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryScheme {
    pub max_attempts: u32,
    pub backoff: ExponentialBackoff,
    pub stream_timeout: Duration,
    pub connection_timeout: Duration,
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            base_delay: std::time::Duration::from_secs(1),
            max_delay: std::time::Duration::from_secs(30),
            factor: 2.0,
            jitter: true,
        }
    }
}

impl Default for RetryScheme {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            backoff: ExponentialBackoff::default(),
            stream_timeout: Duration::from_secs(5),
            connection_timeout: Duration::from_secs(10),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Base case: attempt 0 -> factor^0 == 1, so delay == base_delay.
    #[test]
    fn next_delay_attempt_zero_is_base_delay() {
        let b = ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(30), 2.0, false);
        assert_eq!(b.next_delay(0), Duration::from_secs(1));
    }

    /// Growth: factor 2.0 doubles the delay each attempt.
    #[test]
    fn next_delay_grows_exponentially() {
        let b = ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(30), 2.0, false);
        assert_eq!(b.next_delay(1), Duration::from_secs(2));
        assert_eq!(b.next_delay(2), Duration::from_secs(4));
        assert_eq!(b.next_delay(3), Duration::from_secs(8));
    }

    /// Cap: delay never exceeds max_delay, even for very large attempts.
    #[test]
    fn next_delay_capped_at_max() {
        let b = ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(30), 2.0, false);
        let d = b.next_delay(1000);
        assert_eq!(d, Duration::from_secs(30));
        assert!(d <= Duration::from_secs(30));
    }

    /// Jitter bounds (statistical): with jitter enabled, every result for
    /// attempt 0 must lie in [base, base + jitter_range) where
    /// jitter_range = max(delay_ms/2, 1). Half-open range, so the upper
    /// bound is exclusive. 1000 iterations keeps this deterministic
    /// without needing a seeded RNG.
    #[test]
    fn next_delay_jitter_within_bounds() {
        let b = ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(30), 2.0, true);
        // Deterministic base (jitter disabled) pins the range.
        let base_only = ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(30), 2.0, false)
            .next_delay(0);
        let base_ms = base_only.as_millis() as u64;
        let jitter_range = (base_ms / 2).max(1);
        let upper = base_ms + jitter_range;
        for _ in 0..1000 {
            let d = b.next_delay(0);
            let ms = d.as_millis() as u64;
            assert!(
                ms >= base_ms && ms < upper,
                "jittered delay {ms} outside [{base_ms}, {upper})"
            );
        }
    }

    /// Defaults: ExponentialBackoff and RetryScheme match documented values.
    #[test]
    fn exponential_backoff_defaults() {
        let b = ExponentialBackoff::default();
        assert_eq!(b.base_delay, Duration::from_secs(1));
        assert_eq!(b.max_delay, Duration::from_secs(30));
        assert_eq!(b.factor, 2.0);
        assert!(b.jitter);
    }

    #[test]
    fn retry_scheme_defaults() {
        let r = RetryScheme::default();
        assert_eq!(r.max_attempts, 5);
        assert_eq!(r.stream_timeout, Duration::from_secs(5));
        assert_eq!(r.connection_timeout, Duration::from_secs(10));
        let def = ExponentialBackoff::default();
        assert_eq!(r.backoff.base_delay, def.base_delay);
        assert_eq!(r.backoff.max_delay, def.max_delay);
        assert_eq!(r.backoff.factor, def.factor);
        assert_eq!(r.backoff.jitter, def.jitter);
    }

    /// Serde round-trip: both types derive Serialize/Deserialize and cross
    /// the wire in DownloadOptions / DownloadData.
    #[test]
    fn exponential_backoff_serde_roundtrip() {
        let b = ExponentialBackoff::new(Duration::from_millis(250), Duration::from_secs(60), 1.5, false);
        let json = serde_json::to_string(&b).unwrap();
        let back: ExponentialBackoff = serde_json::from_str(&json).unwrap();
        assert_eq!(b.base_delay, back.base_delay);
        assert_eq!(b.max_delay, back.max_delay);
        assert_eq!(b.factor, back.factor);
        assert_eq!(b.jitter, back.jitter);
    }

    #[test]
    fn retry_scheme_serde_roundtrip() {
        let r = RetryScheme::default();
        let json = serde_json::to_string(&r).unwrap();
        let back: RetryScheme = serde_json::from_str(&json).unwrap();
        assert_eq!(r.max_attempts, back.max_attempts);
        assert_eq!(r.stream_timeout, back.stream_timeout);
        assert_eq!(r.connection_timeout, back.connection_timeout);
        assert_eq!(r.backoff.base_delay, back.backoff.base_delay);
        assert_eq!(r.backoff.max_delay, back.backoff.max_delay);
        assert_eq!(r.backoff.factor, back.backoff.factor);
        assert_eq!(r.backoff.jitter, back.backoff.jitter);
    }
}
