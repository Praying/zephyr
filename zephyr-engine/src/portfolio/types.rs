//! Portfolio data types and structures.
//!
//! This module defines the core data structures for portfolio management.

#![allow(clippy::assign_op_pattern)]

use std::fmt;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use zephyr_core::types::{Amount, Leverage, Timestamp};

/// Unique identifier for a portfolio.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PortfolioId(String);

impl PortfolioId {
    /// Creates a new `PortfolioId`.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generates a new unique `PortfolioId`.
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Returns the ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PortfolioId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for PortfolioId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for PortfolioId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Unique identifier for a strategy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StrategyId(String);

impl StrategyId {
    /// Creates a new `StrategyId`.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StrategyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for StrategyId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for StrategyId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Portfolio status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioStatus {
    /// Portfolio is created but not active
    #[default]
    Created,
    /// Portfolio is active and trading
    Active,
    /// Portfolio is paused (no new signals processed)
    Paused,
    /// Portfolio is archived (read-only)
    Archived,
}

impl PortfolioStatus {
    /// Returns true if the portfolio can process signals.
    #[must_use]
    pub const fn can_trade(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Returns true if the portfolio is in a final state.
    #[must_use]
    pub const fn is_final(&self) -> bool {
        matches!(self, Self::Archived)
    }
}

impl fmt::Display for PortfolioStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

/// Strategy allocation within a portfolio.
///
/// Defines how a strategy contributes to the portfolio including
/// its weight and position multiplier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyAllocation {
    /// Strategy identifier.
    pub strategy_id: StrategyId,
    /// Weight of this strategy in the portfolio (0.0 to 1.0).
    /// Used for signal weighting and attribution.
    pub weight: Decimal,
    /// Position multiplier for scaling strategy positions.
    /// Allows same strategy to run with different capital allocations.
    pub position_multiplier: Decimal,
    /// Whether this strategy is enabled.
    pub enabled: bool,
}

impl StrategyAllocation {
    /// Creates a new strategy allocation.
    #[must_use]
    pub fn new(strategy_id: StrategyId, weight: Decimal, position_multiplier: Decimal) -> Self {
        Self {
            strategy_id,
            weight,
            position_multiplier,
            enabled: true,
        }
    }

    /// Creates a new strategy allocation with equal weight.
    #[must_use]
    pub fn equal_weight(strategy_id: StrategyId) -> Self {
        Self {
            strategy_id,
            weight: Decimal::ONE,
            position_multiplier: Decimal::ONE,
            enabled: true,
        }
    }

    /// Sets the enabled state.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Returns the effective weight (0 if disabled).
    #[must_use]
    pub fn effective_weight(&self) -> Decimal {
        if self.enabled {
            self.weight
        } else {
            Decimal::ZERO
        }
    }

    /// Returns the effective multiplier (0 if disabled).
    #[must_use]
    pub fn effective_multiplier(&self) -> Decimal {
        if self.enabled {
            self.position_multiplier
        } else {
            Decimal::ZERO
        }
    }
}

/// Portfolio-level risk limits.
///
/// These limits apply to the entire portfolio, independent of
/// individual strategy-level limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortfolioRiskLimits {
    /// Maximum allowed drawdown as a decimal (e.g., 0.10 for 10%).
    pub max_drawdown: Decimal,
    /// Maximum leverage allowed for the portfolio.
    pub max_leverage: Leverage,
    /// Maximum total position value across all symbols.
    pub max_position_value: Amount,
    /// Maximum daily loss limit.
    pub daily_loss_limit: Amount,
    /// Maximum number of concurrent positions.
    #[serde(default)]
    pub max_positions: Option<u32>,
    /// Maximum single position size as percentage of portfolio.
    #[serde(default)]
    pub max_single_position_pct: Option<Decimal>,
}

impl PortfolioRiskLimits {
    /// Creates new portfolio risk limits with default values.
    #[must_use]
    pub fn default_limits() -> Self {
        Self {
            max_drawdown: Decimal::new(20, 2), // 20%
            max_leverage: Leverage::new(10).unwrap_or(Leverage::ONE),
            max_position_value: Amount::new_unchecked(Decimal::new(1_000_000, 0)),
            daily_loss_limit: Amount::new_unchecked(Decimal::new(10_000, 0)),
            max_positions: Some(50),
            max_single_position_pct: Some(Decimal::new(10, 2)), // 10%
        }
    }

    /// Creates a builder for `PortfolioRiskLimits`.
    #[must_use]
    pub fn builder() -> PortfolioRiskLimitsBuilder {
        PortfolioRiskLimitsBuilder::default()
    }
}

impl Default for PortfolioRiskLimits {
    fn default() -> Self {
        Self::default_limits()
    }
}

