//! `DualThrust` Strategy Implementation.
//!
//! `DualThrust` is a classic intraday CTA strategy that uses price range breakouts
//! to generate trading signals.
//!
//! # Strategy Logic
//!
//! The strategy calculates upper and lower boundaries based on historical price ranges:
//! - Range = MAX(HH - LC, HC - LL) where:
//!   - HH = Highest High over lookback period
//!   - LC = Lowest Close over lookback period
//!   - HC = Highest Close over lookback period
//!   - LL = Lowest Low over lookback period
//!
//! - Upper Boundary = Open + K1 × Range
//! - Lower Boundary = Open - K2 × Range
//!
//! # Trading Rules
//!
//! - When flat: Go long on upper breakout, go short on lower breakout
//! - When long: Hold on upper breakout, reverse to short on lower breakout
//! - When short: Reverse to long on upper breakout, hold on lower breakout

use crate::Decimal;
use crate::context::StrategyContext;
use crate::loader::StrategyBuilder;
use crate::signal::{Signal, Urgency};
use crate::r#trait::{Strategy, Subscription, Timeframe};
use crate::types::{Bar, Tick};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tracing::{debug, info, warn};
use zephyr_core::data::OrderStatus;
use zephyr_core::types::Symbol;

/// `DualThrust` strategy configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualThrustParams {
    /// Strategy instance name
    #[serde(default = "default_name")]
    pub name: String,
    /// Trading symbol (e.g., "BTC-USDT")
    pub symbol: String,
    /// Number of bars to look back for calculating range (default: 4)
    #[serde(default = "default_lookback")]
    pub lookback: usize,
    /// Upper boundary coefficient K1 (default: 0.5)
    #[serde(default = "default_k")]
    pub k1: Decimal,
    /// Lower boundary coefficient K2 (default: 0.5)
    #[serde(default = "default_k")]
    pub k2: Decimal,
    /// Position size for each trade (default: 1.0)
    #[serde(default = "default_position_size")]
    pub position_size: Decimal,
    /// Bar timeframe for range calculation (default: D1)
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
}

fn default_name() -> String {
    "dual_thrust".to_string()
}

fn default_lookback() -> usize {
    4
}

fn default_k() -> Decimal {
    Decimal::new(5, 1) // 0.5
}

fn default_position_size() -> Decimal {
    Decimal::ONE
}

fn default_timeframe() -> String {
    "D1".to_string()
}

impl Default for DualThrustParams {
    fn default() -> Self {
        Self {
            name: default_name(),
            symbol: "BTC-USDT".to_string(),
            lookback: default_lookback(),
            k1: default_k(),
            k2: default_k(),
            position_size: default_position_size(),
            timeframe: default_timeframe(),
        }
    }
}

impl DualThrustParams {
    /// Validates the configuration parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if any parameter is invalid.
    pub fn validate(&self) -> Result<()> {
        if self.lookback == 0 {
            return Err(anyhow!("lookback must be greater than 0"));
        }
        if self.k1 <= Decimal::ZERO {
            return Err(anyhow!("k1 must be positive"));
        }
        if self.k2 <= Decimal::ZERO {
            return Err(anyhow!("k2 must be positive"));
        }
        if self.position_size <= Decimal::ZERO {
            return Err(anyhow!("position_size must be positive"));
        }
        if self.symbol.is_empty() {
            return Err(anyhow!("symbol cannot be empty"));
        }
        Ok(())
    }

    /// Parses the timeframe string into a `Timeframe` enum.
    fn parse_timeframe(&self) -> Timeframe {
        match self.timeframe.to_uppercase().as_str() {
            "M1" | "1M" => Timeframe::M1,
            "M5" | "5M" => Timeframe::M5,
            "M15" | "15M" => Timeframe::M15,
            "M30" | "30M" => Timeframe::M30,
            "H1" | "1H" => Timeframe::H1,
            "H4" | "4H" => Timeframe::H4,
            "W1" | "1W" => Timeframe::W1,
            _ => Timeframe::D1, // Default to daily (including "D1" and "1D")
        }
    }
}

/// Historical bar data for range calculation.
#[derive(Debug, Clone)]
struct BarData {
    high: Decimal,
    low: Decimal,
    close: Decimal,
    open: Decimal,
}

/// Calculated boundaries for trading decisions.
#[derive(Debug, Clone)]
struct Boundaries {
    /// Upper boundary price
    upper: Decimal,
    /// Lower boundary price
    lower: Decimal,
    /// Base price (open)
    open: Decimal,
    /// Calculated range
    range: Decimal,
}

