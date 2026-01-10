//! Logging configuration types.

use serde::{Deserialize, Serialize};

/// Configuration for the logging system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    /// Default log level (e.g., "info", "debug", "trace")
    #[serde(default = "default_level")]
    pub level: String,

    /// Output format
    #[serde(default)]
    pub format: LogFormat,

    /// Output targets
    #[serde(default = "default_outputs")]
    pub outputs: Vec<LogOutput>,

    /// Include thread IDs in log output
    #[serde(default)]
    pub include_thread_id: bool,

    /// Include file and line information
    #[serde(default)]
    pub include_file_info: bool,

    /// Include span enter/exit events
    #[serde(default)]
    pub include_span_events: bool,

    /// Rate limiting for repetitive messages (messages per second)
    #[serde(default)]
    pub rate_limit: Option<u32>,

    /// Sampling rate for high-frequency events (1 = log all, 10 = log 1 in 10)
    #[serde(default = "default_sampling_rate")]
    pub sampling_rate: u32,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_level(),
            format: LogFormat::default(),
            outputs: default_outputs(),
            include_thread_id: false,
            include_file_info: false,
            include_span_events: false,
            rate_limit: None,
            sampling_rate: default_sampling_rate(),
        }
    }
}

fn default_level() -> String {
    "info".to_string()
}

fn default_outputs() -> Vec<LogOutput> {
    vec![LogOutput::Stdout]
}

fn default_sampling_rate() -> u32 {
    1
}

/// Log output format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// JSON format for log aggregation systems
    #[default]
    Json,
    /// Human-readable format for development
    Pretty,
}

/// Log output target.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LogOutput {
    /// Output to stdout
    Stdout,
    /// Output to file with optional rotation
    File {
        /// Directory path for log files
        path: String,
        /// Rotation configuration
        rotation: Option<RotationConfig>,
    },
}

/// Log rotation configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RotationConfig {
    /// Rotate logs hourly
    Hourly,
    /// Rotate logs daily
    Daily,
    /// Never rotate (single file)
    Never,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LogConfig::default();
        assert_eq!(config.level, "info");
        assert_eq!(config.format, LogFormat::Json);
        assert_eq!(config.outputs.len(), 1);
    }

    #[test]
    fn test_config_serialization() {
        let config = LogConfig {
            level: "debug".to_string(),
            format: LogFormat::Pretty,
            outputs: vec![
                LogOutput::Stdout,
                LogOutput::File {
                    path: "/var/log/zephyr".to_string(),
                    rotation: Some(RotationConfig::Daily),
                },
            ],
            include_thread_id: true,
            include_file_info: true,
            include_span_events: true,
            rate_limit: Some(100),
            sampling_rate: 10,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: LogConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.level, "debug");
        assert_eq!(parsed.format, LogFormat::Pretty);
        assert_eq!(parsed.outputs.len(), 2);
    }
}
