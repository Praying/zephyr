//! API error types.
//!
//! This module provides error types for the API layer including:
//! - Authentication errors
//! - Validation errors
//! - Rate limiting errors
//! - Internal server errors

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

/// API error type.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Authentication failed
    #[error("Authentication failed: {0}")]
    Unauthorized(String),

    /// Access forbidden
    #[error("Access forbidden: {0}")]
    Forbidden(String),

    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Bad request / validation error
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// Conflict (e.g., duplicate resource)
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Internal server error
    #[error("Internal server error: {0}")]
    Internal(String),

    /// Service unavailable
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Request timeout
    #[error("Request timeout")]
    Timeout,
}

impl ApiError {
    /// Returns the HTTP status code for this error.
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Timeout => StatusCode::REQUEST_TIMEOUT,
        }
    }

    /// Returns the error code string.
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::Forbidden(_) => "FORBIDDEN",
            Self::NotFound(_) => "NOT_FOUND",
            Self::BadRequest(_) => "BAD_REQUEST",
            Self::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
            Self::Conflict(_) => "CONFLICT",
            Self::Internal(_) => "INTERNAL_ERROR",
            Self::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
            Self::Timeout => "TIMEOUT",
        }
    }
}

/// Error response body.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error status
    pub status: &'static str,
    /// Error code
    pub code: &'static str,
    /// Error message
    pub message: String,
    /// Request ID (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = ErrorResponse {
            status: "error",
            code: self.error_code(),
            message: self.to_string(),
            request_id: None,
        };

        (status, Json(body)).into_response()
    }
}

/// Result type alias for API operations.
pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            ApiError::Unauthorized("test".to_string()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            ApiError::Forbidden("test".to_string()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            ApiError::NotFound("test".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::BadRequest("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::RateLimitExceeded.status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            ApiError::Unauthorized("test".to_string()).error_code(),
            "UNAUTHORIZED"
        );
        assert_eq!(
            ApiError::Internal("test".to_string()).error_code(),
            "INTERNAL_ERROR"
        );
    }
}
