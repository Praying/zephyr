//! Health check and system status handlers.

use axum::{Json, extract::State};
use serde::Serialize;
use std::sync::Arc;

use crate::response::ApiResponse;
use crate::state::AppState;

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Service status
    pub status: &'static str,
    /// Service version
    pub version: &'static str,
    /// Uptime in seconds
    pub uptime_secs: u64,
    /// Component statuses
    pub components: ComponentStatus,
}

/// Component status.
#[derive(Debug, Serialize)]
pub struct ComponentStatus {
    /// API server status
    pub api: &'static str,
    /// Database status
    pub database: &'static str,
    /// Cache status
    pub cache: &'static str,
    /// Exchange connections status
    pub exchanges: &'static str,
}

/// Health check handler.
///
/// GET /health
pub async fn health_check(State(_state): State<Arc<AppState>>) -> Json<HealthResponse> {
    // In a real implementation, this would check actual component health
    let response = HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: 0, // Would be calculated from start time
        components: ComponentStatus {
            api: "healthy",
            database: "healthy",
            cache: "healthy",
            exchanges: "healthy",
        },
    };

    Json(response)
}

/// Metrics response (Prometheus format placeholder).
#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    /// Total requests
    pub total_requests: u64,
    /// Active connections
    pub active_connections: u32,
    /// Strategy count
    pub strategy_count: u32,
    /// Order count
    pub order_count: u32,
}

/// Metrics handler.
///
/// GET /metrics
pub async fn metrics(State(state): State<Arc<AppState>>) -> ApiResponse<MetricsResponse> {
    let response = MetricsResponse {
        total_requests: 0, // Would be tracked by metrics system
        active_connections: 0,
        strategy_count: u32::try_from(state.strategies.len()).unwrap_or(u32::MAX),
        order_count: u32::try_from(state.orders.len()).unwrap_or(u32::MAX),
    };

    ApiResponse::success(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ApiConfig;

    #[tokio::test]
    async fn test_health_check() {
        let state = Arc::new(AppState::new(ApiConfig::default()));
        let response = health_check(State(state)).await;

        assert_eq!(response.status, "healthy");
        assert_eq!(response.components.api, "healthy");
    }
}
