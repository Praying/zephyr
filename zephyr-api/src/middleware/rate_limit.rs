//! Rate limiting middleware.

use axum::{
    Json,
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use parking_lot::Mutex;
use std::{
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::config::RateLimitConfig;
use crate::error::ErrorResponse;

/// Rate limiter using token bucket algorithm.
#[derive(Debug)]
pub struct RateLimiter {
    /// Configuration
    config: RateLimitConfig,
    /// Buckets per client (keyed by IP or API key)
    buckets: DashMap<String, TokenBucket>,
}

impl RateLimiter {
    /// Creates a new rate limiter.
    #[must_use]
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: DashMap::new(),
        }
    }

    /// Checks if a request is allowed for the given client key.
    pub fn check(&self, client_key: &str) -> RateLimitResult {
        if !self.config.enabled {
            return RateLimitResult::Allowed {
                remaining: u32::MAX,
                reset_at: Instant::now(),
            };
        }

        let mut bucket = self
            .buckets
            .entry(client_key.to_string())
            .or_insert_with(|| {
                TokenBucket::new(
                    self.config.max_requests + self.config.burst,
                    self.config.max_requests,
                    self.config.window(),
                )
            });

        bucket.try_acquire()
    }

    /// Cleans up expired buckets.
    pub fn cleanup(&self) {
        let now = Instant::now();
        let window = self.config.window();

        self.buckets
            .retain(|_, bucket| now.duration_since(bucket.last_update()) < window * 2);
    }
}

/// Result of a rate limit check.
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// Request is allowed
    Allowed {
        /// Remaining requests in the window
        remaining: u32,
        /// When the window resets
        reset_at: Instant,
    },
    /// Request is denied
    Denied {
        /// When the client can retry
        retry_after: Duration,
    },
}

/// Token bucket for rate limiting.
#[derive(Debug)]
struct TokenBucket {
    /// Current number of tokens
    tokens: Mutex<f64>,
    /// Maximum tokens (capacity)
    capacity: u32,
    /// Refill rate (tokens per window)
    refill_rate: u32,
    /// Window duration
    window: Duration,
    /// Last update time
    last_update: Mutex<Instant>,
}

impl TokenBucket {
    fn new(capacity: u32, refill_rate: u32, window: Duration) -> Self {
        Self {
            tokens: Mutex::new(f64::from(capacity)),
            capacity,
            refill_rate,
            window,
            last_update: Mutex::new(Instant::now()),
        }
    }

    fn try_acquire(&mut self) -> RateLimitResult {
        let now = Instant::now();
        let mut tokens = self.tokens.lock();
        let mut last_update = self.last_update.lock();

        // Refill tokens based on elapsed time
        let elapsed = now.duration_since(*last_update);
        let refill =
            (elapsed.as_secs_f64() / self.window.as_secs_f64()) * f64::from(self.refill_rate);
        *tokens = (*tokens + refill).min(f64::from(self.capacity));
        *last_update = now;
        drop(last_update);

        if *tokens >= 1.0 {
            *tokens -= 1.0;
            let reset_at = now + self.window;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let remaining = (tokens.max(0.0) as u32).min(self.capacity);
            drop(tokens);
            RateLimitResult::Allowed {
                remaining,
                reset_at,
            }
        } else {
            // Calculate retry after
            let tokens_needed = 1.0 - *tokens;
            let time_needed =
                (tokens_needed / f64::from(self.refill_rate)) * self.window.as_secs_f64();
            drop(tokens);
            RateLimitResult::Denied {
                retry_after: Duration::from_secs_f64(time_needed),
            }
        }
    }

    fn last_update(&self) -> Instant {
        *self.last_update.lock()
    }
}

/// Rate limit layer for Axum.
#[derive(Clone)]
#[allow(dead_code)]
pub struct RateLimitLayer {
    limiter: Arc<RateLimiter>,
}

