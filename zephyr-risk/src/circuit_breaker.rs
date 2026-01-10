//! Circuit breaker implementation for fault tolerance.
//!
//! The circuit breaker pattern prevents cascading failures by temporarily
//! blocking requests to a failing service.
//!
//! # States
//!
//! - **Closed**: Normal operation, requests pass through
//! - **Open**: Requests are blocked, waiting for recovery
//! - **`HalfOpen`**: Testing if the service has recovered
//!
//! # Example
//!
//! ```
//! use zephyr_risk::{CircuitBreaker, CircuitBreakerConfig};
//! use std::time::Duration;
//!
//! let config = CircuitBreakerConfig {
//!     failure_threshold: 5,
//!     success_threshold: 2,
//!     timeout: Duration::from_secs(30),
//!     half_open_max_calls: 3,
//! };
//!
//! let breaker = CircuitBreaker::new("binance-api", config);
//!
//! // Record failures
//! for _ in 0..5 {
//!     breaker.record_failure();
//! }
//!
//! // Circuit should now be open
//! assert!(!breaker.allow_request());
//! ```

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Normal operation - requests pass through.
    Closed,
    /// Blocking requests - service is considered down.
    Open,
    /// Testing recovery - limited requests allowed.
    HalfOpen,
}

impl CircuitState {
    /// Returns true if the circuit is closed (normal operation).
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        matches!(self, Self::Closed)
    }

    /// Returns true if the circuit is open (blocking requests).
    #[must_use]
    pub const fn is_open(&self) -> bool {
        matches!(self, Self::Open)
    }

    /// Returns true if the circuit is half-open (testing recovery).
    #[must_use]
    pub const fn is_half_open(&self) -> bool {
        matches!(self, Self::HalfOpen)
    }
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "CLOSED"),
            Self::Open => write!(f, "OPEN"),
            Self::HalfOpen => write!(f, "HALF_OPEN"),
        }
    }
}

/// Configuration for a circuit breaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit.
    pub failure_threshold: u32,
    /// Number of successes in half-open state to close the circuit.
    pub success_threshold: u32,
    /// Time to wait before transitioning from open to half-open.
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
    /// Maximum concurrent calls allowed in half-open state.
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(30),
            half_open_max_calls: 3,
        }
    }
}

impl CircuitBreakerConfig {
    /// Creates a new configuration with the given failure threshold.
    #[must_use]
    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Sets the success threshold for recovery.
    #[must_use]
    pub fn with_success_threshold(mut self, threshold: u32) -> Self {
        self.success_threshold = threshold;
        self
    }

    /// Sets the timeout duration.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Internal state for the circuit breaker.
#[derive(Debug)]
struct CircuitBreakerState {
    /// Current state of the circuit.
    state: CircuitState,
    /// Number of consecutive failures.
    failure_count: u32,
    /// Number of consecutive successes (in half-open state).
    success_count: u32,
    /// Time when the circuit was opened.
    opened_at: Option<Instant>,
    /// Number of calls in half-open state.
    half_open_calls: u32,
    /// Last failure reason.
    last_failure_reason: Option<String>,
}

impl Default for CircuitBreakerState {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            opened_at: None,
            half_open_calls: 0,
            last_failure_reason: None,
        }
    }
}

