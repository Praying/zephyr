//! Error context and contextual error wrapper.
//!
//! This module provides structures for capturing and preserving error context,
//! including timestamp, component, operation, and additional metadata.

// Allow HashMap in this module - it's used for single-threaded error context storage
#![allow(clippy::disallowed_types)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write as _;

use crate::types::Timestamp;

use super::ErrorSeverity;
use super::ZephyrError;

/// Error context capturing metadata about when and where an error occurred.
///
/// # Examples
///
/// ```
/// use zephyr_core::error::ErrorContext;
///
/// let context = ErrorContext::new("TraderGateway", "order_insert")
///     .with_request_id("req-123")
///     .with_additional("symbol", "BTC-USDT");
///
/// assert_eq!(context.component(), "TraderGateway");
/// assert_eq!(context.operation(), "order_insert");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]

pub struct ErrorContext {
    /// Timestamp when the error occurred.
    timestamp: Timestamp,
    /// Component where the error originated.
    component: String,
    /// Operation being performed when the error occurred.
    operation: String,
    /// Optional request ID for tracing.
    request_id: Option<String>,
    /// Additional context data as key-value pairs.
    additional: HashMap<String, String>,
}

impl ErrorContext {
    /// Creates a new `ErrorContext` with the current timestamp.
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::error::ErrorContext;
    ///
    /// let context = ErrorContext::new("Engine", "process_tick");
    /// assert_eq!(context.component(), "Engine");
    /// ```
    #[must_use]
    pub fn new(component: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            timestamp: Timestamp::now(),
            component: component.into(),
            operation: operation.into(),
            request_id: None,
            additional: HashMap::new(),
        }
    }

    /// Creates a new `ErrorContext` with a specific timestamp.
    #[must_use]
    pub fn with_timestamp(
        timestamp: Timestamp,
        component: impl Into<String>,
        operation: impl Into<String>,
    ) -> Self {
        Self {
            timestamp,
            component: component.into(),
            operation: operation.into(),
            request_id: None,
            additional: HashMap::new(),
        }
    }

    /// Adds a request ID to the context.
    #[must_use]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Adds additional context data.
    #[must_use]
    pub fn with_additional(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.additional.insert(key.into(), value.into());
        self
    }

    /// Returns the timestamp when the error occurred.
    #[must_use]
    pub const fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Returns the component where the error originated.
    #[must_use]
    pub fn component(&self) -> &str {
        &self.component
    }

    /// Returns the operation being performed when the error occurred.
    #[must_use]
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// Returns the request ID if available.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    /// Returns the additional context data.
    #[must_use]
    pub fn additional(&self) -> &HashMap<String, String> {
        &self.additional
    }

    /// Gets a specific additional context value.
    #[must_use]
    pub fn get_additional(&self, key: &str) -> Option<&str> {
        self.additional.get(key).map(String::as_str)
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}.{}",
            self.timestamp, self.component, self.operation
        )?;
        if let Some(ref req_id) = self.request_id {
            write!(f, " (request_id={req_id})")?;
        }
        if !self.additional.is_empty() {
            write!(f, " {{")?;
            for (i, (k, v)) in self.additional.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{k}={v}")?;
            }
            write!(f, "}}")?;
        }
        Ok(())
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new("unknown", "unknown")
    }
}

/// A wrapper that combines an error with its context.
///
/// This type preserves the full error chain while adding contextual information
/// about when and where the error occurred.
///
/// # Examples
///
/// ```
/// use zephyr_core::error::{ContextualError, ErrorContext, NetworkError, ZephyrError};
///
/// let error = NetworkError::Timeout { timeout_ms: 5000 };
/// let context = ErrorContext::new("BinanceGateway", "connect");
/// let contextual = ContextualError::new(ZephyrError::Network(error), context);
///
/// assert!(contextual.to_string().contains("BinanceGateway"));
/// assert!(contextual.to_string().contains("5000ms"));
/// ```
#[derive(Debug, Clone)]
pub struct ContextualError {
    /// The underlying error.
    error: ZephyrError,
    /// Context information about the error.
    context: ErrorContext,
    /// Optional cause of this error (for error chaining).
    cause_description: Option<String>,
}

