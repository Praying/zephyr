//! Portfolio manager implementation.
//!
//! This module provides the portfolio management functionality including
//! CRUD operations, weight updates, and signal merging coordination.

#![allow(clippy::disallowed_types)]
#![allow(clippy::significant_drop_tightening)]

use std::collections::HashMap;

use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use rust_decimal::Decimal;
use thiserror::Error;
use tracing::{debug, info, warn};

use zephyr_core::data::OrderRequest;
use zephyr_core::types::{Quantity, Symbol, Timestamp};

use super::{
    ExecutionSignal, MergeConfig, Portfolio, PortfolioConfig, PortfolioId, PortfolioMetrics,
    PortfolioStatus, SignalMerger, StrategyAllocation, StrategyId,
};
use crate::StrategySignal;

/// Portfolio management errors.
#[derive(Debug, Error)]
pub enum PortfolioError {
    /// Portfolio not found.
    #[error("portfolio not found: {0}")]
    NotFound(PortfolioId),

    /// Portfolio already exists.
    #[error("portfolio already exists: {0}")]
    AlreadyExists(PortfolioId),

    /// Portfolio is not active.
    #[error("portfolio is not active: {0} (status: {1})")]
    NotActive(PortfolioId, PortfolioStatus),

    /// Strategy not found in portfolio.
    #[error("strategy not found in portfolio: {0}")]
    StrategyNotFound(StrategyId),

    /// Invalid weight value.
    #[error("invalid weight value: {0} (must be between 0 and 1)")]
    InvalidWeight(Decimal),

    /// Invalid multiplier value.
    #[error("invalid multiplier value: {0} (must be positive)")]
    InvalidMultiplier(Decimal),

    /// Portfolio is archived and cannot be modified.
    #[error("portfolio is archived: {0}")]
    Archived(PortfolioId),

    /// Risk limit exceeded.
    #[error("risk limit exceeded: {0}")]
    RiskLimitExceeded(String),

    /// Validation error.
    #[error("validation error: {0}")]
    Validation(String),
}

/// Portfolio manager trait.
///
/// Defines the interface for portfolio management operations.
#[async_trait]
pub trait PortfolioManager: Send + Sync {
    /// Creates a new portfolio.
    async fn create_portfolio(
        &self,
        config: PortfolioConfig,
    ) -> Result<PortfolioId, PortfolioError>;

    /// Gets a portfolio by ID.
    fn get_portfolio(&self, id: &PortfolioId) -> Option<Portfolio>;

    /// Lists all portfolios.
    fn list_portfolios(&self) -> Vec<Portfolio>;

    /// Updates a portfolio's status.
    async fn update_status(
        &self,
        id: &PortfolioId,
        status: PortfolioStatus,
    ) -> Result<(), PortfolioError>;

    /// Updates strategy weights in a portfolio.
    async fn update_weights(
        &self,
        id: &PortfolioId,
        weights: HashMap<StrategyId, Decimal>,
    ) -> Result<(), PortfolioError>;

    /// Updates strategy multipliers in a portfolio.
    async fn update_multipliers(
        &self,
        id: &PortfolioId,
        multipliers: HashMap<StrategyId, Decimal>,
    ) -> Result<(), PortfolioError>;

    /// Adds a strategy to a portfolio.
    async fn add_strategy(
        &self,
        id: &PortfolioId,
        allocation: StrategyAllocation,
    ) -> Result<(), PortfolioError>;

    /// Removes a strategy from a portfolio.
    async fn remove_strategy(
        &self,
        id: &PortfolioId,
        strategy_id: &StrategyId,
    ) -> Result<(), PortfolioError>;

    /// Gets portfolio positions.
    fn get_portfolio_positions(&self, id: &PortfolioId) -> HashMap<Symbol, Quantity>;

    /// Gets portfolio metrics.
    fn get_portfolio_metrics(&self, id: &PortfolioId) -> Option<PortfolioMetrics>;

    /// Merges strategy signals for a portfolio.
    fn merge_signals(
        &self,
        id: &PortfolioId,
        signals: Vec<StrategySignal>,
    ) -> Result<Vec<ExecutionSignal>, PortfolioError>;

