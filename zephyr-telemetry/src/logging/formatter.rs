//! Custom log formatters.

use serde::Serialize;
use std::fmt;

/// Structured log entry for JSON output.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct LogEntry {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Log level
    pub level: String,
    /// Target module
    pub target: String,
    /// Span ID for tracing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    /// Correlation ID for request tracing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    /// Log message
    pub message: String,
    /// Additional structured fields
    #[serde(flatten)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            self.timestamp, self.level, self.target, self.message
        )
    }
}

/// Format a log level for display.
#[allow(dead_code)]
pub fn format_level(level: tracing::Level) -> &'static str {
    match level {
        tracing::Level::TRACE => "TRACE",
        tracing::Level::DEBUG => "DEBUG",
        tracing::Level::INFO => "INFO",
        tracing::Level::WARN => "WARN",
        tracing::Level::ERROR => "ERROR",
    }
}

/// Format a timestamp in ISO 8601 format.
#[allow(dead_code)]
pub fn format_timestamp() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_serialization() {
        let entry = LogEntry {
            timestamp: "2024-01-15T10:30:00.000Z".to_string(),
            level: "INFO".to_string(),
            target: "zephyr::engine".to_string(),
            span_id: Some("abc123".to_string()),
            correlation_id: Some("req-456".to_string()),
            message: "Order submitted".to_string(),
            fields: serde_json::Map::new(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("Order submitted"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn test_format_timestamp() {
        let ts = format_timestamp();
        assert!(ts.contains("T"));
        assert!(ts.ends_with("Z"));
    }
}
