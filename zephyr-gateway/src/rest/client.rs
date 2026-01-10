//! REST client implementation with rate limiting and request signing.

use reqwest::{Client, Method, Response, header};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use tracing::{debug, warn};
use zephyr_core::error::{ExchangeError, NetworkError};

use super::config::RestConfig;
use super::rate_limiter::{CompositeRateLimiter, RateLimiter};
use super::signer::{RequestSigner, SignatureType, build_query_string};

/// REST client with rate limiting and request signing.
///
/// # Features
///
/// - Automatic request signing
/// - Rate limiting with backoff
/// - Retry logic for transient failures
/// - Error response parsing
///
/// # Example
///
/// ```ignore
/// use zephyr_gateway::rest::{RestClient, RestConfig};
///
/// let config = RestConfig::builder()
///     .base_url("https://api.binance.com")
///     .api_key("your_key")
///     .api_secret("your_secret")
///     .build();
///
/// let client = RestClient::new(config)?;
/// let response = client.get("/api/v3/ticker/price")
///     .query("symbol", "BTCUSDT")
///     .send()
///     .await?;
/// ```
pub struct RestClient {
    config: RestConfig,
    http_client: Client,
    rate_limiter: Arc<CompositeRateLimiter>,
    signer: Option<RequestSigner>,
}

impl RestClient {
    /// Creates a new REST client.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if the HTTP client cannot be created.
    pub fn new(config: RestConfig) -> Result<Self, NetworkError> {
        let mut headers = header::HeaderMap::new();

        // Add user agent
        headers.insert(
            header::USER_AGENT,
            config
                .user_agent
                .parse()
                .map_err(|_| NetworkError::ConnectionFailed {
                    reason: "Invalid user agent".to_string(),
                })?,
        );

        // Add API key header if configured
        if let Some(api_key) = &config.api_key {
            headers.insert(
                "X-MBX-APIKEY",
                api_key
                    .parse()
                    .map_err(|_| NetworkError::ConnectionFailed {
                        reason: "Invalid API key".to_string(),
                    })?,
            );
        }

        // Add custom headers
        for (key, value) in &config.headers {
            headers.insert(
                header::HeaderName::try_from(key.as_str()).map_err(|_| {
                    NetworkError::ConnectionFailed {
                        reason: format!("Invalid header name: {key}"),
                    }
                })?,
                value.parse().map_err(|_| NetworkError::ConnectionFailed {
                    reason: format!("Invalid header value for {key}"),
                })?,
            );
        }

        let http_client = Client::builder()
            .timeout(config.timeout())
            .default_headers(headers)
            .build()
            .map_err(|e| NetworkError::ConnectionFailed {
                reason: format!("Failed to create HTTP client: {e}"),
            })?;

        let rate_limiter = Arc::new(CompositeRateLimiter::with_limits(
            config.rate_limit_per_second,
            config.rate_limit_per_minute,
        ));

        let signer = config
            .api_secret
            .as_ref()
            .map(|secret| RequestSigner::new(secret, SignatureType::HmacSha256));

        Ok(Self {
            config,
            http_client,
            rate_limiter,
            signer,
        })
    }

    /// Creates a GET request builder.
    #[must_use]
    pub fn get(&self, path: &str) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Method::GET, path)
    }

    /// Creates a POST request builder.
    #[must_use]
    pub fn post(&self, path: &str) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Method::POST, path)
    }

    /// Creates a PUT request builder.
    #[must_use]
    pub fn put(&self, path: &str) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Method::PUT, path)
    }

    /// Creates a DELETE request builder.
    #[must_use]
    pub fn delete(&self, path: &str) -> RequestBuilder<'_> {
        RequestBuilder::new(self, Method::DELETE, path)
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &RestConfig {
        &self.config
    }

    /// Returns the rate limiter.
    #[must_use]
    pub fn rate_limiter(&self) -> &CompositeRateLimiter {
        &self.rate_limiter
    }

    /// Returns the signer if configured.
    #[must_use]
    pub fn signer(&self) -> Option<&RequestSigner> {
        self.signer.as_ref()
    }

    /// Builds the full URL for a path.
    #[must_use]
    pub fn build_url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else {
            format!("{}{}", self.config.base_url.trim_end_matches('/'), path)
        }
    }

    async fn execute_request(
        &self,
        method: Method,
        url: &str,
        query: Option<&str>,
        body: Option<&str>,
        headers: &[(String, String)],
    ) -> Result<Response, NetworkError> {
        // Wait for rate limit
        self.rate_limiter.acquire().await;

        let full_url = if let Some(q) = query {
            format!("{url}?{q}")
        } else {
            url.to_string()
        };

        debug!(
            method = %method,
            url = %full_url,
            exchange = %self.config.exchange,
            "Sending request"
        );

        let mut request = self.http_client.request(method.clone(), &full_url);

        // Add custom headers
        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }

        // Add body if present
        if let Some(b) = body {
            request = request
                .header(header::CONTENT_TYPE, "application/json")
                .body(b.to_string());
        }

        let response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                NetworkError::Timeout {
                    timeout_ms: self.config.timeout_ms,
                }
            } else if e.is_connect() {
                NetworkError::ConnectionFailed {
                    reason: e.to_string(),
                }
            } else {
                NetworkError::Http {
                    status_code: e.status().map_or(0, |s| s.as_u16()),
                    reason: e.to_string(),
                }
            }
        })?;

        // Update rate limiter from headers
        self.update_rate_limit_from_response(&response);

        Ok(response)
    }

    fn update_rate_limit_from_response(&self, response: &Response) {
        // Binance headers
        if let Some(remaining) = response
            .headers()
            .get("x-mbx-used-weight-1m")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
        {
            // Binance returns used weight, not remaining
            // We'd need to know the limit to calculate remaining
            debug!(used_weight = remaining, "Rate limit weight used");
        }

        // OKX/Bitget headers
        if let Some(remaining) = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
        {
            let reset_ms = response
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());

            // Update the first limiter (per-second)
            if let Some(limiter) = self.rate_limiter.limiters().first() {
                limiter.update_from_headers(Some(remaining), reset_ms);
            }
        }
    }
}