/// `DualThrust` trading strategy.
///
/// A classic intraday breakout strategy that generates signals based on
/// price range breakouts from calculated upper and lower boundaries.
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::strategies::{DualThrust, DualThrustParams};
///
/// let params = DualThrustParams {
///     name: "dual_thrust_btc".to_string(),
///     symbol: "BTC-USDT".to_string(),
///     lookback: 4,
///     k1: dec!(0.5),
///     k2: dec!(0.5),
///     position_size: dec!(1.0),
///     timeframe: "D1".to_string(),
/// };
///
/// let strategy = DualThrust::new(params)?;
/// ```
pub struct DualThrust {
    /// Strategy configuration
    params: DualThrustParams,
    /// Trading symbol
    symbol: Symbol,
    /// Historical bars for range calculation
    bars: VecDeque<BarData>,
    /// Current boundaries (recalculated on new bar)
    boundaries: Option<Boundaries>,
    /// Current position quantity
    current_position: Decimal,
    /// Whether strategy is initialized
    initialized: bool,
}

impl DualThrust {
    /// Creates a new `DualThrust` strategy with the given parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if the parameters are invalid.
    pub fn new(params: DualThrustParams) -> Result<Self> {
        params.validate()?;
        let symbol = Symbol::new(&params.symbol)?;

        Ok(Self {
            params,
            symbol,
            bars: VecDeque::new(),
            boundaries: None,
            current_position: Decimal::ZERO,
            initialized: false,
        })
    }

    /// Calculates the range from historical bars.
    ///
    /// Range = MAX(HH - LC, HC - LL) where:
    /// - HH = Highest High
    /// - LC = Lowest Close
    /// - HC = Highest Close
    /// - LL = Lowest Low
    fn calculate_range(&self) -> Option<Decimal> {
        if self.bars.len() < self.params.lookback {
            return None;
        }

        let mut hh = Decimal::MIN;
        let mut ll = Decimal::MAX;
        let mut hc = Decimal::MIN;
        let mut lc = Decimal::MAX;

        // Use the last `lookback` bars (excluding the current one if we have more)
        let start = self.bars.len().saturating_sub(self.params.lookback);
        for bar in self.bars.iter().skip(start) {
            if bar.high > hh {
                hh = bar.high;
            }
            if bar.low < ll {
                ll = bar.low;
            }
            if bar.close > hc {
                hc = bar.close;
            }
            if bar.close < lc {
                lc = bar.close;
            }
        }

        // Range = MAX(HH - LC, HC - LL)
        let range1 = hh - lc;
        let range2 = hc - ll;
        Some(range1.max(range2))
    }

    /// Calculates upper and lower boundaries based on open price and range.
    fn calculate_boundaries(&self, open: Decimal, range: Decimal) -> Boundaries {
        let upper = open + self.params.k1 * range;
        let lower = open - self.params.k2 * range;

        Boundaries {
            upper,
            lower,
            open,
            range,
        }
    }

    /// Determines the target position based on current price and boundaries.
    ///
    /// Returns `Some(target_qty)` if a position change is needed, `None` otherwise.
    fn determine_position(
        &self,
        current_price: Decimal,
        boundaries: &Boundaries,
    ) -> Option<Decimal> {
        let is_above_upper = current_price > boundaries.upper;
        let is_below_lower = current_price < boundaries.lower;

        let is_flat = self.current_position == Decimal::ZERO;
        let is_long = self.current_position > Decimal::ZERO;
        let is_short = self.current_position < Decimal::ZERO;

        match (is_flat, is_long, is_short, is_above_upper, is_below_lower) {
            // Flat position
            (true, _, _, true, _) => Some(self.params.position_size), // Go long
            (true, _, _, _, true) => Some(-self.params.position_size), // Go short

            // Long position - hold or reverse
            (_, true, _, true, _) => None, // Hold long
            (_, true, _, _, true) => Some(-self.params.position_size), // Reverse to short

            // Short position - reverse or hold
            (_, _, true, true, _) => Some(self.params.position_size), // Reverse to long

            // No signal (price in range or holding short)
            _ => None,
        }
    }