/// Builder for `PortfolioRiskLimits`.
#[derive(Debug, Default)]
pub struct PortfolioRiskLimitsBuilder {
    max_drawdown: Option<Decimal>,
    max_leverage: Option<Leverage>,
    max_position_value: Option<Amount>,
    daily_loss_limit: Option<Amount>,
    max_positions: Option<u32>,
    max_single_position_pct: Option<Decimal>,
}

impl PortfolioRiskLimitsBuilder {
    /// Sets the maximum drawdown.
    #[must_use]
    pub fn max_drawdown(mut self, value: Decimal) -> Self {
        self.max_drawdown = Some(value);
        self
    }

    /// Sets the maximum leverage.
    #[must_use]
    pub fn max_leverage(mut self, value: Leverage) -> Self {
        self.max_leverage = Some(value);
        self
    }

    /// Sets the maximum position value.
    #[must_use]
    pub fn max_position_value(mut self, value: Amount) -> Self {
        self.max_position_value = Some(value);
        self
    }

    /// Sets the daily loss limit.
    #[must_use]
    pub fn daily_loss_limit(mut self, value: Amount) -> Self {
        self.daily_loss_limit = Some(value);
        self
    }

    /// Sets the maximum number of positions.
    #[must_use]
    pub fn max_positions(mut self, value: u32) -> Self {
        self.max_positions = Some(value);
        self
    }

    /// Sets the maximum single position percentage.
    #[must_use]
    pub fn max_single_position_pct(mut self, value: Decimal) -> Self {
        self.max_single_position_pct = Some(value);
        self
    }

    /// Builds the `PortfolioRiskLimits`.
    #[must_use]
    pub fn build(self) -> PortfolioRiskLimits {
        let defaults = PortfolioRiskLimits::default_limits();
        PortfolioRiskLimits {
            max_drawdown: self.max_drawdown.unwrap_or(defaults.max_drawdown),
            max_leverage: self.max_leverage.unwrap_or(defaults.max_leverage),
            max_position_value: self
                .max_position_value
                .unwrap_or(defaults.max_position_value),
            daily_loss_limit: self.daily_loss_limit.unwrap_or(defaults.daily_loss_limit),
            max_positions: self.max_positions.or(defaults.max_positions),
            max_single_position_pct: self
                .max_single_position_pct
                .or(defaults.max_single_position_pct),
        }
    }
}

/// Strategy portfolio containing multiple strategies.
///
/// A portfolio groups strategies together for unified execution,
/// risk management, and performance tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    /// Unique portfolio identifier.
    pub id: PortfolioId,
    /// Human-readable portfolio name.
    pub name: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Strategy allocations in this portfolio.
    pub strategies: Vec<StrategyAllocation>,
    /// Portfolio-level risk limits.
    pub risk_limits: PortfolioRiskLimits,
    /// Current portfolio status.
    pub status: PortfolioStatus,
    /// Creation timestamp.
    pub created_at: Timestamp,
    /// Last update timestamp.
    pub updated_at: Timestamp,
}

impl Portfolio {
    /// Creates a new portfolio builder.
    #[must_use]
    pub fn builder() -> PortfolioBuilder {
        PortfolioBuilder::default()
    }

    /// Returns the number of strategies in the portfolio.
    #[must_use]
    pub fn strategy_count(&self) -> usize {
        self.strategies.len()
    }

    /// Returns the number of enabled strategies.
    #[must_use]
    pub fn enabled_strategy_count(&self) -> usize {
        self.strategies.iter().filter(|s| s.enabled).count()
    }

    /// Returns true if the portfolio can process signals.
    #[must_use]
    pub fn can_trade(&self) -> bool {
        self.status.can_trade() && self.enabled_strategy_count() > 0
    }

    /// Gets a strategy allocation by ID.
    #[must_use]
    pub fn get_strategy(&self, strategy_id: &StrategyId) -> Option<&StrategyAllocation> {
        self.strategies
            .iter()
            .find(|s| &s.strategy_id == strategy_id)
    }

    /// Gets a mutable strategy allocation by ID.
    pub fn get_strategy_mut(
        &mut self,
        strategy_id: &StrategyId,
    ) -> Option<&mut StrategyAllocation> {
        self.strategies
            .iter_mut()
            .find(|s| &s.strategy_id == strategy_id)
    }

    /// Returns the total weight of all enabled strategies.
    #[must_use]
    pub fn total_weight(&self) -> Decimal {
        self.strategies
            .iter()
            .map(StrategyAllocation::effective_weight)
            .sum()
    }