    /// Triggers portfolio rebalancing.
    async fn rebalance(&self, id: &PortfolioId) -> Result<Vec<OrderRequest>, PortfolioError>;

    /// Deletes a portfolio.
    async fn delete_portfolio(&self, id: &PortfolioId) -> Result<(), PortfolioError>;

    /// Clones a portfolio with a new name.
    async fn clone_portfolio(
        &self,
        id: &PortfolioId,
        new_name: String,
    ) -> Result<PortfolioId, PortfolioError>;
}

/// Portfolio manager implementation.
///
/// Thread-safe implementation of the portfolio manager using `DashMap`
/// for concurrent access.
pub struct PortfolioManagerImpl {
    /// Portfolios indexed by ID.
    portfolios: DashMap<PortfolioId, Portfolio>,
    /// Signal mergers per portfolio.
    mergers: DashMap<PortfolioId, RwLock<SignalMerger>>,
    /// Portfolio metrics per portfolio.
    metrics: DashMap<PortfolioId, RwLock<PortfolioMetrics>>,
    /// Current positions per portfolio per symbol.
    positions: DashMap<PortfolioId, HashMap<Symbol, Quantity>>,
    /// Default merge configuration.
    default_merge_config: MergeConfig,
}

impl PortfolioManagerImpl {
    /// Creates a new portfolio manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            portfolios: DashMap::new(),
            mergers: DashMap::new(),
            metrics: DashMap::new(),
            positions: DashMap::new(),
            default_merge_config: MergeConfig::default(),
        }
    }

    /// Creates a new portfolio manager with custom merge configuration.
    #[must_use]
    pub fn with_merge_config(merge_config: MergeConfig) -> Self {
        Self {
            portfolios: DashMap::new(),
            mergers: DashMap::new(),
            metrics: DashMap::new(),
            positions: DashMap::new(),
            default_merge_config: merge_config,
        }
    }

    /// Updates the current position for a portfolio and symbol.
    pub fn update_position(&self, portfolio_id: &PortfolioId, symbol: &Symbol, quantity: Quantity) {
        self.positions
            .entry(portfolio_id.clone())
            .or_default()
            .insert(symbol.clone(), quantity);

        // Update merger's current position
        if let Some(merger) = self.mergers.get(portfolio_id) {
            merger.write().update_current_position(symbol, quantity);
        }
    }

    /// Gets the current position for a portfolio and symbol.
    #[must_use]
    pub fn get_position(&self, portfolio_id: &PortfolioId, symbol: &Symbol) -> Quantity {
        self.positions
            .get(portfolio_id)
            .and_then(|p| p.get(symbol).copied())
            .unwrap_or(Quantity::ZERO)
    }

    /// Validates a portfolio configuration.
    fn validate_config(config: &PortfolioConfig) -> Result<(), PortfolioError> {
        if config.name.is_empty() {
            return Err(PortfolioError::Validation(
                "name cannot be empty".to_string(),
            ));
        }

        // Validate strategy allocations
        for alloc in &config.strategies {
            if alloc.weight < Decimal::ZERO || alloc.weight > Decimal::ONE {
                return Err(PortfolioError::InvalidWeight(alloc.weight));
            }
            if alloc.position_multiplier <= Decimal::ZERO {
                return Err(PortfolioError::InvalidMultiplier(alloc.position_multiplier));
            }
        }

        Ok(())
    }

    /// Checks if a portfolio can be modified.
    fn check_modifiable(&self, id: &PortfolioId) -> Result<(), PortfolioError> {
        let portfolio = self
            .portfolios
            .get(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        if portfolio.status == PortfolioStatus::Archived {
            return Err(PortfolioError::Archived(id.clone()));
        }

        Ok(())
    }
}

