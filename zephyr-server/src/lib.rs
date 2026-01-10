//! # Zephyr Server
//!
//! Main server entry point for the Zephyr cryptocurrency trading system.
//!
//! This crate provides:
//! - Service startup and initialization
//! - Component lifecycle management
//! - Graceful shutdown handling
//! - Plugin loading for strategies and exchange adapters
//!
//! # Architecture
//!
//! The server orchestrates all Zephyr components:
//! - Configuration loading and validation
//! - Telemetry (logging and metrics) initialization
//! - API server startup
//! - Exchange adapter loading
//! - Strategy plugin loading
//! - Graceful shutdown with pending order cancellation

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::module_name_repetitions)]

pub mod config;
pub mod plugin;
pub mod server;
pub mod shutdown;

pub use config::ServerConfig;
pub use plugin::{PluginLoader, PluginRegistry};
pub use server::ZephyrServer;
pub use shutdown::ShutdownController;