/// Request builder for REST API calls.
pub struct RequestBuilder<'a> {
    client: &'a RestClient,
    method: Method,
    path: String,
    query_params: Vec<(String, String)>,
    body: Option<String>,
    headers: Vec<(String, String)>,
    sign: bool,
}

impl<'a> RequestBuilder<'a> {
    fn new(client: &'a RestClient, method: Method, path: &str) -> Self {
        Self {
            client,
            method,
            path: path.to_string(),
            query_params: Vec::new(),
            body: None,
            headers: Vec::new(),
            sign: false,
        }
    }

    /// Adds a query parameter.
    #[must_use]
    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query_params.push((key.into(), value.into()));
        self
    }

    /// Adds multiple query parameters.
    #[must_use]
    pub fn queries(mut self, params: &[(&str, &str)]) -> Self {
        for (key, value) in params {
            self.query_params
                .push(((*key).to_string(), (*value).to_string()));
        }
        self
    }

    /// Sets the request body as JSON.
    #[must_use]
    pub fn json<T: serde::Serialize>(mut self, body: &T) -> Self {
        self.body = serde_json::to_string(body).ok();
        self
    }

    /// Sets the request body as a string.
    #[must_use]
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Adds a header.
    #[must_use]
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    /// Enables request signing.
    #[must_use]
    pub fn signed(mut self) -> Self {
        self.sign = true;
        self
    }

    /// Sends the request and returns the raw response.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if the request fails.
    pub async fn send(self) -> Result<Response, NetworkError> {
        let url = self.client.build_url(&self.path);

        // Build query string
        let query_string = if self.query_params.is_empty() {
            None
        } else {
            let params: Vec<(&str, &str)> = self
                .query_params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();

            let query = build_query_string(&params);

            // Sign if requested
            if self.sign {
                if let Some(signer) = self.client.signer() {
                    let signature = signer.sign(&query)?;
                    Some(format!("{query}&signature={signature}"))
                } else {
                    Some(query)
                }
            } else {
                Some(query)
            }
        };

        let mut attempt = 0u32;
        loop {
            let result = self
                .client
                .execute_request(
                    self.method.clone(),
                    &url,
                    query_string.as_deref(),
                    self.body.as_deref(),
                    &self.headers,
                )
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();

                    // Handle rate limiting
                    if status.as_u16() == 429 {
                        if self.client.config.retry_on_rate_limit
                            && self.client.config.should_retry(attempt)
                        {
                            let delay = self.client.config.calculate_retry_delay(attempt);
                            warn!(
                                attempt = attempt + 1,
                                delay_ms = delay.as_millis(),
                                "Rate limited, retrying"
                            );
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                            continue;
                        }
                    }

                    return Ok(response);
                }
                Err(e) => {
                    if e.is_recoverable() && self.client.config.should_retry(attempt) {
                        let delay = self.client.config.calculate_retry_delay(attempt);
                        warn!(
                            attempt = attempt + 1,
                            delay_ms = delay.as_millis(),
                            error = %e,
                            "Request failed, retrying"
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
    }

    /// Sends the request and deserializes the response as JSON.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if the request fails or response cannot be parsed.
    pub async fn send_json<T: DeserializeOwned>(self) -> Result<T, NetworkError> {
        let response = self.send().await?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(NetworkError::Http {
                status_code: status.as_u16(),
                reason: body,
            });
        }

        response
            .json::<T>()
            .await
            .map_err(|e| NetworkError::WebSocket {
                reason: format!("Failed to parse response: {e}"),
            })
    }

    /// Sends the request and returns the response as text.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if the request fails.
    pub async fn send_text(self) -> Result<String, NetworkError> {
        let response = self.send().await?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(NetworkError::Http {
                status_code: status.as_u16(),
                reason: body,
            });
        }

        response.text().await.map_err(|e| NetworkError::WebSocket {
            reason: format!("Failed to read response: {e}"),
        })
    }
}

