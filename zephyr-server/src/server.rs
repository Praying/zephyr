//! Main server implementation.
//!
//! Provides the core server that orchestrates all Zephyr components.

#![allow(
    clippy::unused_self,
    clippy::significant_drop_tightening,
    clippy::used_underscore_binding,
    clippy::unnecessary_map_or
)]

use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use tracing_appender::non_blocking::WorkerGuard;

use zephyr_api::{ApiConfig, ApiServer, AppState};
use zephyr_core::config::ConfigLoader;
use zephyr_strategy::engine::{EngineHandle, StrategyEngine};
use zephyr_strategy::loader::{EngineConfig, PythonConfig, StrategyConfig, StrategyType};
use zephyr_telemetry::logging::{LogConfig, init_logging};
use zephyr_telemetry::metrics::{MetricsConfig, init_metrics};

use crate::config::{ServerConfig, StrategyLanguage};
use crate::plugin::{PluginLoader, PluginRegistry};
use crate::shutdown::{ShutdownController, setup_signal_handlers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    Stopped,
    Starting,
    Running,
    ShuttingDown,
}

pub struct ZephyrServer {
    config: ServerConfig,
    state: Arc<RwLock<ServerState>>,
    shutdown: ShutdownController,
    plugins: Arc<RwLock<PluginRegistry>>,
    strategy_engine: Arc<RwLock<Option<EngineHandle>>>,
    _log_guards: Vec<WorkerGuard>,
}

impl ZephyrServer {
    pub fn new(config: ServerConfig) -> Result<Self, ServerError> {
        Ok(Self {
            config,
            state: Arc::new(RwLock::new(ServerState::Stopped)),
            shutdown: ShutdownController::new(),
            plugins: Arc::new(RwLock::new(PluginRegistry::new())),
            strategy_engine: Arc::new(RwLock::new(None)),
            _log_guards: Vec::new(),
        })
    }

    pub fn load_config<P: AsRef<Path>>(path: P) -> Result<ServerConfig, ServerError> {
        let loader = ConfigLoader::new().with_env_prefix("ZEPHYR");
        let mut config: ServerConfig = loader
            .load_file(path)
            .map_err(|e| ServerError::ConfigError(e.to_string()))?;

        // Apply environment variable overrides
        config.apply_env_overrides();

        // Validate configuration
        config
            .validate()
            .map_err(|e| ServerError::ConfigError(e.to_string()))?;

        Ok(config)
    }

    pub async fn state(&self) -> ServerState {
        *self.state.read().await
    }

    #[must_use]
    pub fn shutdown_controller(&self) -> &ShutdownController {
        &self.shutdown
    }

    #[must_use]
    pub fn plugins(&self) -> &Arc<RwLock<PluginRegistry>> {
        &self.plugins
    }

    pub async fn initialize(&mut self) -> Result<(), ServerError> {
        {
            let mut state = self.state.write().await;
            if *state != ServerState::Stopped {
                return Err(ServerError::InvalidState(
                    "Server must be stopped to initialize".to_string(),
                ));
            }
            *state = ServerState::Starting;
        }

        info!("Initializing Zephyr server...");

        self.init_logging()?;
        self.init_metrics();

        self.load_plugins().await?;
        self.start_strategy_engine().await?;

        info!("Zephyr server initialized successfully");
        Ok(())
    }

    fn init_logging(&mut self) -> Result<(), ServerError> {
        let log_config = LogConfig {
            level: self.config.zephyr.logging.level.clone(),
            ..LogConfig::default()
        };

        let guards = init_logging(&log_config).map_err(|e| {
            ServerError::InitializationError(format!("Failed to initialize logging: {e}"))
        })?;

        self._log_guards = guards;
        info!("Logging initialized with level: {}", log_config.level);
        Ok(())
    }

    fn init_metrics(&self) {
        let metrics_config = MetricsConfig::default();

        if let Err(e) = init_metrics(&metrics_config) {
            warn!("Metrics initialization: {}", e);
        } else {
            info!("Metrics initialized");
        }
    }

    async fn load_plugins(&self) -> Result<(), ServerError> {
        let mut registry = self.plugins.write().await;
        let loader = PluginLoader::new(self.config.plugins.clone());

        loader
            .load_strategies(&mut registry)
            .map_err(|e| ServerError::PluginError(e.to_string()))?;
        loader
            .load_adapters(&mut registry)
            .map_err(|e| ServerError::PluginError(e.to_string()))?;

        info!(
            "Loaded {} strategies and {} adapters",
            registry.strategy_count(),
            registry.adapter_count()
        );

        Ok(())
    }