#[allow(dead_code)]
impl RateLimitLayer {
    /// Creates a new rate limit layer.
    #[must_use]
    pub fn new(limiter: Arc<RateLimiter>) -> Self {
        Self { limiter }
    }
}

/// Rate limit middleware function.
#[allow(dead_code)]
pub async fn rate_limit_middleware(
    limiter: Arc<RateLimiter>,
    request: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    // Extract client key (prefer API key, fall back to IP)
    let client_key = extract_client_key(&request);

    match limiter.check(&client_key) {
        RateLimitResult::Allowed {
            remaining,
            reset_at,
        } => {
            let mut response = next.run(request).await;

            // Add rate limit headers
            let headers = response.headers_mut();
            headers.insert(
                "X-RateLimit-Remaining",
                remaining.to_string().parse().unwrap(),
            );
            headers.insert(
                "X-RateLimit-Reset",
                reset_at
                    .duration_since(Instant::now())
                    .as_secs()
                    .to_string()
                    .parse()
                    .unwrap(),
            );

            response
        }
        RateLimitResult::Denied { retry_after } => rate_limit_exceeded_response(retry_after),
    }
}

/// Extracts the client key from the request.
#[allow(dead_code)]
fn extract_client_key(request: &Request<Body>) -> String {
    // Try to get API key from header
    if let Some(api_key) = request
        .headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
    {
        return format!("api:{api_key}");
    }

    // Fall back to IP address
    // In production, this should handle X-Forwarded-For for proxied requests
    if let Some(ip) = request
        .headers()
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
    {
        return format!("ip:{ip}");
    }

    // Default fallback
    "unknown".to_string()
}

/// Creates a rate limit exceeded response.
#[allow(dead_code)]
fn rate_limit_exceeded_response(retry_after: Duration) -> Response {
    let body = ErrorResponse {
        status: "error",
        code: "RATE_LIMIT_EXCEEDED",
        message: format!(
            "Rate limit exceeded. Retry after {} seconds.",
            retry_after.as_secs()
        ),
        request_id: None,
    };

    let mut response = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
    response.headers_mut().insert(
        "Retry-After",
        retry_after.as_secs().to_string().parse().unwrap(),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RateLimitConfig {
        RateLimitConfig {
            enabled: true,
            max_requests: 10,
            window_secs: 60,
            burst: 5,
        }
    }

    #[test]
    fn test_rate_limiter_allows_requests() {
        let limiter = RateLimiter::new(test_config());

        // First request should be allowed
        let result = limiter.check("client1");
        assert!(matches!(result, RateLimitResult::Allowed { .. }));
    }

    #[test]
    fn test_rate_limiter_exhausts_tokens() {
        let limiter = RateLimiter::new(RateLimitConfig {
            enabled: true,
            max_requests: 2,
            window_secs: 60,
            burst: 0,
        });

        // Use up all tokens
        let _ = limiter.check("client1");
        let _ = limiter.check("client1");

        // Third request should be denied
        let result = limiter.check("client1");
        assert!(matches!(result, RateLimitResult::Denied { .. }));
    }

    #[test]
    fn test_rate_limiter_disabled() {
        let limiter = RateLimiter::new(RateLimitConfig {
            enabled: false,
            ..test_config()
        });

        // Should always allow when disabled
        for _ in 0..100 {
            let result = limiter.check("client1");
            assert!(matches!(result, RateLimitResult::Allowed { .. }));
        }
    }

    #[test]
    fn test_rate_limiter_separate_clients() {
        let limiter = RateLimiter::new(RateLimitConfig {
            enabled: true,
            max_requests: 1,
            window_secs: 60,
            burst: 0,
        });

        // Each client has their own bucket
        let result1 = limiter.check("client1");
        let result2 = limiter.check("client2");

        assert!(matches!(result1, RateLimitResult::Allowed { .. }));
        assert!(matches!(result2, RateLimitResult::Allowed { .. }));
    }
}
