//! # Zephyr Telemetry
//!
//! Logging, tracing, and metrics for the Zephyr cryptocurrency trading system.
//!
//! This crate provides:
//! - Structured logging with JSON and pretty formats
//! - Log rotation and file management
//! - Sensitive data masking
//! - Distributed tracing with spans
//! - Prometheus metrics export
//!
//! ## Features
//!
//! - **Structured Logging**: Uses `tracing` for structured, contextual logging
//! - **Multiple Outputs**: Supports stdout, file, and network targets
//! - **Log Rotation**: Automatic rotation based on size and time
//! - **Data Masking**: Automatic masking of API keys and secrets
//! - **Metrics**: Prometheus-compatible metrics export
//! - **Spans**: Request tracing across components

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::module_name_repetitions)]

/// Logging configuration and initialization
pub mod logging;

/// Sensitive data masking
pub mod masking;

/// Span definitions for distributed tracing
pub mod spans;

/// Metrics collection and export
pub mod metrics;

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::logging::{LogConfig, LogFormat, LogOutput, init_logging};
    pub use crate::masking::SensitiveDataMasker;
    pub use crate::metrics::{MetricsConfig, ZephyrMetrics, init_metrics};
    pub use crate::spans::*;
}
