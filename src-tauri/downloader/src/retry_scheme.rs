use rand;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExponentialBackoff {
    base_delay: std::time::Duration,
    max_delay: std::time::Duration,
    factor: f64,
    jitter: bool,
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
        let mut delay = self.base_delay.mul_f64(self.factor.powi(attempt as i32));
        if delay > self.max_delay {
            delay = self.max_delay;
        }
        if self.jitter {
            let jitter_range = delay.as_millis() as u64 / 2.max(1);
            let jitter = rand::random::<u64>() % jitter_range;
            delay += std::time::Duration::from_millis(jitter);
        }
        delay
    }
}
///Max attempt counts retries including initial attempt, so a max_attempts of 3 means 1 initial attempt + 2 retries = 3 total attempts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryScheme {
    pub max_attempts: u32,
    pub backoff: ExponentialBackoff,
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
        }
    }
}
