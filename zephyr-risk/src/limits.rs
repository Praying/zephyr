//! Risk limit configuration types.

// Allow HashMap in configuration types - these are serialized/deserialized
// and don't need thread-safe alternatives
#![allow(clippy::disallowed_types)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zephyr_core::types::{Amount, Quantity, Symbol};

/// Risk configuration for the entire system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Global risk limits.
    pub global_limits: RiskLimits,
    /// Per-symbol risk limits (overrides global).
    #[serde(default)]
    pub symbol_limits: HashMap<Symbol, SymbolLimits>,
    /// Order frequency limits.
    pub frequency_limit: FrequencyLimit,
    /// Whether to enable risk checks.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Whether to log risk events.
    #[serde(default = "default_true")]
    pub log_events: bool,
}

fn default_true() -> bool {
    true
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            global_limits: RiskLimits::default(),
            symbol_limits: HashMap::new(),
            frequency_limit: FrequencyLimit::default(),
            enabled: true,
            log_events: true,
        }
    }
}

impl RiskConfig {
    /// Creates a new `RiskConfig` with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the global position limit.
    #[must_use]
    pub fn with_max_position(mut self, limit: Quantity) -> Self {
        self.global_limits.max_position_per_symbol = Some(limit);
        self
    }

    /// Sets the daily loss limit.
    #[must_use]
    pub fn with_daily_loss_limit(mut self, limit: Amount) -> Self {
        self.global_limits.max_daily_loss = Some(limit);
        self
    }

    /// Sets the order frequency limit.
    #[must_use]
    pub fn with_frequency_limit(mut self, limit: FrequencyLimit) -> Self {
        self.frequency_limit = limit;
        self
    }

    /// Adds a symbol-specific limit.
    #[must_use]
    pub fn with_symbol_limit(mut self, symbol: Symbol, limits: SymbolLimits) -> Self {
        self.symbol_limits.insert(symbol, limits);
        self
    }

    /// Gets the position limit for a symbol.
    #[must_use]
    pub fn get_position_limit(&self, symbol: &Symbol) -> Option<Quantity> {
        self.symbol_limits
            .get(symbol)
            .and_then(|l| l.max_position)
            .or(self.global_limits.max_position_per_symbol)
    }

    /// Gets the notional limit for a symbol.
    #[must_use]
    pub fn get_notional_limit(&self, symbol: &Symbol) -> Option<Amount> {
        self.symbol_limits
            .get(symbol)
            .and_then(|l| l.max_notional)
            .or(self.global_limits.max_notional_per_symbol)
    }
}

/// Global risk limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskLimits {
    /// Maximum position size per symbol (in base currency).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_position_per_symbol: Option<Quantity>,
    /// Maximum notional value per symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_notional_per_symbol: Option<Amount>,
    /// Maximum total account exposure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_account_exposure: Option<Amount>,
    /// Maximum daily loss allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_daily_loss: Option<Amount>,
    /// Maximum number of open positions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_open_positions: Option<u32>,
    /// Maximum leverage allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_leverage: Option<u8>,
}

/// Per-symbol risk limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolLimits {
    /// Maximum position size for this symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_position: Option<Quantity>,
    /// Maximum notional value for this symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_notional: Option<Amount>,
    /// Maximum order size for this symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_order_size: Option<Quantity>,
    /// Minimum order size for this symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_order_size: Option<Quantity>,
    /// Whether trading is enabled for this symbol.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for SymbolLimits {
    fn default() -> Self {
        Self {
            max_position: None,
            max_notional: None,
            max_order_size: None,
            min_order_size: None,
            enabled: true,
        }
    }
}

impl SymbolLimits {
    /// Creates new symbol limits with a maximum position.
    #[must_use]
    pub fn with_max_position(max_position: Quantity) -> Self {
        Self {
            max_position: Some(max_position),
            ..Default::default()
        }
    }
}

/// Order frequency limit configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyLimit {
    /// Maximum orders per second.
    pub max_orders_per_second: u32,
    /// Maximum orders per minute.
    pub max_orders_per_minute: u32,
    /// Maximum orders per hour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_orders_per_hour: Option<u32>,
    /// Burst allowance (extra orders allowed in short bursts).
    #[serde(default)]
    pub burst_allowance: u32,
}

impl Default for FrequencyLimit {
    fn default() -> Self {
        Self {
            max_orders_per_second: 10,
            max_orders_per_minute: 300,
            max_orders_per_hour: Some(5000),
            burst_allowance: 5,
        }
    }
}

impl FrequencyLimit {
    /// Creates a new frequency limit with specified per-second and per-minute limits.
    #[must_use]
    pub fn new(per_second: u32, per_minute: u32) -> Self {
        Self {
            max_orders_per_second: per_second,
            max_orders_per_minute: per_minute,
            max_orders_per_hour: None,
            burst_allowance: 0,
        }
    }

    /// Sets the hourly limit.
    #[must_use]
    pub fn with_hourly_limit(mut self, limit: u32) -> Self {
        self.max_orders_per_hour = Some(limit);
        self
    }

    /// Sets the burst allowance.
    #[must_use]
    pub fn with_burst(mut self, burst: u32) -> Self {
        self.burst_allowance = burst;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_risk_config_default() {
        let config = RiskConfig::default();
        assert!(config.enabled);
        assert!(config.log_events);
        assert!(config.symbol_limits.is_empty());
    }

    #[test]
    fn test_risk_config_builder() {
        let config = RiskConfig::new()
            .with_max_position(Quantity::new(dec!(10)).unwrap())
            .with_daily_loss_limit(Amount::new(dec!(1000)).unwrap())
            .with_frequency_limit(FrequencyLimit::new(5, 100));

        assert_eq!(
            config.global_limits.max_position_per_symbol,
            Some(Quantity::new(dec!(10)).unwrap())
        );
        assert_eq!(
            config.global_limits.max_daily_loss,
            Some(Amount::new(dec!(1000)).unwrap())
        );
        assert_eq!(config.frequency_limit.max_orders_per_second, 5);
    }

    #[test]
    fn test_symbol_limits() {
        let symbol = Symbol::new("BTC-USDT").unwrap();
        let limits = SymbolLimits::with_max_position(Quantity::new(dec!(5)).unwrap());

        let config = RiskConfig::new()
            .with_max_position(Quantity::new(dec!(10)).unwrap())
            .with_symbol_limit(symbol.clone(), limits);

        // Symbol-specific limit should override global
        assert_eq!(
            config.get_position_limit(&symbol),
            Some(Quantity::new(dec!(5)).unwrap())
        );

        // Unknown symbol should use global
        let other = Symbol::new("ETH-USDT").unwrap();
        assert_eq!(
            config.get_position_limit(&other),
            Some(Quantity::new(dec!(10)).unwrap())
        );
    }

    #[test]
    fn test_frequency_limit_builder() {
        let limit = FrequencyLimit::new(10, 200)
            .with_hourly_limit(3000)
            .with_burst(10);

        assert_eq!(limit.max_orders_per_second, 10);
        assert_eq!(limit.max_orders_per_minute, 200);
        assert_eq!(limit.max_orders_per_hour, Some(3000));
        assert_eq!(limit.burst_allowance, 10);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = RiskConfig::new()
            .with_max_position(Quantity::new(dec!(10)).unwrap())
            .with_daily_loss_limit(Amount::new(dec!(1000)).unwrap());

        let json = serde_json::to_string(&config).unwrap();
        let parsed: RiskConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(
            parsed.global_limits.max_position_per_symbol,
            config.global_limits.max_position_per_symbol
        );
    }
}
