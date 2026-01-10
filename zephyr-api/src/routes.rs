//! API route definitions.
//!
//! This module defines all API routes and their handlers.

use axum::{
    Router, middleware,
    routing::{delete, get, patch, post, put},
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::config::CorsConfig;
use crate::handlers::{account, health, orders, positions, strategies, tenants, trades};
use crate::middleware::auth::auth_middleware;
use crate::state::AppState;
use crate::ws::{WsState, ws_handler};

/// Creates the API router with all routes.
pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = build_cors_layer(&state.config.cors);

    // Public routes (no authentication required)
    let public_routes = Router::new()
        .route("/health", get(health::health_check))
        .route("/metrics", get(health::metrics));

    // Protected routes (authentication required)
    let protected_routes = Router::new()
        // Strategy routes
        .route("/strategies", get(strategies::list_strategies))
        .route("/strategies", post(strategies::create_strategy))
        .route("/strategies/{id}", get(strategies::get_strategy))
        .route("/strategies/{id}/start", post(strategies::start_strategy))
        .route("/strategies/{id}/stop", post(strategies::stop_strategy))
        // Order routes
        .route("/orders", get(orders::list_orders))
        .route("/orders", post(orders::create_order))
        .route("/orders/{id}", get(orders::get_order))
        .route("/orders/{id}", delete(orders::cancel_order))
        // Position routes
        .route("/positions", get(positions::list_positions))
        .route("/positions/stats", get(positions::position_stats))
        // Trade routes
        .route("/trades", get(trades::list_trades))
        // Account routes
        .route("/account", get(account::get_account))
        .route("/account/risk", get(account::get_risk_metrics))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Admin routes (authentication required, admin only)
    let admin_routes = Router::new()
        // Tenant management routes
        .route("/tenants", get(tenants::list_tenants))
        .route("/tenants", post(tenants::create_tenant))
        .route("/tenants/stats", get(tenants::tenant_stats))
        .route("/tenants/{id}", get(tenants::get_tenant))
        .route("/tenants/{id}", patch(tenants::update_tenant))
        .route("/tenants/{id}", delete(tenants::delete_tenant))
        .route("/tenants/{id}/activate", post(tenants::activate_tenant))
        .route("/tenants/{id}/suspend", post(tenants::suspend_tenant))
        .route("/tenants/{id}/quota", put(tenants::update_tenant_quota))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Create WebSocket state
    let ws_state = Arc::new(WsState::new(
        state.config.websocket.clone(),
        state.jwt_manager.clone(),
    ));

    // WebSocket route (authentication via query param or message)
    let ws_routes = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(ws_state);

    // Combine routes under /api/v1 prefix
    Router::new()
        .nest("/api/v1", public_routes.merge(protected_routes))
        .nest("/api/v1/admin", admin_routes)
        .merge(ws_routes)
        .layer(cors)
        .with_state(state)
}

/// Builds the CORS layer from configuration.
fn build_cors_layer(config: &CorsConfig) -> CorsLayer {
    if !config.enabled {
        return CorsLayer::new();
    }

    let mut cors = CorsLayer::new();

    // Set allowed origins
    if config.allowed_origins.is_empty() {
        cors = cors.allow_origin(Any);
    } else {
        // In production, parse specific origins
        cors = cors.allow_origin(Any);
    }

    // Set allowed methods
    cors = cors.allow_methods(Any);

    // Set allowed headers
    cors = cors.allow_headers(Any);

    // Set credentials
    if config.allow_credentials {
        cors = cors.allow_credentials(true);
    }

    // Set max age
    cors = cors.max_age(std::time::Duration::from_secs(config.max_age_secs));

    cors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ApiConfig;

    #[test]
    fn test_create_router() {
        let state = Arc::new(AppState::new(ApiConfig::default()));
        let _router = create_router(state);
        // Router creation should not panic
    }

    #[test]
    fn test_build_cors_layer_disabled() {
        let config = CorsConfig {
            enabled: false,
            ..Default::default()
        };
        let _cors = build_cors_layer(&config);
    }

    #[test]
    fn test_build_cors_layer_enabled() {
        let config = CorsConfig {
            enabled: true,
            allow_credentials: true,
            max_age_secs: 7200,
            ..Default::default()
        };
        let _cors = build_cors_layer(&config);
    }
}