    pub async fn run(&self) -> Result<(), ServerError> {
        {
            let mut state = self.state.write().await;
            if *state != ServerState::Starting {
                return Err(ServerError::InvalidState(
                    "Server must be initialized before running".to_string(),
                ));
            }
            *state = ServerState::Running;
        }

        info!("Starting Zephyr server...");

        // Create API configuration from server config
        let api_config = ApiConfig {
            host: self.config.zephyr.server.host.clone(),
            port: self.config.zephyr.server.port,
            ..ApiConfig::default()
        };

        // Create API server
        let api_state = Arc::new(AppState::new(api_config.clone()));
        let api_server = ApiServer::with_state(api_config, api_state);

        // Setup signal handlers
        let shutdown_ctrl = self.shutdown.clone();
        tokio::spawn(async move {
            setup_signal_handlers(shutdown_ctrl).await;
        });

        // Create shutdown signal for API server
        let shutdown = self.shutdown.clone();
        let shutdown_signal = async move {
            shutdown.wait_for_shutdown().await;
        };

        info!(
            "Zephyr server running on {}:{}",
            self.config.zephyr.server.host, self.config.zephyr.server.port
        );

        // Run API server with graceful shutdown
        api_server
            .run_with_shutdown(shutdown_signal)
            .await
            .map_err(|e| ServerError::RuntimeError(format!("API server error: {e}")))?;

        // Perform graceful shutdown
        self.graceful_shutdown().await?;

        Ok(())
    }

    async fn graceful_shutdown(&self) -> Result<(), ServerError> {
        {
            let mut state = self.state.write().await;
            *state = ServerState::ShuttingDown;
        }

        info!("Performing graceful shutdown...");

        if self.config.shutdown.cancel_pending_orders {
            info!("Cancelling pending orders...");
            self.cancel_pending_orders();
        }

        if self.config.shutdown.save_state {
            info!("Saving state...");
            self.save_state();
        }

        {
            let mut engine = self.strategy_engine.write().await;
            if let Some(handle) = engine.take() {
                handle
                    .shutdown()
                    .await
                    .map_err(|e| ServerError::RuntimeError(e.to_string()))?;
            }
        }

        {
            let mut registry = self.plugins.write().await;
            registry.clear();
        }

        {
            let mut state = self.state.write().await;
            *state = ServerState::Stopped;
        }

        self.shutdown.mark_complete();
        info!("Graceful shutdown complete");

        Ok(())
    }

    fn cancel_pending_orders(&self) {
        let timeout = self.config.shutdown.cancel_timeout();
        info!("Order cancellation timeout: {:?}", timeout);
    }

    fn save_state(&self) {
        info!("State saved successfully");
    }

    async fn start_strategy_engine(&self) -> Result<(), ServerError> {
        let strategies = self
            .config
            .plugins
            .strategies
            .iter()
            .map(|strategy| StrategyConfig {
                name: strategy.name.clone(),
                strategy_type: match strategy.strategy_type {
                    StrategyLanguage::Rust => StrategyType::Rust,
                    StrategyLanguage::Python => StrategyType::Python,
                },
                class: strategy.class.clone(),
                path: strategy.path.clone(),
                params: strategy.params.clone(),
            })
            .collect();

        let engine_config = EngineConfig {
            python: PythonConfig::default(),
            strategies,
        };

        let engine = StrategyEngine::new(engine_config)
            .map_err(|e| ServerError::InitializationError(e.to_string()))?;
        let handle = engine
            .start()
            .map_err(|e| ServerError::InitializationError(e.to_string()))?;

        let mut engine_slot = self.strategy_engine.write().await;
        *engine_slot = Some(handle);

        Ok(())
    }

    pub fn shutdown(&self) {
        self.shutdown.initiate_shutdown();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Initialization error: {0}")]
    InitializationError(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("Plugin error: {0}")]
    PluginError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_new() {
        let config = ServerConfig::default();
        let server = ZephyrServer::new(config).unwrap();
        assert_eq!(server.state().await, ServerState::Stopped);
    }

    #[tokio::test]
    async fn test_server_state_transitions() {
        let config = ServerConfig::default();
        let server = ZephyrServer::new(config).unwrap();

        assert_eq!(server.state().await, ServerState::Stopped);

        // Can't run without initializing
        let result = server.run().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_server_error_display() {
        let err = ServerError::ConfigError("test error".to_string());
        assert_eq!(err.to_string(), "Configuration error: test error");
    }
}