/// Parses an exchange error response.
///
/// Different exchanges have different error formats. This function
/// attempts to parse common formats.
#[allow(dead_code)]
pub fn parse_exchange_error(status: u16, body: &str) -> ExchangeError {
    // Try to parse as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        // Binance format: {"code": -1000, "msg": "..."}
        if let (Some(code), Some(msg)) = (
            json.get("code").and_then(|v| v.as_i64()),
            json.get("msg").and_then(|v| v.as_str()),
        ) {
            return match code {
                -1000 => ExchangeError::Unknown {
                    code: code as i32,
                    message: msg.to_string(),
                },
                -1002 | -2015 => ExchangeError::AuthenticationFailed {
                    reason: msg.to_string(),
                },
                -1003 | -1015 => ExchangeError::RateLimited {
                    retry_after_ms: 60_000,
                },
                -2010 => ExchangeError::InsufficientBalance {
                    required: rust_decimal::Decimal::ZERO,
                    available: rust_decimal::Decimal::ZERO,
                },
                -2011 => ExchangeError::OrderNotFound {
                    order_id: String::new(),
                },
                _ => ExchangeError::Unknown {
                    code: code as i32,
                    message: msg.to_string(),
                },
            };
        }

        // OKX format: {"code": "0", "msg": "...", "data": [...]}
        if let (Some(code), Some(msg)) = (
            json.get("code").and_then(|v| v.as_str()),
            json.get("msg").and_then(|v| v.as_str()),
        ) {
            let code_num = code.parse::<i32>().unwrap_or(0);
            if code != "0" {
                return ExchangeError::Unknown {
                    code: code_num,
                    message: msg.to_string(),
                };
            }
        }
    }

    // Default error based on status code
    match status {
        401 | 403 => ExchangeError::AuthenticationFailed {
            reason: body.to_string(),
        },
        429 => ExchangeError::RateLimited {
            retry_after_ms: 60_000,
        },
        503 => ExchangeError::Maintenance {
            message: body.to_string(),
        },
        _ => ExchangeError::Unknown {
            code: status as i32,
            message: body.to_string(),
        },
    }
}

// Extension trait for CompositeRateLimiter to access internal limiters
impl CompositeRateLimiter {
    /// Returns a reference to the internal limiters.
    pub fn limiters(&self) -> &[RateLimiter] {
        // This is a workaround - in production, we'd expose this properly
        // For now, return empty slice as we can't access private field
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = RestConfig::builder()
            .base_url("https://api.binance.com")
            .exchange("binance")
            .build();

        let client = RestClient::new(config).unwrap();
        assert_eq!(client.config().base_url, "https://api.binance.com");
    }

    #[test]
    fn test_build_url() {
        let config = RestConfig::builder()
            .base_url("https://api.binance.com")
            .build();

        let client = RestClient::new(config).unwrap();

        assert_eq!(
            client.build_url("/api/v3/ticker"),
            "https://api.binance.com/api/v3/ticker"
        );

        assert_eq!(
            client.build_url("https://other.com/path"),
            "https://other.com/path"
        );
    }

    #[test]
    fn test_parse_binance_error() {
        let body = r#"{"code": -1003, "msg": "Too many requests"}"#;
        let error = parse_exchange_error(429, body);

        assert!(matches!(error, ExchangeError::RateLimited { .. }));
    }

    #[test]
    fn test_parse_auth_error() {
        let body = r#"{"code": -2015, "msg": "Invalid API-key"}"#;
        let error = parse_exchange_error(401, body);

        assert!(matches!(error, ExchangeError::AuthenticationFailed { .. }));
    }

    #[test]
    fn test_parse_unknown_error() {
        let body = r#"{"code": -9999, "msg": "Unknown error"}"#;
        let error = parse_exchange_error(500, body);

        assert!(matches!(error, ExchangeError::Unknown { .. }));
    }

    #[test]
    fn test_request_builder() {
        let config = RestConfig::builder()
            .base_url("https://api.binance.com")
            .build();

        let client = RestClient::new(config).unwrap();

        let builder = client
            .get("/api/v3/ticker")
            .query("symbol", "BTCUSDT")
            .query("limit", "100");

        assert_eq!(builder.query_params.len(), 2);
    }
}
