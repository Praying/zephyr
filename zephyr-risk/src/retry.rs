//! Retry policy implementation with exponential backoff.
//!
//! This module provides configurable retry strategies for handling
//! transient failures in network operations.
//!
//! # Example
//!
//! ```
//! use zephyr_risk::{RetryPolicy, RetryConfig, BackoffStrategy};
//! use std::time::Duration;
//!
//! let config = RetryConfig {
//!     max_retries: 3,
//!     initial_delay: Duration::from_millis(100),
//!     max_delay: Duration::from_secs(10),
//!     backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
//!     jitter: true,
//! };
//!
//! let policy = RetryPolicy::new(config);

// Allow precision loss and truncation in floating point calculations - acceptable for retry delays
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::suboptimal_flops)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::manual_midpoint)]
#![allow(clippy::unused_self)]
//!
//! // Get delay for each retry attempt
//! let delay1 = policy.get_delay(1); // ~100ms
//! let delay2 = policy.get_delay(2); // ~200ms
//! let delay3 = policy.get_delay(3); // ~400ms
//! ```

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;

/// Backoff strategy for retry delays.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BackoffStrategy {
    /// Fixed delay between retries.
    Fixed,
    /// Linear increase: delay = initial + (attempt * increment).
    Linear {
        /// Amount to add per retry attempt.
        increment_ms: u64,
    },
    /// Exponential increase: delay = initial * (multiplier ^ attempt).
    Exponential {
        /// Multiplier for each retry (typically 2.0).
        multiplier: f64,
    },
    /// Decorrelated jitter (AWS-style): delay = random(initial, previous * 3).
    DecorrelatedJitter,
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        Self::Exponential { multiplier: 2.0 }
    }
}

/// Configuration for retry policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Initial delay before first retry.
    #[serde(with = "humantime_serde")]
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    #[serde(with = "humantime_serde")]
    pub max_delay: Duration,
    /// Backoff strategy to use.
    pub backoff_strategy: BackoffStrategy,
    /// Whether to add random jitter to delays.
    #[serde(default)]
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Creates a new configuration with the given max retries.
    #[must_use]
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Sets the initial delay.
    #[must_use]
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Sets the maximum delay.
    #[must_use]
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Sets the backoff strategy.
    #[must_use]
    pub fn with_backoff(mut self, strategy: BackoffStrategy) -> Self {
        self.backoff_strategy = strategy;
        self
    }

    /// Enables or disables jitter.
    #[must_use]
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }
}

/// Retry policy for handling transient failures.
///
/// The policy calculates appropriate delays between retry attempts
/// using configurable backoff strategies.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Configuration.
    config: RetryConfig,
    /// Previous delay (for decorrelated jitter).
    previous_delay: Duration,
}

impl RetryPolicy {
    /// Creates a new retry policy with the given configuration.
    #[must_use]
    pub fn new(config: RetryConfig) -> Self {
        Self {
            previous_delay: config.initial_delay,
            config,
        }
    }

