//! Rate limiter for API requests.

#![allow(clippy::significant_drop_in_scrutinee)]
#![allow(clippy::unchecked_time_subtraction)]

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Token bucket rate limiter.
///
/// Implements a sliding window rate limiter that tracks request timestamps
/// and enforces rate limits.
#[derive(Debug)]
pub struct RateLimiter {
    /// Maximum requests per window.
    max_requests: u32,
    /// Window duration.
    window: Duration,
    /// Request timestamps.
    timestamps: Mutex<VecDeque<Instant>>,
    /// Remaining requests (from exchange headers).
    remaining: Mutex<Option<u32>>,
    /// Reset time (from exchange headers).
    reset_at: Mutex<Option<Instant>>,
}

impl RateLimiter {
    /// Creates a new rate limiter.
    ///
    /// # Arguments
    ///
    /// * `max_requests` - Maximum requests allowed in the window
    /// * `window` - Duration of the sliding window
    #[must_use]
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            timestamps: Mutex::new(VecDeque::with_capacity(max_requests as usize)),
            remaining: Mutex::new(None),
            reset_at: Mutex::new(None),
        }
    }

    /// Creates a rate limiter for requests per second.
    #[must_use]
    pub fn per_second(max_requests: u32) -> Self {
        Self::new(max_requests, Duration::from_secs(1))
    }

    /// Creates a rate limiter for requests per minute.
    #[must_use]
    pub fn per_minute(max_requests: u32) -> Self {
        Self::new(max_requests, Duration::from_secs(60))
    }

    /// Checks if a request can be made immediately.
    #[must_use]
    pub fn can_proceed(&self) -> bool {
        self.wait_time().is_zero()
    }

    /// Returns the time to wait before the next request can be made.
    #[must_use]
    pub fn wait_time(&self) -> Duration {
        // Check exchange-provided remaining count first
        if let Some(remaining) = *self.remaining.lock() {
            if remaining == 0 {
                if let Some(reset_at) = *self.reset_at.lock() {
                    let now = Instant::now();
                    if reset_at > now {
                        return reset_at - now;
                    }
                }
            }
        }

        let mut timestamps = self.timestamps.lock();
        let now = Instant::now();

        // Remove expired timestamps
        while let Some(&oldest) = timestamps.front() {
            if now.duration_since(oldest) >= self.window {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        // Check if we're at the limit
        if timestamps.len() >= self.max_requests as usize {
            if let Some(&oldest) = timestamps.front() {
                let elapsed = now.duration_since(oldest);
                if elapsed < self.window {
                    return self.window - elapsed;
                }
            }
        }

        Duration::ZERO
    }

    /// Records a request and returns whether it was allowed.
    ///
    /// If the request would exceed the rate limit, returns `false`
    /// and does not record the request.
    pub fn try_acquire(&self) -> bool {
        let wait = self.wait_time();
        if !wait.is_zero() {
            return false;
        }

        let mut timestamps = self.timestamps.lock();
        timestamps.push_back(Instant::now());
        true
    }

    /// Records a request unconditionally.
    ///
    /// Use this when you've already waited for the rate limit.
    pub fn record_request(&self) {
        let mut timestamps = self.timestamps.lock();
        timestamps.push_back(Instant::now());
    }

    /// Waits until a request can be made, then records it.
    pub async fn acquire(&self) {
        loop {
            let wait = self.wait_time();
            if wait.is_zero() {
                self.record_request();
                return;
            }
            tokio::time::sleep(wait).await;
        }
    }

    /// Updates the rate limiter with information from exchange headers.
    ///
    /// Many exchanges return rate limit information in response headers.
    pub fn update_from_headers(&self, remaining: Option<u32>, reset_ms: Option<u64>) {
        if let Some(r) = remaining {
            *self.remaining.lock() = Some(r);
        }

        if let Some(reset) = reset_ms {
            let reset_at = Instant::now() + Duration::from_millis(reset);
            *self.reset_at.lock() = Some(reset_at);
        }
    }

    /// Resets the rate limiter state.
    pub fn reset(&self) {
        self.timestamps.lock().clear();
        *self.remaining.lock() = None;
        *self.reset_at.lock() = None;
    }

    /// Returns the current number of requests in the window.
    #[must_use]
    pub fn current_count(&self) -> usize {
        let mut timestamps = self.timestamps.lock();
        let now = Instant::now();

        // Remove expired timestamps
        while let Some(&oldest) = timestamps.front() {
            if now.duration_since(oldest) >= self.window {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        timestamps.len()
    }

    /// Returns the maximum requests allowed.
    #[must_use]
    pub fn max_requests(&self) -> u32 {
        self.max_requests
    }

    /// Returns the window duration.
    #[must_use]
    pub fn window(&self) -> Duration {
        self.window
    }

    /// Returns the remaining requests (from exchange headers).
    #[must_use]
    pub fn remaining(&self) -> Option<u32> {
        *self.remaining.lock()
    }
}

/// Composite rate limiter that enforces multiple limits.
///
/// Useful when an exchange has both per-second and per-minute limits.
#[derive(Debug)]
pub struct CompositeRateLimiter {
    limiters: Vec<RateLimiter>,
}

impl CompositeRateLimiter {
    /// Creates a new composite rate limiter.
    #[must_use]
    pub fn new(limiters: Vec<RateLimiter>) -> Self {
        Self { limiters }
    }

    /// Creates a composite limiter with per-second and per-minute limits.
    #[must_use]
    pub fn with_limits(per_second: u32, per_minute: u32) -> Self {
        let mut limiters = Vec::new();
        if per_second > 0 {
            limiters.push(RateLimiter::per_second(per_second));
        }
        if per_minute > 0 {
            limiters.push(RateLimiter::per_minute(per_minute));
        }
        Self { limiters }
    }

    /// Checks if a request can be made immediately.
    #[must_use]
    pub fn can_proceed(&self) -> bool {
        self.limiters.iter().all(RateLimiter::can_proceed)
    }

    /// Returns the maximum wait time across all limiters.
    #[must_use]
    pub fn wait_time(&self) -> Duration {
        self.limiters
            .iter()
            .map(RateLimiter::wait_time)
            .max()
            .unwrap_or(Duration::ZERO)
    }

    /// Tries to acquire from all limiters.
    pub fn try_acquire(&self) -> bool {
        if !self.can_proceed() {
            return false;
        }

        for limiter in &self.limiters {
            limiter.record_request();
        }
        true
    }

    /// Waits until all limiters allow a request.
    pub async fn acquire(&self) {
        loop {
            let wait = self.wait_time();
            if wait.is_zero() {
                for limiter in &self.limiters {
                    limiter.record_request();
                }
                return;
            }
            tokio::time::sleep(wait).await;
        }
    }

    /// Resets all limiters.
    pub fn reset(&self) {
        for limiter in &self.limiters {
            limiter.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_basic() {
        let limiter = RateLimiter::new(3, Duration::from_secs(1));

        assert!(limiter.can_proceed());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        // Should be at limit now
        assert!(!limiter.can_proceed());
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn test_rate_limiter_per_second() {
        let limiter = RateLimiter::per_second(5);
        assert_eq!(limiter.max_requests(), 5);
        assert_eq!(limiter.window(), Duration::from_secs(1));
    }

    #[test]
    fn test_rate_limiter_per_minute() {
        let limiter = RateLimiter::per_minute(60);
        assert_eq!(limiter.max_requests(), 60);
        assert_eq!(limiter.window(), Duration::from_secs(60));
    }

    #[test]
    fn test_rate_limiter_current_count() {
        let limiter = RateLimiter::new(10, Duration::from_secs(1));

        assert_eq!(limiter.current_count(), 0);
        limiter.record_request();
        assert_eq!(limiter.current_count(), 1);
        limiter.record_request();
        assert_eq!(limiter.current_count(), 2);
    }

    #[test]
    fn test_rate_limiter_reset() {
        let limiter = RateLimiter::new(3, Duration::from_secs(1));

        limiter.record_request();
        limiter.record_request();
        assert_eq!(limiter.current_count(), 2);

        limiter.reset();
        assert_eq!(limiter.current_count(), 0);
    }

    #[test]
    fn test_rate_limiter_update_from_headers() {
        let limiter = RateLimiter::new(10, Duration::from_secs(1));

        limiter.update_from_headers(Some(5), Some(1000));
        assert_eq!(limiter.remaining(), Some(5));
    }

    #[test]
    fn test_composite_rate_limiter() {
        let limiter = CompositeRateLimiter::with_limits(2, 10);

        assert!(limiter.can_proceed());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        // Per-second limit reached
        assert!(!limiter.can_proceed());
    }

    #[test]
    fn test_composite_rate_limiter_reset() {
        let limiter = CompositeRateLimiter::with_limits(2, 10);

        limiter.try_acquire();
        limiter.try_acquire();
        assert!(!limiter.can_proceed());

        limiter.reset();
        assert!(limiter.can_proceed());
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire() {
        let limiter = RateLimiter::new(2, Duration::from_millis(100));

        limiter.acquire().await;
        limiter.acquire().await;
        // Third acquire should wait
        let start = Instant::now();
        limiter.acquire().await;
        let elapsed = start.elapsed();

        // Should have waited approximately 100ms
        assert!(elapsed >= Duration::from_millis(50));
    }
}
