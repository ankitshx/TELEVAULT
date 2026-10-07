//! Exponential backoff and retry policy implementation.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration defining retry limits and exponential backoff parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum allowed retry attempts before declaring permanent failure.
    pub max_retries: u32,
    /// Initial backoff delay in milliseconds.
    pub initial_backoff_ms: u64,
    /// Maximum backoff ceiling in milliseconds.
    pub max_backoff_ms: u64,
    /// Multiplicative factor applied to backoff duration after each failure.
    pub backoff_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 2000,
            backoff_factor: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Creates a new custom [`RetryPolicy`].
    pub fn new(
        max_retries: u32,
        initial_backoff_ms: u64,
        max_backoff_ms: u64,
        backoff_factor: f64,
    ) -> Self {
        Self {
            max_retries,
            initial_backoff_ms,
            max_backoff_ms,
            backoff_factor,
        }
    }

    /// Returns `true` if another retry attempt is permitted.
    pub fn can_retry(&self, attempts_so_far: u32) -> bool {
        attempts_so_far < self.max_retries
    }

    /// Calculates the exponential backoff duration for the given 0-indexed attempt count.
    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(self.initial_backoff_ms);
        }

        let factor = self.backoff_factor.powi(attempt as i32);
        let calculated_ms = (self.initial_backoff_ms as f64 * factor).round() as u64;
        let clamped_ms = calculated_ms.min(self.max_backoff_ms);
        Duration::from_millis(clamped_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_defaults_and_backoff() {
        let policy = RetryPolicy::default();
        assert!(policy.can_retry(0));
        assert!(policy.can_retry(2));
        assert!(!policy.can_retry(3));

        let b0 = policy.calculate_backoff(0);
        let b1 = policy.calculate_backoff(1);
        let b2 = policy.calculate_backoff(2);
        let b10 = policy.calculate_backoff(10);

        assert_eq!(b0, Duration::from_millis(100));
        assert_eq!(b1, Duration::from_millis(200));
        assert_eq!(b2, Duration::from_millis(400));
        // Ceiling enforced
        assert_eq!(b10, Duration::from_millis(2000));
    }
}