    /// Normalizes strategy weights to sum to 1.0.
    pub fn normalize_weights(&mut self) {
        let total = self.total_weight();
        if total > Decimal::ZERO && total != Decimal::ONE {
            for strategy in &mut self.strategies {
                if strategy.enabled {
                    strategy.weight = strategy.weight / total;
                }
            }
        }
    }
}

/// Builder for `Portfolio`.
#[derive(Debug, Default)]
pub struct PortfolioBuilder {
    id: Option<PortfolioId>,
    name: Option<String>,
    description: Option<String>,
    strategies: Vec<StrategyAllocation>,
    risk_limits: Option<PortfolioRiskLimits>,
    status: PortfolioStatus,
}

impl PortfolioBuilder {
    /// Sets the portfolio ID.
    #[must_use]
    pub fn id(mut self, id: PortfolioId) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the portfolio name.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the portfolio description.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Adds a strategy allocation.
    #[must_use]
    pub fn add_strategy(mut self, allocation: StrategyAllocation) -> Self {
        self.strategies.push(allocation);
        self
    }

    /// Sets all strategy allocations.
    #[must_use]
    pub fn strategies(mut self, strategies: Vec<StrategyAllocation>) -> Self {
        self.strategies = strategies;
        self
    }

    /// Sets the risk limits.
    #[must_use]
    pub fn risk_limits(mut self, limits: PortfolioRiskLimits) -> Self {
        self.risk_limits = Some(limits);
        self
    }

    /// Sets the initial status.
    #[must_use]
    pub fn status(mut self, status: PortfolioStatus) -> Self {
        self.status = status;
        self
    }

    /// Builds the `Portfolio`.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<Portfolio, PortfolioBuildError> {
        let now = Timestamp::now();
        Ok(Portfolio {
            id: self.id.unwrap_or_else(PortfolioId::generate),
            name: self.name.ok_or(PortfolioBuildError::MissingField("name"))?,
            description: self.description,
            strategies: self.strategies,
            risk_limits: self.risk_limits.unwrap_or_default(),
            status: self.status,
            created_at: now,
            updated_at: now,
        })
    }
}

/// Error building a portfolio.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PortfolioBuildError {
    /// Missing required field.
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}

/// Configuration for creating a new portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioConfig {
    /// Portfolio name.
    pub name: String,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Initial strategy allocations.
    #[serde(default)]
    pub strategies: Vec<StrategyAllocation>,
    /// Risk limits.
    #[serde(default)]
    pub risk_limits: PortfolioRiskLimits,
}

