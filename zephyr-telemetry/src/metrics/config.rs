//! Metrics configuration types.

use serde::{Deserialize, Serialize};

/// Configuration for the metrics system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Whether to expose a Prometheus HTTP endpoint
    #[serde(default = "default_expose_endpoint")]
    pub expose_endpoint: bool,

    /// Address for the Prometheus endpoint (e.g., "0.0.0.0:9090")
    #[serde(default = "default_endpoint_address")]
    pub endpoint_address: String,

    /// Prefix for all metric names
    #[serde(default = "default_prefix")]
    pub prefix: String,

    /// Default histogram buckets for latency metrics (in seconds)
    #[serde(default = "default_latency_buckets")]
    pub latency_buckets: Vec<f64>,

    /// Enable memory usage metrics
    #[serde(default = "default_true")]
    pub enable_memory_metrics: bool,

    /// Enable process metrics
    #[serde(default = "default_true")]
    pub enable_process_metrics: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            expose_endpoint: default_expose_endpoint(),
            endpoint_address: default_endpoint_address(),
            prefix: default_prefix(),
            latency_buckets: default_latency_buckets(),
            enable_memory_metrics: true,
            enable_process_metrics: true,
        }
    }
}

fn default_expose_endpoint() -> bool {
    true
}

fn default_endpoint_address() -> String {
    "0.0.0.0:9090".to_string()
}

fn default_prefix() -> String {
    "zephyr".to_string()
}

fn default_latency_buckets() -> Vec<f64> {
    vec![
        0.0001, // 100µs
        0.0005, // 500µs
        0.001,  // 1ms
        0.005,  // 5ms
        0.01,   // 10ms
        0.05,   // 50ms
        0.1,    // 100ms
        0.5,    // 500ms
        1.0,    // 1s
        5.0,    // 5s
    ]
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MetricsConfig::default();
        assert!(config.expose_endpoint);
        assert_eq!(config.endpoint_address, "0.0.0.0:9090");
        assert_eq!(config.prefix, "zephyr");
        assert!(!config.latency_buckets.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = MetricsConfig {
            expose_endpoint: false,
            endpoint_address: "127.0.0.1:8080".to_string(),
            prefix: "trading".to_string(),
            latency_buckets: vec![0.001, 0.01, 0.1],
            enable_memory_metrics: false,
            enable_process_metrics: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: MetricsConfig = serde_json::from_str(&json).unwrap();

        assert!(!parsed.expose_endpoint);
        assert_eq!(parsed.endpoint_address, "127.0.0.1:8080");
        assert_eq!(parsed.prefix, "trading");
    }
}
