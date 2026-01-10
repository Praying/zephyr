//! API middleware components.
//!
//! This module provides middleware for:
//! - JWT authentication
//! - Rate limiting
//! - Request logging
//! - Request ID generation

pub mod auth;
mod rate_limit;
mod request_id;

pub use auth::{Auth, AuthLayer, AuthenticatedUser};
pub use rate_limit::{RateLimitLayer, RateLimiter};
pub use request_id::{RequestId, RequestIdLayer};