/// Circuit breaker for protecting against cascading failures.
///
/// The circuit breaker monitors failures and temporarily blocks requests
/// when a service is failing, allowing it time to recover.
pub struct CircuitBreaker {
    /// Name of the service being protected.
    name: String,
    /// Configuration.
    config: CircuitBreakerConfig,
    /// Internal state.
    state: RwLock<CircuitBreakerState>,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker with the given name and configuration.
    #[must_use]
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            config,
            state: RwLock::new(CircuitBreakerState::default()),
        }
    }

    /// Creates a new circuit breaker with default configuration.
    #[must_use]
    pub fn with_defaults(name: impl Into<String>) -> Self {
        Self::new(name, CircuitBreakerConfig::default())
    }

    /// Returns the name of the service.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the current state of the circuit.
    #[must_use]
    pub fn state(&self) -> CircuitState {
        self.maybe_transition_to_half_open();
        self.state.read().state
    }

    /// Returns true if requests should be allowed.
    #[must_use]
    pub fn allow_request(&self) -> bool {
        self.maybe_transition_to_half_open();

        let mut state = self.state.write();
        match state.state {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => {
                if state.half_open_calls < self.config.half_open_max_calls {
                    state.half_open_calls += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Records a successful request.
    pub fn record_success(&self) {
        let mut state = self.state.write();

        match state.state {
            CircuitState::Closed => {
                // Reset failure count on success
                state.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                state.success_count += 1;
                debug!(
                    service = %self.name,
                    success_count = state.success_count,
                    threshold = self.config.success_threshold,
                    "Circuit breaker success in half-open state"
                );

                if state.success_count >= self.config.success_threshold {
                    info!(service = %self.name, "Circuit breaker closed - service recovered");
                    state.state = CircuitState::Closed;
                    state.failure_count = 0;
                    state.success_count = 0;
                    state.half_open_calls = 0;
                    state.opened_at = None;
                    state.last_failure_reason = None;
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but ignore
            }
        }
    }

    /// Records a failed request.
    pub fn record_failure(&self) {
        self.record_failure_with_reason(None);
    }

    /// Records a failed request with a reason.
    pub fn record_failure_with_reason(&self, reason: Option<String>) {
        let mut state = self.state.write();

        match state.state {
            CircuitState::Closed => {
                state.failure_count += 1;
                if let Some(r) = &reason {
                    state.last_failure_reason.clone_from(&Some(r.clone()));
                } else {
                    state.last_failure_reason = None;
                }
                debug!(
                    service = %self.name,
                    failure_count = state.failure_count,
                    threshold = self.config.failure_threshold,
                    reason = ?reason,
                    "Circuit breaker failure recorded"
                );

                if state.failure_count >= self.config.failure_threshold {
                    warn!(
                        service = %self.name,
                        failures = state.failure_count,
                        reason = ?state.last_failure_reason,
                        "Circuit breaker opened"
                    );
                    state.state = CircuitState::Open;
                    state.opened_at = Some(Instant::now());
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state reopens the circuit
                warn!(
                    service = %self.name,
                    reason = ?reason,
                    "Circuit breaker reopened from half-open state"
                );
                state.state = CircuitState::Open;
                state.opened_at = Some(Instant::now());
                state.success_count = 0;
                state.half_open_calls = 0;
                state.last_failure_reason = reason;
            }
            CircuitState::Open => {
                // Already open, just update the reason
                state.last_failure_reason = reason;
            }
        }
    }

    /// Manually resets the circuit breaker to closed state.
    pub fn reset(&self) {
        let mut state = self.state.write();
        info!(service = %self.name, "Circuit breaker manually reset");
        state.state = CircuitState::Closed;
        state.failure_count = 0;
        state.success_count = 0;
        state.half_open_calls = 0;
        state.opened_at = None;
        state.last_failure_reason = None;
    }

    /// Returns the last failure reason, if any.
    #[must_use]
    pub fn last_failure_reason(&self) -> Option<String> {
        self.state.read().last_failure_reason.clone()
    }

    /// Returns the current failure count.
    #[must_use]
    pub fn failure_count(&self) -> u32 {
        self.state.read().failure_count
    }

    /// Returns the time remaining until the circuit transitions to half-open.
    #[must_use]
    pub fn time_until_half_open(&self) -> Option<Duration> {
        let state = self.state.read();
        if state.state != CircuitState::Open {
            return None;
        }

        state.opened_at.map(|opened| {
            let elapsed = opened.elapsed();
            if elapsed >= self.config.timeout {
                Duration::ZERO
            } else {
                self.config.timeout.saturating_sub(elapsed)
            }
        })
    }

    /// Checks if the circuit should transition from open to half-open.
    fn maybe_transition_to_half_open(&self) {
        let mut state = self.state.write();
        if state.state != CircuitState::Open {
            return;
        }

        if let Some(opened_at) = state.opened_at
            && opened_at.elapsed() >= self.config.timeout
        {
            info!(
                service = %self.name,
                "Circuit breaker transitioning to half-open"
            );
            state.state = CircuitState::HalfOpen;
            state.success_count = 0;
            state.half_open_calls = 0;
        }
    }
}

impl std::fmt::Debug for CircuitBreaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitBreaker")
            .field("name", &self.name)
            .field("state", &self.state())
            .field("failure_count", &self.failure_count())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let breaker = CircuitBreaker::with_defaults("test");
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.allow_request());
    }

    #[test]
    fn test_circuit_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", config);

        // Record failures
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Closed);

        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Closed);

        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(!breaker.allow_request());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", config);

        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.failure_count(), 2);

        breaker.record_success();
        assert_eq!(breaker.failure_count(), 0);
    }

    #[test]
    fn test_circuit_transitions_to_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(50),
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", config);

        // Open the circuit
        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);

        // Wait for timeout
        sleep(Duration::from_millis(60));

        // Should transition to half-open
        assert_eq!(breaker.state(), CircuitState::HalfOpen);
        assert!(breaker.allow_request());
    }

    #[test]
    fn test_half_open_closes_on_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(10),
            half_open_max_calls: 5,
        };
        let breaker = CircuitBreaker::new("test", config);

        // Open the circuit
        breaker.record_failure();
        breaker.record_failure();

        // Wait for half-open
        sleep(Duration::from_millis(15));
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        // Record successes
        breaker.record_success();
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        breaker.record_success();
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_reopens_on_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(10),
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", config);

        // Open the circuit
        breaker.record_failure();
        breaker.record_failure();

        // Wait for half-open
        sleep(Duration::from_millis(15));
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        // Failure should reopen
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
    }

    #[test]
    fn test_half_open_limits_calls() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(10),
            half_open_max_calls: 2,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", config);

        // Open the circuit
        breaker.record_failure();
        breaker.record_failure();

        // Wait for half-open
        sleep(Duration::from_millis(15));

        // Should allow limited calls
        assert!(breaker.allow_request());
        assert!(breaker.allow_request());
        assert!(!breaker.allow_request()); // Exceeded limit
    }

    #[test]
    fn test_manual_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", config);

        // Open the circuit
        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);

        // Manual reset
        breaker.reset();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.allow_request());
    }

    #[test]
    fn test_failure_reason() {
        let breaker = CircuitBreaker::with_defaults("test");

        breaker.record_failure_with_reason(Some("Connection timeout".to_string()));
        assert_eq!(
            breaker.last_failure_reason(),
            Some("Connection timeout".to_string())
        );
    }

    #[test]
    fn test_time_until_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_secs(30),
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", config);

        // Not open yet
        assert!(breaker.time_until_half_open().is_none());

        // Open the circuit
        breaker.record_failure();
        breaker.record_failure();

        // Should have time remaining
        let remaining = breaker.time_until_half_open().unwrap();
        assert!(remaining > Duration::from_secs(29));
        assert!(remaining <= Duration::from_secs(30));
    }
}
