//! # Zephyr API
//!
//! REST and WebSocket API for the Zephyr trading system.
//!
//! This crate provides:
//! - REST API endpoints for strategy and order management
//! - WebSocket server for real-time data streaming
//! - JWT authentication middleware
//! - Rate limiting middleware
//! - CORS configuration
//!
//! # Architecture
//!
//! The API layer is built on Axum and provides:
//! - `/api/v1/strategies` - Strategy management
//! - `/api/v1/orders` - Order management
//! - `/api/v1/positions` - Position queries
//! - `/api/v1/account` - Account information
//! - `/api/v1/health` - Health check
//! - `/api/v1/metrics` - Prometheus metrics
//! - `/ws` - WebSocket endpoint for real-time updates
//!
//! # Authentication
//!
//! All endpoints (except health and metrics) require JWT authentication.
//! Tokens are passed via the `Authorization: Bearer <token>` header.
//!
//! For WebSocket connections, tokens can be passed via:
//! - Query parameter: `ws://host/ws?token=<jwt>`
//! - Auth message after connection: `{"type":"auth","token":"<jwt>"}`

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

pub mod auth;
pub mod config;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod response;
pub mod routes;
pub mod server;
pub mod state;
pub mod ws;

pub use config::ApiConfig;
pub use error::ApiError;
pub use server::ApiServer;
pub use state::AppState;
pub use ws::{WsBroadcaster, WsConfig, WsState};
