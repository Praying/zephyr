//! WebSocket server state.
//!
//! This module provides the shared state for the WebSocket server.

use std::sync::Arc;

use super::config::WsConfig;
use super::connection::ConnectionRegistry;
use crate::auth::JwtManager;

/// Shared WebSocket server state.
#[derive(Debug)]
pub struct WsState {
    /// WebSocket configuration
    pub config: WsConfig,
    /// JWT manager for authentication
    pub jwt_manager: Arc<JwtManager>,
    /// Connection registry
    pub registry: ConnectionRegistry,
}

impl WsState {
    /// Creates a new WebSocket state.
    #[must_use]
    pub fn new(config: WsConfig, jwt_manager: Arc<JwtManager>) -> Self {
        Self {
            config,
            jwt_manager,
            registry: ConnectionRegistry::new(),
        }
    }

    /// Returns the number of active connections.
    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.registry.connection_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::JwtConfig;

    fn test_jwt_manager() -> Arc<JwtManager> {
        let config = JwtConfig {
            secret: "test-secret".to_string(),
            expiration_secs: 3600,
            issuer: "test".to_string(),
            audience: "test".to_string(),
        };
        Arc::new(JwtManager::new(&config))
    }

    #[test]
    fn test_ws_state_new() {
        let config = WsConfig::default();
        let jwt_manager = test_jwt_manager();
        let state = WsState::new(config, jwt_manager);

        assert_eq!(state.connection_count(), 0);
    }
}
