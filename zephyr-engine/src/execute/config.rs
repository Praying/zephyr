//! Configuration types for execution units.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Execution configuration.
///
/// Contains common configuration options for all execution units.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteConfig {
    /// Position multiplier for scaling.
    #[serde(default = "default_multiplier")]
    pub multiplier: Decimal,

    /// Slippage configuration.
    #[serde(default)]
    pub slippage: SlippageConfig,

    /// Whether to use aggressive pricing (cross the spread).
    #[serde(default)]
    pub aggressive: bool,

    /// Maximum order size per submission.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_order_size: Option<Decimal>,

    /// Minimum order size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_order_size: Option<Decimal>,

    /// Price tick size for rounding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_size: Option<Decimal>,

    /// Quantity step size for rounding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_size: Option<Decimal>,

    /// Timeout for order fills before cancellation.
    #[serde(default = "default_order_timeout")]
    #[serde(with = "humantime_serde")]
    pub order_timeout: Duration,

    /// Whether to cancel unfilled orders on completion.
    #[serde(default = "default_true")]
    pub cancel_on_complete: bool,

    /// Whether to retry failed orders.
    #[serde(default = "default_true")]
    pub retry_on_failure: bool,

    /// Maximum retry attempts.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_multiplier() -> Decimal {
    Decimal::ONE
}

fn default_order_timeout() -> Duration {
    Duration::from_secs(60)
}

fn default_true() -> bool {
    true
}

fn default_max_retries() -> u32 {
    3
}

impl Default for ExecuteConfig {
    fn default() -> Self {
        Self {
            multiplier: default_multiplier(),
            slippage: SlippageConfig::default(),
            aggressive: false,
            max_order_size: None,
            min_order_size: None,
            tick_size: None,
            step_size: None,
            order_timeout: default_order_timeout(),
            cancel_on_complete: true,
            retry_on_failure: true,
            max_retries: default_max_retries(),
        }
    }
}

impl ExecuteConfig {
    /// Creates a new builder for `ExecuteConfig`.
    #[must_use]
    pub fn builder() -> ExecuteConfigBuilder {
        ExecuteConfigBuilder::default()
    }

    /// Rounds a price to the configured tick size.
    #[must_use]
    pub fn round_price(&self, price: Decimal) -> Decimal {
        if let Some(tick) = self.tick_size {
            if tick > Decimal::ZERO {
                return (price / tick).round() * tick;
            }
        }
        price
    }

    /// Rounds a quantity to the configured step size.
    #[must_use]
    pub fn round_quantity(&self, qty: Decimal) -> Decimal {
        if let Some(step) = self.step_size {
            if step > Decimal::ZERO {
                return (qty / step).floor() * step;
            }
        }
        qty
    }

    /// Clamps quantity to min/max bounds.
    #[must_use]
    pub fn clamp_quantity(&self, qty: Decimal) -> Decimal {
        let mut result = qty;
        if let Some(min) = self.min_order_size {
            if result < min {
                result = Decimal::ZERO; // Below minimum, don't order
            }
        }
        if let Some(max) = self.max_order_size {
            if result > max {
                result = max;
            }
        }
        result
    }
}

/// Builder for `ExecuteConfig`.
#[derive(Debug, Default)]
pub struct ExecuteConfigBuilder {
    config: ExecuteConfig,
}

impl ExecuteConfigBuilder {
    /// Sets the position multiplier.
    #[must_use]
    pub fn multiplier(mut self, multiplier: Decimal) -> Self {
        self.config.multiplier = multiplier;
        self
    }

    /// Sets the slippage configuration.
    #[must_use]
    pub fn slippage(mut self, slippage: SlippageConfig) -> Self {
        self.config.slippage = slippage;
        self
    }

    /// Sets aggressive pricing mode.
    #[must_use]
    pub fn aggressive(mut self, aggressive: bool) -> Self {
        self.config.aggressive = aggressive;
        self
    }

    /// Sets the maximum order size.
    #[must_use]
    pub fn max_order_size(mut self, size: Decimal) -> Self {
        self.config.max_order_size = Some(size);
        self
    }

    /// Sets the minimum order size.
    #[must_use]
    pub fn min_order_size(mut self, size: Decimal) -> Self {
        self.config.min_order_size = Some(size);
        self
    }

    /// Sets the price tick size.
    #[must_use]
    pub fn tick_size(mut self, tick: Decimal) -> Self {
        self.config.tick_size = Some(tick);
        self
    }

    /// Sets the quantity step size.
    #[must_use]
    pub fn step_size(mut self, step: Decimal) -> Self {
        self.config.step_size = Some(step);
        self
    }

    /// Sets the order timeout.
    #[must_use]
    pub fn order_timeout(mut self, timeout: Duration) -> Self {
        self.config.order_timeout = timeout;
        self
    }

    /// Sets whether to cancel on completion.
    #[must_use]
    pub fn cancel_on_complete(mut self, cancel: bool) -> Self {
        self.config.cancel_on_complete = cancel;
        self
    }