impl Default for PortfolioManagerImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PortfolioManager for PortfolioManagerImpl {
    async fn create_portfolio(
        &self,
        config: PortfolioConfig,
    ) -> Result<PortfolioId, PortfolioError> {
        Self::validate_config(&config)?;

        let portfolio = Portfolio::builder()
            .name(&config.name)
            .description(config.description.unwrap_or_default())
            .strategies(config.strategies)
            .risk_limits(config.risk_limits)
            .status(PortfolioStatus::Created)
            .build()
            .map_err(|e| PortfolioError::Validation(e.to_string()))?;

        let id = portfolio.id.clone();

        if self.portfolios.contains_key(&id) {
            return Err(PortfolioError::AlreadyExists(id));
        }

        info!(portfolio_id = %id, name = %portfolio.name, "Creating portfolio");

        // Initialize merger and metrics
        self.mergers.insert(
            id.clone(),
            RwLock::new(SignalMerger::with_config(self.default_merge_config.clone())),
        );
        self.metrics
            .insert(id.clone(), RwLock::new(PortfolioMetrics::new()));
        self.positions.insert(id.clone(), HashMap::new());
        self.portfolios.insert(id.clone(), portfolio);

        Ok(id)
    }

    fn get_portfolio(&self, id: &PortfolioId) -> Option<Portfolio> {
        self.portfolios.get(id).map(|p| p.clone())
    }

    fn list_portfolios(&self) -> Vec<Portfolio> {
        self.portfolios.iter().map(|p| p.clone()).collect()
    }

