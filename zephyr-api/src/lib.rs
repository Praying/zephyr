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
// Allow various code style issues that match project conventions
#![allow(clippy::too_many_lines)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::struct_field_names)]
#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::branches_sharing_code)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::doc_markdown)]
// Allow async issues
#![allow(clippy::unused_async)]
#![allow(clippy::await_holding_lock)]
#![allow(clippy::future_not_send)]
// Allow lifetime and reference issues
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::needless_pass_by_ref_mut)]
#![allow(clippy::trivially_copy_pass_by_ref)]
// Allow more issues
#![allow(clippy::wildcard_imports)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::map_identity)]
#![allow(clippy::use_self)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::must_use_unit)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::unused_self)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::let_underscore_must_use)]
#![allow(clippy::missing_trait_methods)]
#![allow(clippy::must_use_candidate)]
// Allow test-only issues
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(test, allow(clippy::indexing_slicing))]

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
