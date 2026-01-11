//! API response types.
//!
//! This module provides standardized response types for the API.

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

/// Standard API response wrapper.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T>
where
    T: Serialize,
{
    /// Response status
    pub status: &'static str,
    /// Response data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Request ID for tracing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Creates a successful response with data.
    #[must_use]
    pub fn success(data: T) -> Self {
        Self {
            status: "success",
            data: Some(data),
            message: None,
            request_id: None,
        }
    }

    /// Creates a successful response with data and message.
    #[must_use]
    pub fn success_with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            status: "success",
            data: Some(data),
            message: Some(message.into()),
            request_id: None,
        }
    }

    /// Sets the request ID.
    #[must_use]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

/// Empty success response (for operations that don't return data).
#[derive(Debug, Serialize)]
pub struct EmptyResponse {
    /// Response status
    pub status: &'static str,
    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl EmptyResponse {
    /// Creates an empty success response.
    #[must_use]
    pub fn success() -> Self {
        Self {
            status: "success",
            message: None,
        }
    }

    /// Creates an empty success response with a message.
    #[must_use]
    pub fn success_with_message(message: impl Into<String>) -> Self {
        Self {
            status: "success",
            message: Some(message.into()),
        }
    }
}

impl IntoResponse for EmptyResponse {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

/// Paginated response wrapper.
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T>
where
    T: Serialize,
{
    /// Response status
    pub status: &'static str,
    /// Response data
    pub data: Vec<T>,
    /// Pagination info
    pub pagination: PaginationInfo,
}

/// Pagination information.
#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    /// Current page number (1-indexed)
    pub page: u32,
    /// Items per page
    pub per_page: u32,
    /// Total number of items
    pub total: u64,
    /// Total number of pages
    pub total_pages: u32,
    /// Has next page
    pub has_next: bool,
    /// Has previous page
    pub has_prev: bool,
}

impl PaginationInfo {
    /// Creates pagination info from parameters.
    #[must_use]
    pub fn new(page: u32, per_page: u32, total: u64) -> Self {
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let total_pages =
            u32::try_from(((total as f64) / f64::from(per_page)).ceil() as u64).unwrap_or(u32::MAX);
        Self {
            page,
            per_page,
            total,
            total_pages,
            has_next: page < total_pages,
            has_prev: page > 1,
        }
    }
}

impl<T: Serialize> PaginatedResponse<T> {
    /// Creates a paginated response.
    #[must_use]
    pub fn new(data: Vec<T>, page: u32, per_page: u32, total: u64) -> Self {
        Self {
            status: "success",
            data,
            pagination: PaginationInfo::new(page, per_page, total),
        }
    }
}

impl<T: Serialize> IntoResponse for PaginatedResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

/// Created response (HTTP 201).
#[derive(Debug, Serialize)]
pub struct CreatedResponse<T>
where
    T: Serialize,
{
    /// Response status
    pub status: &'static str,
    /// Created resource
    pub data: T,
    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T: Serialize> CreatedResponse<T> {
    /// Creates a new created response.
    #[must_use]
    pub fn new(data: T) -> Self {
        Self {
            status: "success",
            data,
            message: None,
        }
    }

    /// Creates a new created response with a message.
    #[must_use]
    pub fn with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            status: "success",
            data,
            message: Some(message.into()),
        }
    }
}

impl<T: Serialize> IntoResponse for CreatedResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::CREATED, Json(self)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert_eq!(response.status, "success");
        assert_eq!(response.data, Some("test data"));
        assert!(response.message.is_none());
    }

    #[test]
    fn test_api_response_with_message() {
        let response = ApiResponse::success_with_message("data", "Operation completed");
        assert_eq!(response.message, Some("Operation completed".to_string()));
    }

    #[test]
    fn test_empty_response() {
        let response = EmptyResponse::success();
        assert_eq!(response.status, "success");
        assert!(response.message.is_none());
    }

    #[test]
    fn test_pagination_info() {
        let info = PaginationInfo::new(2, 10, 45);
        assert_eq!(info.page, 2);
        assert_eq!(info.per_page, 10);
        assert_eq!(info.total, 45);
        assert_eq!(info.total_pages, 5);
        assert!(info.has_next);
        assert!(info.has_prev);
    }

    #[test]
    fn test_pagination_first_page() {
        let info = PaginationInfo::new(1, 10, 45);
        assert!(info.has_next);
        assert!(!info.has_prev);
    }

    #[test]
    fn test_pagination_last_page() {
        let info = PaginationInfo::new(5, 10, 45);
        assert!(!info.has_next);
        assert!(info.has_prev);
    }

    #[test]
    fn test_paginated_response() {
        let response = PaginatedResponse::new(vec![1, 2, 3], 1, 10, 100);
        assert_eq!(response.status, "success");
        assert_eq!(response.data.len(), 3);
        assert_eq!(response.pagination.total_pages, 10);
    }
}