    async fn update_status(
        &self,
        id: &PortfolioId,
        status: PortfolioStatus,
    ) -> Result<(), PortfolioError> {
        let mut portfolio = self
            .portfolios
            .get_mut(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        // Cannot transition from archived
        if portfolio.status == PortfolioStatus::Archived && status != PortfolioStatus::Archived {
            return Err(PortfolioError::Archived(id.clone()));
        }

        info!(
            portfolio_id = %id,
            old_status = %portfolio.status,
            new_status = %status,
            "Updating portfolio status"
        );

        portfolio.status = status;
        portfolio.updated_at = Timestamp::now();

        Ok(())
    }

    async fn update_weights(
        &self,
        id: &PortfolioId,
        weights: HashMap<StrategyId, Decimal>,
    ) -> Result<(), PortfolioError> {
        self.check_modifiable(id)?;

        let mut portfolio = self
            .portfolios
            .get_mut(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        for (strategy_id, weight) in weights {
            if weight < Decimal::ZERO || weight > Decimal::ONE {
                return Err(PortfolioError::InvalidWeight(weight));
            }

            let strategy = portfolio
                .get_strategy_mut(&strategy_id)
                .ok_or_else(|| PortfolioError::StrategyNotFound(strategy_id.clone()))?;

            debug!(
                portfolio_id = %id,
                strategy_id = %strategy_id,
                old_weight = %strategy.weight,
                new_weight = %weight,
                "Updating strategy weight"
            );

            strategy.weight = weight;
        }

        portfolio.updated_at = Timestamp::now();

        Ok(())
    }

    async fn update_multipliers(
        &self,
        id: &PortfolioId,
        multipliers: HashMap<StrategyId, Decimal>,
    ) -> Result<(), PortfolioError> {
        self.check_modifiable(id)?;

        let mut portfolio = self
            .portfolios
            .get_mut(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        for (strategy_id, multiplier) in multipliers {
            if multiplier <= Decimal::ZERO {
                return Err(PortfolioError::InvalidMultiplier(multiplier));
            }

            let strategy = portfolio
                .get_strategy_mut(&strategy_id)
                .ok_or_else(|| PortfolioError::StrategyNotFound(strategy_id.clone()))?;

            debug!(
                portfolio_id = %id,
                strategy_id = %strategy_id,
                old_multiplier = %strategy.position_multiplier,
                new_multiplier = %multiplier,
                "Updating strategy multiplier"
            );

            strategy.position_multiplier = multiplier;
        }

        portfolio.updated_at = Timestamp::now();

        Ok(())
    }

    async fn add_strategy(
        &self,
        id: &PortfolioId,
        allocation: StrategyAllocation,
    ) -> Result<(), PortfolioError> {
        self.check_modifiable(id)?;

        if allocation.weight < Decimal::ZERO || allocation.weight > Decimal::ONE {
            return Err(PortfolioError::InvalidWeight(allocation.weight));
        }
        if allocation.position_multiplier <= Decimal::ZERO {
            return Err(PortfolioError::InvalidMultiplier(
                allocation.position_multiplier,
            ));
        }

        let mut portfolio = self
            .portfolios
            .get_mut(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        // Check if strategy already exists
        if portfolio.get_strategy(&allocation.strategy_id).is_some() {
            return Err(PortfolioError::Validation(format!(
                "strategy {} already exists in portfolio",
                allocation.strategy_id
            )));
        }

        info!(
            portfolio_id = %id,
            strategy_id = %allocation.strategy_id,
            weight = %allocation.weight,
            multiplier = %allocation.position_multiplier,
            "Adding strategy to portfolio"
        );

        portfolio.strategies.push(allocation);
        portfolio.updated_at = Timestamp::now();

        Ok(())
    }

    async fn remove_strategy(
        &self,
        id: &PortfolioId,
        strategy_id: &StrategyId,
    ) -> Result<(), PortfolioError> {
        self.check_modifiable(id)?;

        let mut portfolio = self
            .portfolios
            .get_mut(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        let initial_len = portfolio.strategies.len();
        portfolio
            .strategies
            .retain(|s| &s.strategy_id != strategy_id);

        if portfolio.strategies.len() == initial_len {
            return Err(PortfolioError::StrategyNotFound(strategy_id.clone()));
        }

        info!(
            portfolio_id = %id,
            strategy_id = %strategy_id,
            "Removed strategy from portfolio"
        );

        // Remove from merger
        if let Some(merger) = self.mergers.get(id) {
            merger.write().remove_strategy(strategy_id);
        }

        portfolio.updated_at = Timestamp::now();

        Ok(())
    }

    fn get_portfolio_positions(&self, id: &PortfolioId) -> HashMap<Symbol, Quantity> {
        self.positions
            .get(id)
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    fn get_portfolio_metrics(&self, id: &PortfolioId) -> Option<PortfolioMetrics> {
        self.metrics.get(id).map(|m| m.read().clone())
    }

    fn merge_signals(
        &self,
        id: &PortfolioId,
        signals: Vec<StrategySignal>,
    ) -> Result<Vec<ExecutionSignal>, PortfolioError> {
        let portfolio = self
            .portfolios
            .get(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        if !portfolio.can_trade() {
            return Err(PortfolioError::NotActive(id.clone(), portfolio.status));
        }

        let merger = self
            .mergers
            .get(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        let mut merger_guard = merger.write();

        // Add all signals
        for signal in signals {
            merger_guard.add_signal(signal);
        }

        // Merge and return execution signals
        Ok(merger_guard.merge_signals(&portfolio))
    }

    async fn rebalance(&self, id: &PortfolioId) -> Result<Vec<OrderRequest>, PortfolioError> {
        let portfolio = self
            .portfolios
            .get(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        if !portfolio.can_trade() {
            return Err(PortfolioError::NotActive(id.clone(), portfolio.status));
        }

        // TODO: Implement rebalancing logic
        // This would:
        // 1. Get current positions
        // 2. Calculate target positions based on strategy weights
        // 3. Generate order requests to move from current to target

        warn!(portfolio_id = %id, "Rebalancing not yet implemented");
        Ok(Vec::new())
    }

    async fn delete_portfolio(&self, id: &PortfolioId) -> Result<(), PortfolioError> {
        let portfolio = self
            .portfolios
            .get(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        // Only allow deletion of non-active portfolios
        if portfolio.status == PortfolioStatus::Active {
            return Err(PortfolioError::Validation(
                "cannot delete active portfolio, pause or archive first".to_string(),
            ));
        }

        drop(portfolio);

        info!(portfolio_id = %id, "Deleting portfolio");

        self.portfolios.remove(id);
        self.mergers.remove(id);
        self.metrics.remove(id);
        self.positions.remove(id);

        Ok(())
    }

    async fn clone_portfolio(
        &self,
        id: &PortfolioId,
        new_name: String,
    ) -> Result<PortfolioId, PortfolioError> {
        let portfolio = self
            .portfolios
            .get(id)
            .ok_or_else(|| PortfolioError::NotFound(id.clone()))?;

        let config = PortfolioConfig {
            name: new_name,
            description: portfolio.description.clone(),
            strategies: portfolio.strategies.clone(),
            risk_limits: portfolio.risk_limits.clone(),
        };

        drop(portfolio);

        self.create_portfolio(config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_config() -> PortfolioConfig {
        PortfolioConfig::new("Test Portfolio")
            .with_description("A test portfolio")
            .with_strategy(StrategyAllocation::new(
                StrategyId::new("s1"),
                dec!(0.6),
                dec!(1.0),
            ))
            .with_strategy(StrategyAllocation::new(
                StrategyId::new("s2"),
                dec!(0.4),
                dec!(1.0),
            ))
    }

    fn create_signal(strategy: &str, symbol: &str, qty: Decimal) -> StrategySignal {
        StrategySignal {
            strategy_name: strategy.to_string(),
            symbol: Symbol::new(symbol).unwrap(),
            target_position: Quantity::new(qty).unwrap(),
            tag: "test".to_string(),
            timestamp: Timestamp::now(),
        }
    }

    #[tokio::test]
    async fn test_create_portfolio() {
        let manager = PortfolioManagerImpl::new();
        let config = create_test_config();

        let id = manager.create_portfolio(config).await.unwrap();

        let portfolio = manager.get_portfolio(&id).unwrap();
        assert_eq!(portfolio.name, "Test Portfolio");
        assert_eq!(portfolio.strategy_count(), 2);
        assert_eq!(portfolio.status, PortfolioStatus::Created);
    }

    #[tokio::test]
    async fn test_create_portfolio_empty_name() {
        let manager = PortfolioManagerImpl::new();
        let config = PortfolioConfig::new("");

        let result = manager.create_portfolio(config).await;
        assert!(matches!(result, Err(PortfolioError::Validation(_))));
    }

    #[tokio::test]
    async fn test_create_portfolio_invalid_weight() {
        let manager = PortfolioManagerImpl::new();
        let config = PortfolioConfig::new("Test").with_strategy(StrategyAllocation::new(
            StrategyId::new("s1"),
            dec!(1.5), // Invalid: > 1
            dec!(1.0),
        ));

        let result = manager.create_portfolio(config).await;
        assert!(matches!(result, Err(PortfolioError::InvalidWeight(_))));
    }

    #[tokio::test]
    async fn test_list_portfolios() {
        let manager = PortfolioManagerImpl::new();

        manager
            .create_portfolio(PortfolioConfig::new("Portfolio 1"))
            .await
            .unwrap();
        manager
            .create_portfolio(PortfolioConfig::new("Portfolio 2"))
            .await
            .unwrap();

        let portfolios = manager.list_portfolios();
        assert_eq!(portfolios.len(), 2);
    }

    #[tokio::test]
    async fn test_update_status() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        manager
            .update_status(&id, PortfolioStatus::Active)
            .await
            .unwrap();

        let portfolio = manager.get_portfolio(&id).unwrap();
        assert_eq!(portfolio.status, PortfolioStatus::Active);
    }

    #[tokio::test]
    async fn test_update_status_archived_cannot_change() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        manager
            .update_status(&id, PortfolioStatus::Archived)
            .await
            .unwrap();

        let result = manager.update_status(&id, PortfolioStatus::Active).await;
        assert!(matches!(result, Err(PortfolioError::Archived(_))));
    }

    #[tokio::test]
    async fn test_update_weights() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        let mut weights = HashMap::new();
        weights.insert(StrategyId::new("s1"), dec!(0.7));
        weights.insert(StrategyId::new("s2"), dec!(0.3));

        manager.update_weights(&id, weights).await.unwrap();

        let portfolio = manager.get_portfolio(&id).unwrap();
        assert_eq!(
            portfolio
                .get_strategy(&StrategyId::new("s1"))
                .unwrap()
                .weight,
            dec!(0.7)
        );
    }

    #[tokio::test]
    async fn test_update_weights_invalid() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        let mut weights = HashMap::new();
        weights.insert(StrategyId::new("s1"), dec!(-0.1)); // Invalid

        let result = manager.update_weights(&id, weights).await;
        assert!(matches!(result, Err(PortfolioError::InvalidWeight(_))));
    }

    #[tokio::test]
    async fn test_update_multipliers() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        let mut multipliers = HashMap::new();
        multipliers.insert(StrategyId::new("s1"), dec!(2.0));

        manager.update_multipliers(&id, multipliers).await.unwrap();

        let portfolio = manager.get_portfolio(&id).unwrap();
        assert_eq!(
            portfolio
                .get_strategy(&StrategyId::new("s1"))
                .unwrap()
                .position_multiplier,
            dec!(2.0)
        );
    }

    #[tokio::test]
    async fn test_add_strategy() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(PortfolioConfig::new("Test"))
            .await
            .unwrap();

        let allocation = StrategyAllocation::new(StrategyId::new("s1"), dec!(1.0), dec!(1.0));
        manager.add_strategy(&id, allocation).await.unwrap();

        let portfolio = manager.get_portfolio(&id).unwrap();
        assert_eq!(portfolio.strategy_count(), 1);
    }

    #[tokio::test]
    async fn test_add_strategy_duplicate() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        let allocation = StrategyAllocation::new(StrategyId::new("s1"), dec!(0.5), dec!(1.0));
        let result = manager.add_strategy(&id, allocation).await;
        assert!(matches!(result, Err(PortfolioError::Validation(_))));
    }

    #[tokio::test]
    async fn test_remove_strategy() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        manager
            .remove_strategy(&id, &StrategyId::new("s1"))
            .await
            .unwrap();

        let portfolio = manager.get_portfolio(&id).unwrap();
        assert_eq!(portfolio.strategy_count(), 1);
        assert!(portfolio.get_strategy(&StrategyId::new("s1")).is_none());
    }

    #[tokio::test]
    async fn test_remove_strategy_not_found() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        let result = manager
            .remove_strategy(&id, &StrategyId::new("nonexistent"))
            .await;
        assert!(matches!(result, Err(PortfolioError::StrategyNotFound(_))));
    }

    #[tokio::test]
    async fn test_get_portfolio_positions() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        let btc = Symbol::new("BTC-USDT").unwrap();
        manager.update_position(&id, &btc, Quantity::new(dec!(1.0)).unwrap());

        let positions = manager.get_portfolio_positions(&id);
        assert_eq!(positions.get(&btc).unwrap().as_decimal(), dec!(1.0));
    }

    #[tokio::test]
    async fn test_merge_signals() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        // Activate portfolio
        manager
            .update_status(&id, PortfolioStatus::Active)
            .await
            .unwrap();

        let signals = vec![
            create_signal("s1", "BTC-USDT", dec!(1.0)),
            create_signal("s2", "BTC-USDT", dec!(2.0)),
        ];

        let exec_signals = manager.merge_signals(&id, signals).unwrap();
        assert_eq!(exec_signals.len(), 1);
        assert_eq!(exec_signals[0].target_position.as_decimal(), dec!(3.0));
    }

    #[tokio::test]
    async fn test_merge_signals_not_active() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        let signals = vec![create_signal("s1", "BTC-USDT", dec!(1.0))];

        let result = manager.merge_signals(&id, signals);
        assert!(matches!(result, Err(PortfolioError::NotActive(_, _))));
    }

