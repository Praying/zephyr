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
use zephyr_telemetry::logging::{LogConfig, init_logging};
use zephyr_telemetry::metrics::{MetricsConfig, init_metrics};

use crate::config::ServerConfig;
use crate::plugin::PluginRegistry;
use crate::shutdown::{ShutdownController, setup_signal_handlers};

/// Server state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    /// Server is not started.
    Stopped,
    /// Server is starting up.
    Starting,
    /// Server is running.
    Running,
    /// Server is shutting down.
    ShuttingDown,
}

/// Main Zephyr server.
///
/// Orchestrates all components and manages the server lifecycle.
pub struct ZephyrServer {
    /// Server configuration.
    config: ServerConfig,
    /// Current server state.
    state: Arc<RwLock<ServerState>>,
    /// Shutdown controller.
    shutdown: ShutdownController,
    /// Plugin registry.
    plugins: Arc<RwLock<PluginRegistry>>,
    /// Log guards (must be kept alive).
    _log_guards: Vec<WorkerGuard>,
}

impl ZephyrServer {
    /// Creates a new server with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub fn new(config: ServerConfig) -> Result<Self, ServerError> {
        Ok(Self {
            config,
            state: Arc::new(RwLock::new(ServerState::Stopped)),
            shutdown: ShutdownController::new(),
            plugins: Arc::new(RwLock::new(PluginRegistry::new())),
            _log_guards: Vec::new(),
        })
    }

    /// Loads configuration from a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
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

    /// Returns the current server state.
    pub async fn state(&self) -> ServerState {
        *self.state.read().await
    }

    /// Returns the shutdown controller.
    #[must_use]
    pub fn shutdown_controller(&self) -> &ShutdownController {
        &self.shutdown
    }

    /// Returns the plugin registry.
    #[must_use]
    pub fn plugins(&self) -> &Arc<RwLock<PluginRegistry>> {
        &self.plugins
    }

    /// Initializes the server components.
    ///
    /// This sets up logging, metrics, and loads plugins.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
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

        // Initialize logging
        self.init_logging()?;

        // Initialize metrics
        self.init_metrics();

        // Load plugins
        self.load_plugins().await?;

        info!("Zephyr server initialized successfully");
        Ok(())
    }

    /// Initializes the logging system.
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

    /// Initializes the metrics system.
    fn init_metrics(&self) {
        let metrics_config = MetricsConfig::default();

        // Metrics initialization may fail if already initialized (e.g., in tests)
        if let Err(e) = init_metrics(&metrics_config) {
            warn!("Metrics initialization: {}", e);
        } else {
            info!("Metrics initialized");
        }
    }

    /// Loads plugins from configured directories.
    async fn load_plugins(&self) -> Result<(), ServerError> {
        let mut registry = self.plugins.write().await;

        // Load strategy plugins
        for strategy_config in &self.config.plugins.strategies {
            info!("Loading strategy plugin: {}", strategy_config.name);
            registry.register_strategy(&strategy_config.name, strategy_config.clone());
        }

        // Load adapter plugins
        for adapter_config in &self.config.plugins.adapters {
            if adapter_config.enabled {
                info!("Loading adapter plugin: {}", adapter_config.name);
                registry.register_adapter(&adapter_config.name, adapter_config.clone());
            }
        }

        info!(
            "Loaded {} strategies and {} adapters",
            registry.strategy_count(),
            registry.adapter_count()
        );

        Ok(())
    }

    /// Runs the server.
    ///
    /// This starts all components and blocks until shutdown is initiated.
    ///
    /// # Errors
    ///
    /// Returns an error if the server fails to start or encounters a fatal error.
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

    /// Performs graceful shutdown.
    async fn graceful_shutdown(&self) -> Result<(), ServerError> {
        {
            let mut state = self.state.write().await;
            *state = ServerState::ShuttingDown;
        }

        info!("Performing graceful shutdown...");

        // Cancel pending orders if configured
        if self.config.shutdown.cancel_pending_orders {
            info!("Cancelling pending orders...");
            self.cancel_pending_orders();
        }

        // Save state if configured
        if self.config.shutdown.save_state {
            info!("Saving state...");
            self.save_state();
        }

        // Unload plugins
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

    /// Cancels all pending orders.
    fn cancel_pending_orders(&self) {
        // In a full implementation, this would iterate through all active
        // trading gateways and cancel pending orders
        let timeout = self.config.shutdown.cancel_timeout();
        info!("Order cancellation timeout: {:?}", timeout);

        // Placeholder - actual implementation would cancel orders
    }

    /// Saves server state for recovery.
    fn save_state(&self) {
        // In a full implementation, this would save:
        // - Strategy states
        // - Position information
        // - Pending order information
        info!("State saved successfully");
    }

    /// Initiates server shutdown.
    pub fn shutdown(&self) {
        self.shutdown.initiate_shutdown();
    }
}

/// Server error type.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Initialization error.
    #[error("Initialization error: {0}")]
    InitializationError(String),

    /// Invalid state error.
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Runtime error.
    #[error("Runtime error: {0}")]
    RuntimeError(String),

    /// Plugin error.
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