impl ContextualError {
    /// Creates a new `ContextualError` with the given error and context.
    #[must_use]
    pub fn new(error: ZephyrError, context: ErrorContext) -> Self {
        Self {
            error,
            context,
            cause_description: None,
        }
    }

    /// Creates a `ContextualError` with a cause description.
    #[must_use]
    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause_description = Some(cause.into());
        self
    }

    /// Returns a reference to the underlying error.
    #[must_use]
    pub const fn error(&self) -> &ZephyrError {
        &self.error
    }

    /// Returns a reference to the error context.
    #[must_use]
    pub const fn context(&self) -> &ErrorContext {
        &self.context
    }

    /// Returns the cause description if available.
    #[must_use]
    pub fn cause_description(&self) -> Option<&str> {
        self.cause_description.as_deref()
    }

    /// Returns the error type as a string.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match &self.error {
            ZephyrError::Network(_) => "NetworkError",
            ZephyrError::Exchange(_) => "ExchangeError",
            ZephyrError::Data(_) => "DataError",
            ZephyrError::Strategy(_) => "StrategyError",
            ZephyrError::Config(_) => "ConfigError",
            ZephyrError::Storage(_) => "StorageError",
        }
    }

    /// Returns the severity level of the error.
    #[must_use]
    pub fn severity(&self) -> ErrorSeverity {
        self.error.severity()
    }

    /// Converts the contextual error to a log-friendly string format.
    ///
    /// Format: `[SEVERITY] [timestamp] component.operation: error_type - message`
    #[must_use]
    pub fn to_log_string(&self) -> String {
        let mut result = format!(
            "[{}] [{}] {}.{}: {} - {}",
            self.severity(),
            self.context.timestamp,
            self.context.component,
            self.context.operation,
            self.error_type(),
            self.error
        );

        if let Some(ref req_id) = self.context.request_id {
            let _ = write!(result, " (request_id={req_id})");
        }

        if let Some(ref cause) = self.cause_description {
            let _ = write!(result, " caused by: {cause}");
        }

        if !self.context.additional.is_empty() {
            result.push_str(" {");
            for (i, (k, v)) in self.context.additional.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                let _ = write!(result, "{k}={v}");
            }
            result.push('}');
        }

        result
    }

    /// Consumes the contextual error and returns the underlying error.
    #[must_use]
    pub fn into_inner(self) -> ZephyrError {
        self.error
    }
}