    /// Processes a new bar and updates boundaries.
    fn process_bar(&mut self, bar: &Bar, ctx: &mut StrategyContext) {
        // Skip if not our symbol
        if bar.symbol != self.symbol {
            return;
        }

        let bar_data = BarData {
            high: bar.high.as_decimal(),
            low: bar.low.as_decimal(),
            close: bar.close.as_decimal(),
            open: bar.open.as_decimal(),
        };

        // Add bar to history
        self.bars.push_back(bar_data.clone());

        // Keep only necessary history
        while self.bars.len() > self.params.lookback + 1 {
            self.bars.pop_front();
        }

        // Calculate new boundaries if we have enough data
        if let Some(range) = self.calculate_range() {
            let boundaries = self.calculate_boundaries(bar_data.open, range);

            info!(
                strategy = %self.params.name,
                symbol = %self.symbol,
                open = %boundaries.open,
                upper = %boundaries.upper,
                lower = %boundaries.lower,
                range = %boundaries.range,
                "DualThrust boundaries updated"
            );

            self.boundaries = Some(boundaries);
        } else {
            debug!(
                strategy = %self.params.name,
                bars_available = self.bars.len(),
                bars_needed = self.params.lookback,
                "Insufficient bars for range calculation"
            );
        }

        // Check for signal on bar close
        self.check_signal(bar_data.close, ctx);
    }

    /// Checks for trading signal and emits if needed.
    fn check_signal(&mut self, current_price: Decimal, ctx: &mut StrategyContext) {
        let boundaries = match &self.boundaries {
            Some(b) => b.clone(),
            None => return,
        };

        if let Some(target) = self.determine_position(current_price, &boundaries) {
            let urgency = if (target - self.current_position).abs() > self.params.position_size {
                Urgency::High // Reversal needs urgency
            } else {
                Urgency::Medium
            };

            let signal = Signal::target_position(self.symbol.clone(), target, urgency);

            info!(
                strategy = %self.params.name,
                symbol = %self.symbol,
                price = %current_price,
                upper = %boundaries.upper,
                lower = %boundaries.lower,
                from_position = %self.current_position,
                to_position = %target,
                "DualThrust signal triggered"
            );

            match ctx.emit_signal(signal) {
                Ok(signal_id) => {
                    debug!(
                        strategy = %self.params.name,
                        signal_id = %signal_id,
                        "Signal emitted successfully"
                    );
                    // Update our tracked position (actual position update comes via on_order_status)
                    self.current_position = target;
                }
                Err(e) => {
                    warn!(
                        strategy = %self.params.name,
                        error = %e,
                        "Failed to emit signal"
                    );
                }
            }
        }
    }
}

impl Strategy for DualThrust {
    fn name(&self) -> &str {
        &self.params.name
    }

    fn required_subscriptions(&self) -> Vec<Subscription> {
        let timeframe = self.params.parse_timeframe();
        vec![
            // Subscribe to bars for range calculation
            Subscription::bar(self.symbol.clone(), timeframe),
            // Subscribe to ticks for real-time signal generation
            Subscription::tick(self.symbol.clone()),
        ]
    }

    fn on_init(&mut self, ctx: &StrategyContext) -> Result<()> {
        ctx.log_info(&format!(
            "DualThrust strategy initialized: symbol={}, lookback={}, k1={}, k2={}, size={}",
            self.symbol,
            self.params.lookback,
            self.params.k1,
            self.params.k2,
            self.params.position_size
        ));

        self.initialized = true;
        Ok(())
    }

    fn on_tick(&mut self, tick: &Tick, ctx: &mut StrategyContext) {
        // Skip if not our symbol
        if tick.symbol != self.symbol {
            return;
        }

        // Skip if not initialized or no boundaries yet
        if !self.initialized || self.boundaries.is_none() {
            return;
        }

        let current_price = tick.price.as_decimal();
        self.check_signal(current_price, ctx);
    }

    fn on_bar(&mut self, bar: &Bar, ctx: &mut StrategyContext) {
        if !self.initialized {
            return;
        }

        self.process_bar(bar, ctx);
    }

    fn on_order_status(&mut self, status: &OrderStatus, _ctx: &mut StrategyContext) {
        // Update position tracking based on order fills
        // This is a simplified implementation - in production you'd track
        // the actual filled quantity from the order status
        debug!(
            strategy = %self.params.name,
            status = ?status,
            "Order status update received"
        );
    }

    fn on_stop(&mut self, ctx: &mut StrategyContext) -> Result<()> {
        ctx.log_info(&format!("DualThrust strategy {} stopped", self.params.name));
        self.initialized = false;
        Ok(())
    }
}

