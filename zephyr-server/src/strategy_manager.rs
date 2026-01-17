//! Strategy lifecycle management.

use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::{error, info};

use zephyr_core::types::StrategyType;
use zephyr_engine::ExecutionManager;
use zephyr_engine::signal::SignalAggregator;

use crate::config::StrategyPluginConfig;

/// Strategy status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyStatus {
    Stopped,
    Running,
    Error,
}

/// Strategy manager for lifecycle management.
pub struct StrategyManager {
    configs: DashMap<String, StrategyPluginConfig>,
    statuses: DashMap<String, StrategyStatus>,
    signal_aggregator: Arc<SignalAggregator>,
}

impl StrategyManager {
    /// Creates a new strategy manager.
    pub fn new(signal_aggregator: Arc<SignalAggregator>) -> Self {
        // Create execution manager and register as listener
        let execution_manager = Arc::new(ExecutionManager::new());
        signal_aggregator.add_listener(execution_manager);

        Self {
            configs: DashMap::new(),
            statuses: DashMap::new(),
            signal_aggregator,
        }
    }

    /// Registers a strategy configuration.
    pub fn register(&self, config: StrategyPluginConfig) {
        let name = config.name.clone();
        self.configs.insert(name.clone(), config);
        self.statuses.insert(name, StrategyStatus::Stopped);
    }

    /// Starts a strategy.
    pub async fn start(&self, name: &str) -> Result<(), String> {
        if let Some(mut status) = self.statuses.get_mut(name) {
            *status = StrategyStatus::Running;
            info!("Strategy started: {}", name);
            Ok(())
        } else {
            Err(format!("Strategy not found: {}", name))
        }
    }

    /// Stops a strategy.
    pub async fn stop(&self, name: &str) -> Result<(), String> {
        if let Some(mut status) = self.statuses.get_mut(name) {
            *status = StrategyStatus::Stopped;
            info!("Strategy stopped: {}", name);
            Ok(())
        } else {
            Err(format!("Strategy not found: {}", name))
        }
    }

    /// Gets strategy status.
    pub fn get_status(&self, name: &str) -> Option<StrategyStatus> {
        self.statuses.get(name).map(|s| *s)
    }

    /// Lists all strategy names.
    pub fn list_strategies(&self) -> Vec<String> {
        self.configs.iter().map(|e| e.key().clone()).collect()
    }

    /// Gets strategy configuration.
    pub fn get_config(&self, name: &str) -> Option<StrategyPluginConfig> {
        self.configs.get(name).map(|c| c.clone())
    }
}
