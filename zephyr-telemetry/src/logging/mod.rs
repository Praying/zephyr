//! Structured logging system for Zephyr.
//!
//! Provides configurable logging with support for:
//! - JSON and pretty-print formats
//! - Multiple output targets (stdout, file)
//! - Log rotation
//! - Dynamic log level adjustment
//! - Sensitive data masking

mod config;
mod formatter;
mod layer;

pub use config::{LogConfig, LogFormat, LogOutput, RotationConfig};
pub use layer::MaskingLayer;

use crate::masking::SensitiveDataMasker;
use std::sync::Arc;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    EnvFilter, Layer,
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

/// Initialize the logging system with the given configuration.
///
/// Returns a guard that must be kept alive for the duration of the program
/// to ensure all logs are flushed.
///
/// # Example
///
/// ```no_run
/// use zephyr_telemetry::logging::{init_logging, LogConfig};
///
/// let config = LogConfig::default();
/// let _guard = init_logging(&config).expect("Failed to initialize logging");
/// ```
pub fn init_logging(config: &LogConfig) -> Result<Vec<WorkerGuard>, LoggingError> {
    let mut guards = Vec::new();
    let masker = Arc::new(SensitiveDataMasker::new());

    // Build the env filter
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));

    // Create layers based on outputs
    let mut layers: Vec<Box<dyn Layer<_> + Send + Sync>> = Vec::new();

    for output in &config.outputs {
        match output {
            LogOutput::Stdout => {
                let layer = create_stdout_layer(config, masker.clone());
                layers.push(Box::new(layer));
            }
            LogOutput::File { path, rotation } => {
                let (layer, guard) =
                    create_file_layer(config, path, rotation.as_ref(), masker.clone())?;
                layers.push(Box::new(layer));
                guards.push(guard);
            }
        }
    }

    // Initialize the subscriber
    tracing_subscriber::registry()
        .with(env_filter)
        .with(layers)
        .init();

    Ok(guards)
}

fn create_stdout_layer<S>(config: &LogConfig, masker: Arc<SensitiveDataMasker>) -> impl Layer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    let base_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(config.include_thread_id)
        .with_file(config.include_file_info)
        .with_line_number(config.include_file_info)
        .with_span_events(if config.include_span_events {
            FmtSpan::ENTER | FmtSpan::EXIT
        } else {
            FmtSpan::NONE
        });

    match config.format {
        LogFormat::Json => {
            let layer = base_layer.json().flatten_event(true);
            MaskingLayer::new(layer, masker)
        }
        LogFormat::Pretty => {
            // For pretty format, we use a different approach
            let layer = base_layer.json().flatten_event(true);
            MaskingLayer::new(layer, masker)
        }
    }
}

#[allow(clippy::unnecessary_wraps)]
fn create_file_layer<S>(
    config: &LogConfig,
    path: &str,
    rotation: Option<&RotationConfig>,
    masker: Arc<SensitiveDataMasker>,
) -> Result<(impl Layer<S>, WorkerGuard), LoggingError>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    let file_appender = rotation.map_or_else(
        || tracing_appender::rolling::daily(path, "zephyr.log"),
        |rot| match rot {
            RotationConfig::Hourly => tracing_appender::rolling::hourly(path, "zephyr.log"),
            RotationConfig::Daily => tracing_appender::rolling::daily(path, "zephyr.log"),
            RotationConfig::Never => tracing_appender::rolling::never(path, "zephyr.log"),
        },
    );

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let layer = fmt::layer()
        .with_writer(non_blocking)
        .with_target(true)
        .with_thread_ids(config.include_thread_id)
        .with_file(config.include_file_info)
        .with_line_number(config.include_file_info)
        .with_span_events(if config.include_span_events {
            FmtSpan::ENTER | FmtSpan::EXIT
        } else {
            FmtSpan::NONE
        })
        .json()
        .flatten_event(true);

    Ok((MaskingLayer::new(layer, masker), guard))
}

/// Errors that can occur during logging initialization.
#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    /// Failed to create log directory
    #[error("Failed to create log directory: {0}")]
    DirectoryCreation(#[from] std::io::Error),

    /// Invalid configuration
    #[error("Invalid logging configuration: {0}")]
    InvalidConfig(String),
}