impl PortfolioConfig {
    /// Creates a new portfolio configuration.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            strategies: Vec::new(),
            risk_limits: PortfolioRiskLimits::default(),
        }
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Adds a strategy allocation.
    #[must_use]
    pub fn with_strategy(mut self, allocation: StrategyAllocation) -> Self {
        self.strategies.push(allocation);
        self
    }

    /// Sets the risk limits.
    #[must_use]
    pub fn with_risk_limits(mut self, limits: PortfolioRiskLimits) -> Self {
        self.risk_limits = limits;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_portfolio_id_generate() {
        let id1 = PortfolioId::generate();
        let id2 = PortfolioId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_portfolio_id_from_string() {
        let id: PortfolioId = "test-portfolio".into();
        assert_eq!(id.as_str(), "test-portfolio");
    }

    #[test]
    fn test_strategy_id_display() {
        let id = StrategyId::new("my-strategy");
        assert_eq!(format!("{id}"), "my-strategy");
    }

    #[test]
    fn test_portfolio_status_can_trade() {
        assert!(!PortfolioStatus::Created.can_trade());
        assert!(PortfolioStatus::Active.can_trade());
        assert!(!PortfolioStatus::Paused.can_trade());
        assert!(!PortfolioStatus::Archived.can_trade());
    }

    #[test]
    fn test_strategy_allocation_new() {
        let alloc = StrategyAllocation::new(StrategyId::new("s1"), dec!(0.5), dec!(2.0));
        assert_eq!(alloc.weight, dec!(0.5));
        assert_eq!(alloc.position_multiplier, dec!(2.0));
        assert!(alloc.enabled);
    }

    #[test]
    fn test_strategy_allocation_effective_weight() {
        let mut alloc = StrategyAllocation::new(StrategyId::new("s1"), dec!(0.5), dec!(1.0));
        assert_eq!(alloc.effective_weight(), dec!(0.5));

        alloc.enabled = false;
        assert_eq!(alloc.effective_weight(), Decimal::ZERO);
    }

    #[test]
    fn test_portfolio_risk_limits_default() {
        let limits = PortfolioRiskLimits::default();
        assert_eq!(limits.max_drawdown, dec!(0.20));
        assert!(limits.max_positions.is_some());
    }

    #[test]
    fn test_portfolio_risk_limits_builder() {
        let limits = PortfolioRiskLimits::builder()
            .max_drawdown(dec!(0.15))
            .max_positions(100)
            .build();
        assert_eq!(limits.max_drawdown, dec!(0.15));
        assert_eq!(limits.max_positions, Some(100));
    }

    #[test]
    fn test_portfolio_builder() {
        let portfolio = Portfolio::builder()
            .name("Test Portfolio")
            .description("A test portfolio")
            .add_strategy(StrategyAllocation::equal_weight(StrategyId::new("s1")))
            .add_strategy(StrategyAllocation::equal_weight(StrategyId::new("s2")))
            .build()
            .unwrap();

        assert_eq!(portfolio.name, "Test Portfolio");
        assert_eq!(portfolio.strategy_count(), 2);
        assert_eq!(portfolio.enabled_strategy_count(), 2);
    }

    #[test]
    fn test_portfolio_builder_missing_name() {
        let result = Portfolio::builder().build();
        assert!(matches!(
            result,
            Err(PortfolioBuildError::MissingField("name"))
        ));
    }

    #[test]
    fn test_portfolio_can_trade() {
        let mut portfolio = Portfolio::builder()
            .name("Test")
            .add_strategy(StrategyAllocation::equal_weight(StrategyId::new("s1")))
            .status(PortfolioStatus::Active)
            .build()
            .unwrap();

        assert!(portfolio.can_trade());

        portfolio.status = PortfolioStatus::Paused;
        assert!(!portfolio.can_trade());
    }

    #[test]
    fn test_portfolio_get_strategy() {
        let portfolio = Portfolio::builder()
            .name("Test")
            .add_strategy(StrategyAllocation::new(
                StrategyId::new("s1"),
                dec!(0.6),
                dec!(1.0),
            ))
            .add_strategy(StrategyAllocation::new(
                StrategyId::new("s2"),
                dec!(0.4),
                dec!(1.0),
            ))
            .build()
            .unwrap();

        let s1 = portfolio.get_strategy(&StrategyId::new("s1"));
        assert!(s1.is_some());
        assert_eq!(s1.unwrap().weight, dec!(0.6));

        let s3 = portfolio.get_strategy(&StrategyId::new("s3"));
        assert!(s3.is_none());
    }

    #[test]
    fn test_portfolio_total_weight() {
        let portfolio = Portfolio::builder()
            .name("Test")
            .add_strategy(StrategyAllocation::new(
                StrategyId::new("s1"),
                dec!(0.6),
                dec!(1.0),
            ))
            .add_strategy(StrategyAllocation::new(
                StrategyId::new("s2"),
                dec!(0.4),
                dec!(1.0),
            ))
            .build()
            .unwrap();

        assert_eq!(portfolio.total_weight(), dec!(1.0));
    }

    #[test]
    fn test_portfolio_normalize_weights() {
        let mut portfolio = Portfolio::builder()
            .name("Test")
            .add_strategy(StrategyAllocation::new(
                StrategyId::new("s1"),
                dec!(3.0),
                dec!(1.0),
            ))
            .add_strategy(StrategyAllocation::new(
                StrategyId::new("s2"),
                dec!(1.0),
                dec!(1.0),
            ))
            .build()
            .unwrap();

        portfolio.normalize_weights();

        assert_eq!(
            portfolio
                .get_strategy(&StrategyId::new("s1"))
                .unwrap()
                .weight,
            dec!(0.75)
        );
        assert_eq!(
            portfolio
                .get_strategy(&StrategyId::new("s2"))
                .unwrap()
                .weight,
            dec!(0.25)
        );
    }

    #[test]
    fn test_portfolio_config() {
        let config = PortfolioConfig::new("My Portfolio")
            .with_description("Test description")
            .with_strategy(StrategyAllocation::equal_weight(StrategyId::new("s1")));

        assert_eq!(config.name, "My Portfolio");
        assert_eq!(config.description, Some("Test description".to_string()));
        assert_eq!(config.strategies.len(), 1);
    }

    #[test]
    fn test_portfolio_serde_roundtrip() {
        let portfolio = Portfolio::builder()
            .name("Test Portfolio")
            .add_strategy(StrategyAllocation::new(
                StrategyId::new("s1"),
                dec!(0.5),
                dec!(2.0),
            ))
            .status(PortfolioStatus::Active)
            .build()
            .unwrap();

        let json = serde_json::to_string(&portfolio).unwrap();
        let parsed: Portfolio = serde_json::from_str(&json).unwrap();

        assert_eq!(portfolio.name, parsed.name);
        assert_eq!(portfolio.strategies.len(), parsed.strategies.len());
    }
}
