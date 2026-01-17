//! Strategy lifecycle management.

use dashmap::DashMap;
use std::sync::{Arc, RwLock};
use tracing::{error, info};

use zephyr_core::types::StrategyType;
use zephyr_engine::ExecutionManager;

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
}

impl StrategyManager {
    /// Creates a new strategy manager.
    pub fn new() -> Self {
        Self {
            configs: DashMap::new(),
            statuses: DashMap::new(),
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