impl fmt::Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}", self.error, self.context)?;
        if let Some(ref cause) = self.cause_description {
            write!(f, " (caused by: {cause})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ContextualError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl From<ContextualError> for ZephyrError {
    fn from(contextual: ContextualError) -> Self {
        contextual.error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NetworkError;

    #[test]
    fn test_error_severity_display() {
        assert_eq!(ErrorSeverity::Fatal.to_string(), "FATAL");
        assert_eq!(ErrorSeverity::Recoverable.to_string(), "RECOVERABLE");
        assert_eq!(ErrorSeverity::Warning.to_string(), "WARNING");
        assert_eq!(ErrorSeverity::Info.to_string(), "INFO");
    }

    #[test]
    fn test_error_severity_is_recoverable() {
        assert!(!ErrorSeverity::Fatal.is_recoverable());
        assert!(ErrorSeverity::Recoverable.is_recoverable());
        assert!(ErrorSeverity::Warning.is_recoverable());
        assert!(ErrorSeverity::Info.is_recoverable());
    }

    #[test]
    fn test_error_severity_is_fatal() {
        assert!(ErrorSeverity::Fatal.is_fatal());
        assert!(!ErrorSeverity::Recoverable.is_fatal());
        assert!(!ErrorSeverity::Warning.is_fatal());
        assert!(!ErrorSeverity::Info.is_fatal());
    }

    #[test]
    fn test_error_context_new() {
        let context = ErrorContext::new("TestComponent", "test_operation");
        assert_eq!(context.component(), "TestComponent");
        assert_eq!(context.operation(), "test_operation");
        assert!(context.request_id().is_none());
        assert!(context.additional().is_empty());
    }

    #[test]
    fn test_error_context_with_request_id() {
        let context = ErrorContext::new("Gateway", "connect").with_request_id("req-12345");
        assert_eq!(context.request_id(), Some("req-12345"));
    }

    #[test]
    fn test_error_context_with_additional() {
        let context = ErrorContext::new("Gateway", "connect")
            .with_additional("symbol", "BTC-USDT")
            .with_additional("exchange", "binance");
        assert_eq!(context.get_additional("symbol"), Some("BTC-USDT"));
        assert_eq!(context.get_additional("exchange"), Some("binance"));
        assert_eq!(context.get_additional("missing"), None);
    }

    #[test]
    fn test_error_context_display() {
        let context = ErrorContext::new("Gateway", "connect")
            .with_request_id("req-123")
            .with_additional("symbol", "BTC-USDT");
        let display = context.to_string();
        assert!(display.contains("Gateway.connect"));
        assert!(display.contains("req-123"));
        assert!(display.contains("symbol=BTC-USDT"));
    }

    #[test]
    fn test_error_context_serde_roundtrip() {
        let context = ErrorContext::new("Engine", "process")
            .with_request_id("req-456")
            .with_additional("key", "value");
        let json = serde_json::to_string(&context).unwrap();
        let parsed: ErrorContext = serde_json::from_str(&json).unwrap();
        assert_eq!(context.component(), parsed.component());
        assert_eq!(context.operation(), parsed.operation());
        assert_eq!(context.request_id(), parsed.request_id());
    }

    #[test]
    fn test_contextual_error_new() {
        let error = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        let context = ErrorContext::new("BinanceGateway", "connect");
        let contextual = ContextualError::new(error, context);

        assert_eq!(contextual.error_type(), "NetworkError");
        assert_eq!(contextual.context().component(), "BinanceGateway");
        assert_eq!(contextual.severity(), ErrorSeverity::Recoverable);
    }

    #[test]
    fn test_contextual_error_with_cause() {
        let error = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        let context = ErrorContext::new("Gateway", "connect");
        let contextual = ContextualError::new(error, context).with_cause("DNS lookup failed");

        assert_eq!(contextual.cause_description(), Some("DNS lookup failed"));
    }

    #[test]
    fn test_contextual_error_display() {
        let error = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        let context = ErrorContext::new("Gateway", "connect");
        let contextual = ContextualError::new(error, context);
        let display = contextual.to_string();

        assert!(display.contains("5000ms"));
        assert!(display.contains("Gateway.connect"));
    }

    #[test]
    fn test_contextual_error_to_log_string() {
        let error = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        let context = ErrorContext::new("Gateway", "connect")
            .with_request_id("req-789")
            .with_additional("exchange", "binance");
        let contextual = ContextualError::new(error, context).with_cause("Network unreachable");
        let log_str = contextual.to_log_string();

        assert!(log_str.contains("[RECOVERABLE]"));
        assert!(log_str.contains("Gateway.connect"));
        assert!(log_str.contains("NetworkError"));
        assert!(log_str.contains("req-789"));
        assert!(log_str.contains("exchange=binance"));
        assert!(log_str.contains("caused by: Network unreachable"));
    }

    #[test]
    fn test_contextual_error_into_inner() {
        let error = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        let context = ErrorContext::new("Gateway", "connect");
        let contextual = ContextualError::new(error.clone(), context);
        let inner = contextual.into_inner();

        assert_eq!(inner, error);
    }

    #[test]
    fn test_zephyr_error_with_context() {
        let error = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        let contextual = error.with_context(ErrorContext::new("Gateway", "connect"));

        assert_eq!(contextual.error_type(), "NetworkError");
        assert_eq!(contextual.context().component(), "Gateway");
    }

    #[test]
    fn test_with_context_trait() {
        let error = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        let contextual = error.with_context(ErrorContext::new("Gateway", "connect"));

        assert_eq!(contextual.error_type(), "NetworkError");
    }

    #[test]
    fn test_contextual_error_std_error() {
        use std::error::Error;

        let error = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        let context = ErrorContext::new("Gateway", "connect");
        let contextual = ContextualError::new(error, context);

        // Test that it implements std::error::Error
        let _: &dyn std::error::Error = &contextual;
        assert!(contextual.source().is_some());
    }

    #[test]
    fn test_from_contextual_error_to_zephyr_error() {
        let error = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        let context = ErrorContext::new("Gateway", "connect");
        let contextual = ContextualError::new(error.clone(), context);
        let converted: ZephyrError = contextual.into();

        assert_eq!(converted, error);
    }
}