    #[tokio::test]
    async fn test_delete_portfolio() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        manager.delete_portfolio(&id).await.unwrap();

        assert!(manager.get_portfolio(&id).is_none());
    }

    #[tokio::test]
    async fn test_delete_active_portfolio_fails() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        manager
            .update_status(&id, PortfolioStatus::Active)
            .await
            .unwrap();

        let result = manager.delete_portfolio(&id).await;
        assert!(matches!(result, Err(PortfolioError::Validation(_))));
    }

    #[tokio::test]
    async fn test_clone_portfolio() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        let new_id = manager
            .clone_portfolio(&id, "Cloned Portfolio".to_string())
            .await
            .unwrap();

        let original = manager.get_portfolio(&id).unwrap();
        let cloned = manager.get_portfolio(&new_id).unwrap();

        assert_ne!(original.id, cloned.id);
        assert_eq!(cloned.name, "Cloned Portfolio");
        assert_eq!(original.strategies.len(), cloned.strategies.len());
    }

    #[tokio::test]
    async fn test_get_portfolio_metrics() {
        let manager = PortfolioManagerImpl::new();
        let id = manager
            .create_portfolio(create_test_config())
            .await
            .unwrap();

        let metrics = manager.get_portfolio_metrics(&id);
        assert!(metrics.is_some());
    }
}