/// Builder for creating `DualThrust` strategies.
///
/// This builder is automatically registered with the strategy registry
/// using the `inventory` crate.
pub struct DualThrustBuilder;

impl crate::loader::ConstDefault for DualThrustBuilder {
    const DEFAULT: Self = Self;
}

impl Default for DualThrustBuilder {
    fn default() -> Self {
        Self
    }
}

impl StrategyBuilder for DualThrustBuilder {
    fn name(&self) -> &'static str {
        "DualThrust"
    }

    fn build(&self, config: serde_json::Value) -> Result<Box<dyn Strategy>> {
        let params: DualThrustParams = serde_json::from_value(config)
            .map_err(|e| anyhow!("Failed to parse DualThrust config: {e}"))?;

        let strategy = DualThrust::new(params)?;
        Ok(Box::new(strategy))
    }
}

// Auto-register the DualThrust builder with the strategy registry
crate::register_strategy!(DualThrustBuilder);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r#trait::DataType;
    use rust_decimal_macros::dec;

    #[test]
    fn test_params_default() {
        let params = DualThrustParams::default();
        assert_eq!(params.lookback, 4);
        assert_eq!(params.k1, dec!(0.5));
        assert_eq!(params.k2, dec!(0.5));
        assert_eq!(params.position_size, dec!(1.0));
    }

    #[test]
    fn test_params_validation() {
        let valid = DualThrustParams::default();
        assert!(valid.validate().is_ok());

        let invalid_lookback = DualThrustParams {
            lookback: 0,
            ..Default::default()
        };
        assert!(invalid_lookback.validate().is_err());

        let invalid_k1 = DualThrustParams {
            k1: dec!(-0.5),
            ..Default::default()
        };
        assert!(invalid_k1.validate().is_err());

        let invalid_symbol = DualThrustParams {
            symbol: String::new(),
            ..Default::default()
        };
        assert!(invalid_symbol.validate().is_err());
    }

    #[test]
    fn test_strategy_creation() {
        let params = DualThrustParams {
            name: "test_dual_thrust".to_string(),
            symbol: "BTC-USDT".to_string(),
            lookback: 4,
            k1: dec!(0.5),
            k2: dec!(0.5),
            position_size: dec!(1.0),
            timeframe: "D1".to_string(),
        };

        let strategy = DualThrust::new(params).unwrap();
        assert_eq!(strategy.name(), "test_dual_thrust");
    }

    #[test]
    fn test_required_subscriptions() {
        let params = DualThrustParams {
            symbol: "ETH-USDT".to_string(),
            timeframe: "H1".to_string(),
            ..Default::default()
        };

        let strategy = DualThrust::new(params).unwrap();
        let subs = strategy.required_subscriptions();

        assert_eq!(subs.len(), 2);
        assert!(
            subs.iter()
                .any(|s| s.data_type == DataType::Bar(Timeframe::H1))
        );
        assert!(subs.iter().any(|s| s.data_type == DataType::Tick));
    }

    #[test]
    fn test_range_calculation() {
        let params = DualThrustParams {
            lookback: 3,
            ..Default::default()
        };

        let mut strategy = DualThrust::new(params).unwrap();

        // Add test bars
        strategy.bars.push_back(BarData {
            high: dec!(100),
            low: dec!(90),
            close: dec!(95),
            open: dec!(92),
        });
        strategy.bars.push_back(BarData {
            high: dec!(105),
            low: dec!(88),
            close: dec!(102),
            open: dec!(95),
        });
        strategy.bars.push_back(BarData {
            high: dec!(110),
            low: dec!(85),
            close: dec!(98),
            open: dec!(102),
        });

        let range = strategy.calculate_range().unwrap();
        // HH = 110, LL = 85, HC = 102, LC = 95
        // Range = MAX(110-95, 102-85) = MAX(15, 17) = 17
        assert_eq!(range, dec!(17));
    }

    #[test]
    fn test_boundary_calculation() {
        let params = DualThrustParams {
            k1: dec!(0.5),
            k2: dec!(0.4),
            ..Default::default()
        };

        let strategy = DualThrust::new(params).unwrap();
        let boundaries = strategy.calculate_boundaries(dec!(100), dec!(20));

        // Upper = 100 + 0.5 * 20 = 110
        assert_eq!(boundaries.upper, dec!(110));
        // Lower = 100 - 0.4 * 20 = 92
        assert_eq!(boundaries.lower, dec!(92));
    }

    #[test]
    fn test_position_determination_flat() {
        let params = DualThrustParams {
            position_size: dec!(1.0),
            ..Default::default()
        };

        let strategy = DualThrust::new(params).unwrap();
        let boundaries = Boundaries {
            upper: dec!(100),
            lower: dec!(80),
            open: dec!(90),
            range: dec!(20),
        };

        // Flat + above upper = go long
        let pos = strategy.determine_position(dec!(101), &boundaries);
        assert_eq!(pos, Some(dec!(1.0)));

        // Flat + below lower = go short
        let pos = strategy.determine_position(dec!(79), &boundaries);
        assert_eq!(pos, Some(dec!(-1.0)));

        // Flat + in range = no signal
        let pos = strategy.determine_position(dec!(90), &boundaries);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_position_determination_long() {
        let params = DualThrustParams {
            position_size: dec!(1.0),
            ..Default::default()
        };

        let mut strategy = DualThrust::new(params).unwrap();
        strategy.current_position = dec!(1.0); // Already long

        let boundaries = Boundaries {
            upper: dec!(100),
            lower: dec!(80),
            open: dec!(90),
            range: dec!(20),
        };

        // Long + above upper = hold (no change)
        let pos = strategy.determine_position(dec!(101), &boundaries);
        assert_eq!(pos, None);

        // Long + below lower = reverse to short
        let pos = strategy.determine_position(dec!(79), &boundaries);
        assert_eq!(pos, Some(dec!(-1.0)));
    }

    #[test]
    fn test_position_determination_short() {
        let params = DualThrustParams {
            position_size: dec!(1.0),
            ..Default::default()
        };

        let mut strategy = DualThrust::new(params).unwrap();
        strategy.current_position = dec!(-1.0); // Already short

        let boundaries = Boundaries {
            upper: dec!(100),
            lower: dec!(80),
            open: dec!(90),
            range: dec!(20),
        };

        // Short + above upper = reverse to long
        let pos = strategy.determine_position(dec!(101), &boundaries);
        assert_eq!(pos, Some(dec!(1.0)));

        // Short + below lower = hold (no change)
        let pos = strategy.determine_position(dec!(79), &boundaries);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_builder_name() {
        let builder = DualThrustBuilder;
        assert_eq!(builder.name(), "DualThrust");
    }

    #[test]
    fn test_builder_build() {
        let builder = DualThrustBuilder;
        let config = serde_json::json!({
            "name": "test_strategy",
            "symbol": "BTC-USDT",
            "lookback": 5,
            "k1": "0.6",
            "k2": "0.4",
            "position_size": "2.0"
        });

        let strategy = builder.build(config).unwrap();
        assert_eq!(strategy.name(), "test_strategy");
    }

    #[test]
    fn test_builder_build_invalid_config() {
        let builder = DualThrustBuilder;
        let config = serde_json::json!({
            "symbol": "BTC-USDT",
            "lookback": 0  // Invalid: must be > 0
        });

        let result = builder.build(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_timeframe_parsing() {
        let params = DualThrustParams {
            timeframe: "H4".to_string(),
            ..Default::default()
        };
        assert_eq!(params.parse_timeframe(), Timeframe::H4);

        let params = DualThrustParams {
            timeframe: "1h".to_string(),
            ..Default::default()
        };
        assert_eq!(params.parse_timeframe(), Timeframe::H1);

        let params = DualThrustParams {
            timeframe: "invalid".to_string(),
            ..Default::default()
        };
        assert_eq!(params.parse_timeframe(), Timeframe::D1); // Default
    }
}

#[test]
fn test_strategy_registered() {
    use crate::loader::{is_rust_strategy_registered, list_rust_strategies, load_rust_strategy};

    // Verify DualThrust is in the list
    let strategies = list_rust_strategies();
    assert!(
        strategies.contains(&"DualThrust"),
        "DualThrust should be registered"
    );

    // Verify we can check registration
    assert!(is_rust_strategy_registered("DualThrust"));

    // Verify we can load it
    let config = serde_json::json!({
        "name": "test_strategy",
        "symbol": "BTC-USDT"
    });
    let strategy = load_rust_strategy("DualThrust", config);
    assert!(
        strategy.is_ok(),
        "Should be able to load DualThrust strategy"
    );
    assert_eq!(strategy.unwrap().name(), "test_strategy");
}
