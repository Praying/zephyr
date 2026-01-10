//! Metrics collection and export for Zephyr.
//!
//! Provides Prometheus-compatible metrics for monitoring:
//! - Order metrics (submitted, filled, rejected)
//! - Latency metrics (order, tick processing, strategy)
//! - Position and PnL metrics
//! - WebSocket and API metrics

mod config;
mod recorder;

pub use config::MetricsConfig;
pub use recorder::ZephyrMetrics;

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::net::SocketAddr;
use std::sync::OnceLock;

static METRICS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Initialize the metrics system with the given configuration.
///
/// # Errors
///
/// Returns an error if metrics initialization fails.
///
/// # Example
///
/// ```no_run
/// use zephyr_telemetry::metrics::{init_metrics, MetricsConfig};
///
/// let config = MetricsConfig::default();
/// init_metrics(&config).expect("Failed to initialize metrics");
/// ```
pub fn init_metrics(config: &MetricsConfig) -> Result<(), MetricsError> {
    let builder = PrometheusBuilder::new();

    let handle = if config.expose_endpoint {
        let addr: SocketAddr = config
            .endpoint_address
            .parse()
            .map_err(|e| MetricsError::InvalidAddress(format!("{e}")))?;

        builder
            .with_http_listener(addr)
            .install_recorder()
            .map_err(|e| MetricsError::InitializationFailed(format!("{e}")))?
    } else {
        builder
            .install_recorder()
            .map_err(|e| MetricsError::InitializationFailed(format!("{e}")))?
    };

    METRICS_HANDLE
        .set(handle)
        .map_err(|_| MetricsError::AlreadyInitialized)?;

    // Register all metrics
    ZephyrMetrics::register();

    Ok(())
}

/// Get the Prometheus metrics output as a string.
///
/// # Panics
///
/// Panics if metrics have not been initialized.
#[must_use]
pub fn render_metrics() -> String {
    METRICS_HANDLE
        .get()
        .map(PrometheusHandle::render)
        .unwrap_or_default()
}

/// Errors that can occur during metrics initialization.
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    /// Metrics already initialized
    #[error("Metrics system already initialized")]
    AlreadyInitialized,

    /// Invalid endpoint address
    #[error("Invalid endpoint address: {0}")]
    InvalidAddress(String),

    /// Initialization failed
    #[error("Metrics initialization failed: {0}")]
    InitializationFailed(String),
}
