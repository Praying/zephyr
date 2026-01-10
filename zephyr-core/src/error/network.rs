//! Network-related error types.
//!
//! This module provides error types for network operations including
//! connection failures, timeouts, DNS resolution, TLS, and WebSocket errors.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Network error type covering connection failures, timeouts, DNS resolution errors, and TLS errors.
///
/// # Examples
///
/// ```
/// use zephyr_core::error::NetworkError;
///
/// let error = NetworkError::ConnectionFailed {
///     reason: "Connection refused".to_string(),
/// };
/// assert!(error.to_string().contains("Connection refused"));
/// ```
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkError {
    /// Connection to remote host failed.
    #[error("[Network] Connection failed: {reason}")]
    ConnectionFailed {
        /// Reason for the connection failure.
        reason: String,
    },

    /// Connection timed out.
    #[error("[Network] Connection timeout after {timeout_ms}ms")]
    Timeout {
        /// Timeout duration in milliseconds.
        timeout_ms: u64,
    },

    /// DNS resolution failed.
    #[error("[Network] DNS resolution failed for {host}")]
    DnsResolution {
        /// Host that failed to resolve.
        host: String,
    },

    /// TLS/SSL error occurred.
    #[error("[Network] TLS error: {reason}")]
    Tls {
        /// Reason for the TLS error.
        reason: String,
    },

    /// WebSocket error occurred.
    #[error("[Network] WebSocket error: {reason}")]
    WebSocket {
        /// Reason for the WebSocket error.
        reason: String,
    },

    /// HTTP request failed.
    #[error("[Network] HTTP error: status {status_code} - {reason}")]
    Http {
        /// HTTP status code.
        status_code: u16,
        /// Reason for the HTTP error.
        reason: String,
    },

    /// Connection was closed unexpectedly.
    #[error("[Network] Connection closed: {reason}")]
    ConnectionClosed {
        /// Reason for the connection closure.
        reason: String,
    },
}

impl NetworkError {
    /// Returns true if this error is recoverable (can be retried).
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Timeout { .. }
                | Self::ConnectionFailed { .. }
                | Self::ConnectionClosed { .. }
                | Self::WebSocket { .. }
        )
    }

    /// Returns the severity level of this error.
    #[must_use]
    pub fn severity(&self) -> super::ErrorSeverity {
        use super::ErrorSeverity;
        match self {
            Self::DnsResolution { .. } | Self::Tls { .. } => ErrorSeverity::Fatal,
            Self::Timeout { .. }
            | Self::ConnectionFailed { .. }
            | Self::ConnectionClosed { .. }
            | Self::WebSocket { .. } => ErrorSeverity::Recoverable,
            Self::Http { status_code, .. } if *status_code >= 500 => ErrorSeverity::Recoverable,
            Self::Http { .. } => ErrorSeverity::Warning,
        }
    }

    /// Returns a suggested retry delay in milliseconds, if applicable.
    #[must_use]
    pub fn suggested_retry_delay_ms(&self) -> Option<u64> {
        match self {
            Self::Timeout { timeout_ms } => Some(*timeout_ms / 2),
            Self::ConnectionFailed { .. } | Self::ConnectionClosed { .. } => Some(1000),
            Self::WebSocket { .. } => Some(500),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_failed() {
        let error = NetworkError::ConnectionFailed {
            reason: "Connection refused".to_string(),
        };
        assert!(error.to_string().contains("Connection refused"));
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_timeout() {
        let error = NetworkError::Timeout { timeout_ms: 5000 };
        assert!(error.to_string().contains("5000ms"));
        assert!(error.is_recoverable());
        assert_eq!(error.suggested_retry_delay_ms(), Some(2500));
    }

    #[test]
    fn test_dns_resolution() {
        let error = NetworkError::DnsResolution {
            host: "api.example.com".to_string(),
        };
        assert!(error.to_string().contains("api.example.com"));
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_tls_error() {
        let error = NetworkError::Tls {
            reason: "Certificate expired".to_string(),
        };
        assert!(error.to_string().contains("Certificate expired"));
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_serde_roundtrip() {
        let error = NetworkError::Timeout { timeout_ms: 3000 };
        let json = serde_json::to_string(&error).unwrap();
        let parsed: NetworkError = serde_json::from_str(&json).unwrap();
        assert_eq!(error, parsed);
    }
}