    /// Creates a retry policy with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Creates a retry policy for aggressive retries (short delays).
    #[must_use]
    pub fn aggressive() -> Self {
        Self::new(RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 1.5 },
            jitter: true,
        })
    }

    /// Creates a retry policy for conservative retries (longer delays).
    #[must_use]
    pub fn conservative() -> Self {
        Self::new(RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
            jitter: true,
        })
    }

    /// Returns the maximum number of retries.
    #[must_use]
    pub fn max_retries(&self) -> u32 {
        self.config.max_retries
    }

    /// Returns true if the given attempt should be retried.
    #[must_use]
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.config.max_retries
    }

    /// Calculates the delay for the given retry attempt (1-indexed).
    ///
    /// # Arguments
    ///
    /// * `attempt` - The retry attempt number (1 for first retry)
    ///
    /// # Returns
    ///
    /// The duration to wait before the retry.
    #[must_use]
    pub fn get_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let base_delay = self.calculate_base_delay(attempt);
        let capped_delay = base_delay.min(self.config.max_delay);

        let final_delay = if self.config.jitter {
            self.add_jitter(capped_delay)
        } else {
            capped_delay
        };

        debug!(
            attempt = attempt,
            delay_ms = final_delay.as_millis(),
            "Calculated retry delay"
        );

        final_delay
    }

    /// Calculates the delay and updates internal state (for decorrelated jitter).
    pub fn get_delay_mut(&mut self, attempt: u32) -> Duration {
        let delay = self.get_delay(attempt);
        self.previous_delay = delay;
        delay
    }

    /// Resets the policy state (for reuse).
    pub fn reset(&mut self) {
        self.previous_delay = self.config.initial_delay;
    }

    /// Calculates the base delay without jitter.
    fn calculate_base_delay(&self, attempt: u32) -> Duration {
        let initial_ms = self.config.initial_delay.as_millis() as f64;

        let delay_ms = match self.config.backoff_strategy {
            BackoffStrategy::Fixed => initial_ms,
            BackoffStrategy::Linear { increment_ms } => {
                initial_ms + (attempt as f64 * increment_ms as f64)
            }
            BackoffStrategy::Exponential { multiplier } => {
                initial_ms * multiplier.powi(attempt as i32 - 1)
            }
            BackoffStrategy::DecorrelatedJitter => {
                // AWS-style decorrelated jitter
                let prev_ms = self.previous_delay.as_millis() as f64;
                let max_ms = prev_ms * 3.0;
                // Without actual randomness, use a deterministic approximation
                (initial_ms + max_ms) / 2.0
            }
        };

        Duration::from_millis(delay_ms as u64)
    }

    /// Adds jitter to a delay.
    fn add_jitter(&self, delay: Duration) -> Duration {
        // Simple jitter: reduce by up to 25%
        // In production, this would use actual randomness
        let ms = delay.as_millis() as f64;
        let jitter_factor = 0.75 + (0.25 * self.deterministic_random(ms));
        Duration::from_millis((ms * jitter_factor) as u64)
    }

    /// Deterministic pseudo-random for testing (in production, use actual RNG).
    fn deterministic_random(&self, seed: f64) -> f64 {
        // Simple hash-based pseudo-random
        let hash = (seed * 1000.0) as u64;
        let mixed = hash.wrapping_mul(0x517c_c1b7_2722_0a95);
        (mixed % 1000) as f64 / 1000.0
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Builder for creating retry policies.
#[derive(Debug, Default)]
pub struct RetryPolicyBuilder {
    config: RetryConfig,
}

impl RetryPolicyBuilder {
    /// Creates a new builder with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum number of retries.
    #[must_use]
    pub fn max_retries(mut self, max: u32) -> Self {
        self.config.max_retries = max;
        self
    }

    /// Sets the initial delay.
    #[must_use]
    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.config.initial_delay = delay;
        self
    }

    /// Sets the maximum delay.
    #[must_use]
    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.config.max_delay = delay;
        self
    }

    /// Sets exponential backoff with the given multiplier.
    #[must_use]
    pub fn exponential_backoff(mut self, multiplier: f64) -> Self {
        self.config.backoff_strategy = BackoffStrategy::Exponential { multiplier };
        self
    }

    /// Sets linear backoff with the given increment.
    #[must_use]
    pub fn linear_backoff(mut self, increment: Duration) -> Self {
        self.config.backoff_strategy = BackoffStrategy::Linear {
            increment_ms: increment.as_millis() as u64,
        };
        self
    }

    /// Sets fixed delay (no backoff).
    #[must_use]
    pub fn fixed_delay(mut self) -> Self {
        self.config.backoff_strategy = BackoffStrategy::Fixed;
        self
    }

    /// Enables jitter.
    #[must_use]
    pub fn with_jitter(mut self) -> Self {
        self.config.jitter = true;
        self
    }

    /// Disables jitter.
    #[must_use]
    pub fn without_jitter(mut self) -> Self {
        self.config.jitter = false;
        self
    }

    /// Builds the retry policy.
    #[must_use]
    pub fn build(self) -> RetryPolicy {
        RetryPolicy::new(self.config)
    }
}

/// Iterator over retry delays.
pub struct RetryIterator {
    policy: RetryPolicy,
    current_attempt: u32,
}

impl RetryIterator {
    /// Creates a new retry iterator.
    #[must_use]
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            current_attempt: 0,
        }
    }
}

impl Iterator for RetryIterator {
    type Item = Duration;

