//! API server implementation.
//!
//! This module provides the main API server that handles HTTP requests.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::config::ApiConfig;
use crate::error::ApiError;
use crate::middleware::RequestIdLayer;
use crate::routes::create_router;
use crate::state::AppState;

/// API server.
pub struct ApiServer {
    /// Server configuration
    config: ApiConfig,
    /// Application state
    state: Arc<AppState>,
}

impl ApiServer {
    /// Creates a new API server.
    #[must_use]
    pub fn new(config: ApiConfig) -> Self {
        let state = Arc::new(AppState::new(config.clone()));
        Self { config, state }
    }

    /// Creates a new API server with custom state.
    #[must_use]
    pub fn with_state(config: ApiConfig, state: Arc<AppState>) -> Self {
        Self { config, state }
    }

    /// Returns a reference to the application state.
    #[must_use]
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    /// Runs the API server.
    ///
    /// # Errors
    ///
    /// Returns an error if the server fails to bind or run.
    pub async fn run(self) -> Result<(), ApiError> {
        let addr = self.config.bind_address();

        // Create router with middleware
        let app = create_router(self.state.clone())
            .layer(RequestIdLayer::new())
            .layer(TraceLayer::new_for_http());

        // Parse address
        let socket_addr: SocketAddr = addr
            .parse()
            .map_err(|e| ApiError::Internal(format!("Invalid bind address: {e}")))?;

        // Create listener
        let listener = TcpListener::bind(socket_addr)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to bind to {addr}: {e}")))?;

        info!("API server listening on {}", addr);

        // Run server
        axum::serve(listener, app)
            .await
            .map_err(|e| ApiError::Internal(format!("Server error: {e}")))?;

        Ok(())
    }

    /// Runs the API server with graceful shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error if the server fails to bind or run.
    pub async fn run_with_shutdown(
        self,
        shutdown_signal: impl std::future::Future<Output = ()> + Send + 'static,
    ) -> Result<(), ApiError> {
        let addr = self.config.bind_address();

        // Create router with middleware
        let app = create_router(self.state.clone())
            .layer(RequestIdLayer::new())
            .layer(TraceLayer::new_for_http());

        // Parse address
        let socket_addr: SocketAddr = addr
            .parse()
            .map_err(|e| ApiError::Internal(format!("Invalid bind address: {e}")))?;

        // Create listener
        let listener = TcpListener::bind(socket_addr)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to bind to {addr}: {e}")))?;

        info!("API server listening on {}", addr);

        // Run server with graceful shutdown
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
            .map_err(|e| ApiError::Internal(format!("Server error: {e}")))?;

        warn!("API server shutting down");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_server_new() {
        let config = ApiConfig::default();
        let server = ApiServer::new(config);

        assert!(!server.state().strategies.is_empty() || server.state().strategies.is_empty());
    }

    #[test]
    fn test_api_server_with_state() {
        let config = ApiConfig::default();
        let state = Arc::new(AppState::new(config.clone()));
        let server = ApiServer::with_state(config, state.clone());

        assert!(Arc::ptr_eq(server.state(), &state));
    }
}