    /// Sets whether to retry on failure.
    #[must_use]
    pub fn retry_on_failure(mut self, retry: bool) -> Self {
        self.config.retry_on_failure = retry;
        self
    }

    /// Sets the maximum retry attempts.
    #[must_use]
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.config.max_retries = retries;
        self
    }

    /// Builds the configuration.
    #[must_use]
    pub fn build(self) -> ExecuteConfig {
        self.config
    }
}

/// Slippage configuration.
///
/// Controls how much price deviation is acceptable during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageConfig {
    /// Maximum slippage in basis points (1 bp = 0.01%).
    #[serde(default = "default_max_slippage_bps")]
    pub max_slippage_bps: Decimal,

    /// Price offset in ticks for limit orders.
    #[serde(default)]
    pub price_offset_ticks: i32,

    /// Whether to use market orders when slippage limit is reached.
    #[serde(default)]
    pub use_market_on_limit: bool,
}

fn default_max_slippage_bps() -> Decimal {
    Decimal::from(50) // 50 bps = 0.5%
}

impl Default for SlippageConfig {
    fn default() -> Self {
        Self {
            max_slippage_bps: default_max_slippage_bps(),
            price_offset_ticks: 0,
            use_market_on_limit: false,
        }
    }
}

impl SlippageConfig {
    /// Calculates the maximum acceptable price for a buy order.
    #[must_use]
    pub fn max_buy_price(&self, reference_price: Decimal) -> Decimal {
        let slippage_factor = Decimal::ONE + (self.max_slippage_bps / Decimal::from(10000));
        reference_price * slippage_factor
    }

    /// Calculates the minimum acceptable price for a sell order.
    #[must_use]
    pub fn min_sell_price(&self, reference_price: Decimal) -> Decimal {
        let slippage_factor = Decimal::ONE - (self.max_slippage_bps / Decimal::from(10000));
        reference_price * slippage_factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_execute_config_default() {
        let config = ExecuteConfig::default();
        assert_eq!(config.multiplier, Decimal::ONE);
        assert!(!config.aggressive);
        assert!(config.cancel_on_complete);
    }

    #[test]
    fn test_execute_config_builder() {
        let config = ExecuteConfig::builder()
            .multiplier(dec!(2))
            .aggressive(true)
            .max_order_size(dec!(100))
            .tick_size(dec!(0.01))
            .build();

        assert_eq!(config.multiplier, dec!(2));
        assert!(config.aggressive);
        assert_eq!(config.max_order_size, Some(dec!(100)));
        assert_eq!(config.tick_size, Some(dec!(0.01)));
    }

    #[test]
    fn test_round_price() {
        let config = ExecuteConfig::builder().tick_size(dec!(0.01)).build();

        // 100.123 rounds to 100.12 (banker's rounding)
        assert_eq!(config.round_price(dec!(100.123)), dec!(100.12));
        // 100.125 rounds to 100.12 (banker's rounding - round to even)
        assert_eq!(config.round_price(dec!(100.125)), dec!(100.12));
        // 100.126 rounds to 100.13
        assert_eq!(config.round_price(dec!(100.126)), dec!(100.13));
        // 100.135 rounds to 100.14 (banker's rounding - round to even)
        assert_eq!(config.round_price(dec!(100.135)), dec!(100.14));
    }

    #[test]
    fn test_round_quantity() {
        let config = ExecuteConfig::builder().step_size(dec!(0.001)).build();

        assert_eq!(config.round_quantity(dec!(1.2345)), dec!(1.234));
        assert_eq!(config.round_quantity(dec!(1.2349)), dec!(1.234));
    }

    #[test]
    fn test_clamp_quantity() {
        let config = ExecuteConfig::builder()
            .min_order_size(dec!(0.01))
            .max_order_size(dec!(100))
            .build();

        assert_eq!(config.clamp_quantity(dec!(50)), dec!(50));
        assert_eq!(config.clamp_quantity(dec!(150)), dec!(100));
        assert_eq!(config.clamp_quantity(dec!(0.001)), Decimal::ZERO);
    }

    #[test]
    fn test_slippage_config_default() {
        let config = SlippageConfig::default();
        assert_eq!(config.max_slippage_bps, dec!(50));
        assert_eq!(config.price_offset_ticks, 0);
        assert!(!config.use_market_on_limit);
    }

    #[test]
    fn test_slippage_max_buy_price() {
        let config = SlippageConfig {
            max_slippage_bps: dec!(100), // 1%
            ..Default::default()
        };

        let max_price = config.max_buy_price(dec!(100));
        assert_eq!(max_price, dec!(101));
    }

    #[test]
    fn test_slippage_min_sell_price() {
        let config = SlippageConfig {
            max_slippage_bps: dec!(100), // 1%
            ..Default::default()
        };

        let min_price = config.min_sell_price(dec!(100));
        assert_eq!(min_price, dec!(99));
    }
}