    fn next(&mut self) -> Option<Self::Item> {
        self.current_attempt += 1;
        if self.policy.should_retry(self.current_attempt) {
            Some(self.policy.get_delay(self.current_attempt))
        } else {
            None
        }
    }
}

impl IntoIterator for RetryPolicy {
    type Item = Duration;
    type IntoIter = RetryIterator;

    fn into_iter(self) -> Self::IntoIter {
        RetryIterator::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = RetryPolicy::with_defaults();
        assert_eq!(policy.max_retries(), 3);
        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
    }

    #[test]
    fn test_exponential_backoff() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
            jitter: false,
        };
        let policy = RetryPolicy::new(config);

        // Without jitter, delays should be exact
        assert_eq!(policy.get_delay(1), Duration::from_millis(100));
        assert_eq!(policy.get_delay(2), Duration::from_millis(200));
        assert_eq!(policy.get_delay(3), Duration::from_millis(400));
        assert_eq!(policy.get_delay(4), Duration::from_millis(800));
    }

    #[test]
    fn test_linear_backoff() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Linear { increment_ms: 100 },
            jitter: false,
        };
        let policy = RetryPolicy::new(config);

        assert_eq!(policy.get_delay(1), Duration::from_millis(200));
        assert_eq!(policy.get_delay(2), Duration::from_millis(300));
        assert_eq!(policy.get_delay(3), Duration::from_millis(400));
    }

    #[test]
    fn test_fixed_backoff() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Fixed,
            jitter: false,
        };
        let policy = RetryPolicy::new(config);

        assert_eq!(policy.get_delay(1), Duration::from_millis(100));
        assert_eq!(policy.get_delay(2), Duration::from_millis(100));
        assert_eq!(policy.get_delay(3), Duration::from_millis(100));
    }

    #[test]
    fn test_max_delay_cap() {
        let config = RetryConfig {
            max_retries: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(500),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
            jitter: false,
        };
        let policy = RetryPolicy::new(config);

        // Should be capped at 500ms
        assert_eq!(policy.get_delay(5), Duration::from_millis(500));
        assert_eq!(policy.get_delay(10), Duration::from_millis(500));
    }

    #[test]
    fn test_jitter_reduces_delay() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Fixed,
            jitter: true,
        };
        let policy = RetryPolicy::new(config);

        let delay = policy.get_delay(1);
        // With jitter, delay should be between 750ms and 1000ms
        assert!(delay >= Duration::from_millis(750));
        assert!(delay <= Duration::from_millis(1000));
    }

    #[test]
    fn test_builder() {
        let policy = RetryPolicyBuilder::new()
            .max_retries(5)
            .initial_delay(Duration::from_millis(200))
            .max_delay(Duration::from_secs(5))
            .exponential_backoff(1.5)
            .without_jitter()
            .build();

        assert_eq!(policy.max_retries(), 5);
        assert_eq!(policy.get_delay(1), Duration::from_millis(200));
    }

    #[test]
    fn test_aggressive_policy() {
        let policy = RetryPolicy::aggressive();
        assert_eq!(policy.max_retries(), 5);
        assert!(policy.config.initial_delay < Duration::from_millis(100));
    }

    #[test]
    fn test_conservative_policy() {
        let policy = RetryPolicy::conservative();
        assert_eq!(policy.max_retries(), 3);
        assert!(policy.config.initial_delay >= Duration::from_secs(1));
    }

    #[test]
    fn test_retry_iterator() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Fixed,
            jitter: false,
        };
        let policy = RetryPolicy::new(config);

        assert_eq!(policy.into_iter().count(), 2); // 2 retries (attempts 1 and 2)
    }

    #[test]
    fn test_zero_attempt() {
        let policy = RetryPolicy::with_defaults();
        assert_eq!(policy.get_delay(0), Duration::ZERO);
    }

    #[test]
    fn test_reset() {
        let mut policy = RetryPolicy::with_defaults();
        policy.get_delay_mut(1);
        policy.get_delay_mut(2);
        policy.reset();
        // After reset, previous_delay should be back to initial
        assert_eq!(policy.previous_delay, policy.config.initial_delay);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_strategy: BackoffStrategy::Exponential { multiplier: 2.0 },
            jitter: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: RetryConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.max_retries, config.max_retries);
        assert_eq!(parsed.initial_delay, config.initial_delay);
    }
}
