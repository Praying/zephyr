//! DualThrust Strategy Implementation.
//!
//! DualThrust is a classic intraday CTA strategy that uses price range breakouts
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
//! - Upper Boundary = Today's Open + K1 × Range
//! - Lower Boundary = Today's Open - K2 × Range
//!
//! # Trading Rules
//!
//! - When flat: Go long on upper breakout, go short on lower breakout
//! - When long: Hold on upper breakout, reverse to short on lower breakout
//! - When short: Reverse to long on upper breakout, hold on lower breakout
//!
//! # Example
//!
//! ```ignore
//! use zephyr_engine::strategies::{DualThrustStrategy, DualThrustConfig};
//! use zephyr_core::types::Symbol;
//! use rust_decimal_macros::dec;
//!
//! let config = DualThrustConfig {
//!     lookback_days: 4,
//!     k1: dec!(0.5),  // Upper boundary coefficient
//!     k2: dec!(0.5),  // Lower boundary coefficient
//!     position_size: dec!(1.0),
//! };
//!
//! let strategy = DualThrustStrategy::new(
//!     "dual_thrust_btc",
//!     Symbol::new("BTC-USDT").unwrap(),
//!     config,
//! );
//! ```

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use zephyr_core::data::{KlineData, KlinePeriod, TickData};
use zephyr_core::error::StrategyError;
use zephyr_core::traits::{CtaStrategy, CtaStrategyContext, LogLevel};
use zephyr_core::types::{Price, Quantity, Symbol};

/// DualThrust strategy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualThrustConfig {
    /// Number of days to look back for calculating range (default: 4)
    pub lookback_days: usize,
    /// Upper boundary coefficient K1 (default: 0.5)
    pub k1: Decimal,
    /// Lower boundary coefficient K2 (default: 0.5)
    pub k2: Decimal,
    /// Position size for each trade (default: 1.0)
    pub position_size: Decimal,
}

impl Default for DualThrustConfig {
    fn default() -> Self {
        Self {
            lookback_days: 4,
            k1: Decimal::new(5, 1), // 0.5
            k2: Decimal::new(5, 1), // 0.5
            position_size: Decimal::ONE,
        }
    }
}

impl DualThrustConfig {
    /// Creates a new configuration with custom coefficients.
    #[must_use]
    pub fn new(lookback_days: usize, k1: Decimal, k2: Decimal, position_size: Decimal) -> Self {
        Self {
            lookback_days,
            k1,
            k2,
            position_size,
        }
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration is invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.lookback_days == 0 {
            return Err("lookback_days must be greater than 0");
        }
        if self.k1 <= Decimal::ZERO {
            return Err("k1 must be positive");
        }
        if self.k2 <= Decimal::ZERO {
            return Err("k2 must be positive");
        }
        if self.position_size <= Decimal::ZERO {
            return Err("position_size must be positive");
        }
        Ok(())
    }
}

/// DualThrust range calculation result.
#[derive(Debug, Clone)]
struct RangeData {
    /// Highest high over lookback period
    hh: Decimal,
    /// Lowest low over lookback period
    ll: Decimal,
    /// Highest close over lookback period
    hc: Decimal,
    /// Lowest close over lookback period
    lc: Decimal,
    /// Calculated range: MAX(HH-LC, HC-LL)
    range: Decimal,
}

/// DualThrust boundary calculation result.
#[derive(Debug, Clone)]
struct Boundaries {
    /// Upper boundary price
    upper: Decimal,
    /// Lower boundary price
    lower: Decimal,
    /// Today's open price (base price)
    open: Decimal,
    /// The range used for calculation
    range: Decimal,
}

/// DualThrust CTA Strategy.
///
/// A classic intraday breakout strategy that generates signals based on
/// price range breakouts from calculated upper and lower boundaries.
pub struct DualThrustStrategy {
    /// Strategy name
    name: String,
    /// Trading symbol
    symbol: Symbol,
    /// Strategy configuration
    config: DualThrustConfig,
    /// Today's open price (reset daily)
    today_open: Option<Price>,
    /// Current boundaries (recalculated on new day)
    boundaries: Option<Boundaries>,
    /// Last bar timestamp (for detecting new day)
    last_bar_day: Option<i64>,
    /// Whether strategy is initialized
    initialized: bool,
}

impl DualThrustStrategy {
    /// Creates a new DualThrust strategy.
    ///
    /// # Arguments
    ///
    /// * `name` - Strategy name for identification
    /// * `symbol` - Trading symbol
    /// * `config` - Strategy configuration
    #[must_use]
    pub fn new(name: impl Into<String>, symbol: Symbol, config: DualThrustConfig) -> Self {
        Self {
            name: name.into(),
            symbol,
            config,
            today_open: None,
            boundaries: None,
            last_bar_day: None,
            initialized: false,
        }
    }

    /// Creates a new DualThrust strategy with default configuration.
    #[must_use]
    pub fn with_defaults(name: impl Into<String>, symbol: Symbol) -> Self {
        Self::new(name, symbol, DualThrustConfig::default())
    }

    /// Calculates range data from historical bars.
    fn calculate_range(&self, bars: &[KlineData]) -> Option<RangeData> {
        if bars.len() < self.config.lookback_days {
            return None;
        }

        // Get the last N days of data (excluding today)
        let lookback_bars = &bars[bars.len().saturating_sub(self.config.lookback_days + 1)
            ..bars.len().saturating_sub(1)];

        if lookback_bars.is_empty() {
            return None;
        }

        let mut hh = Decimal::MIN;
        let mut ll = Decimal::MAX;
        let mut hc = Decimal::MIN;
        let mut lc = Decimal::MAX;

        for bar in lookback_bars {
            let high = bar.high.as_decimal();
            let low = bar.low.as_decimal();
            let close = bar.close.as_decimal();

            if high > hh {
                hh = high;
            }
            if low < ll {
                ll = low;
            }
            if close > hc {
                hc = close;
            }
            if close < lc {
                lc = close;
            }
        }

        // Range = MAX(HH - LC, HC - LL)
        let range1 = hh - lc;
        let range2 = hc - ll;
        let range = range1.max(range2);

        Some(RangeData {
            hh,
            ll,
            hc,
            lc,
            range,
        })
    }

    /// Calculates upper and lower boundaries.
    fn calculate_boundaries(&self, open: Decimal, range_data: &RangeData) -> Boundaries {
        let upper = open + self.config.k1 * range_data.range;
        let lower = open - self.config.k2 * range_data.range;

        Boundaries {
            upper,
            lower,
            open,
            range: range_data.range,
        }
    }

    /// Checks if a new trading day has started.
    fn is_new_day(&self, bar: &KlineData) -> bool {
        let bar_day = bar.timestamp.as_millis() / (24 * 60 * 60 * 1000);
        match self.last_bar_day {
            Some(last_day) => bar_day > last_day,
            None => true,
        }
    }

    /// Gets the day number from a bar.
    fn get_day(&self, bar: &KlineData) -> i64 {
        bar.timestamp.as_millis() / (24 * 60 * 60 * 1000)
    }

    /// Determines the target position based on current price and boundaries.
    fn determine_position(
        &self,
        current_price: Decimal,
        current_position: Decimal,
        boundaries: &Boundaries,
    ) -> Option<Decimal> {
        let is_above_upper = current_price > boundaries.upper;
        let is_below_lower = current_price < boundaries.lower;

        // Position states: 0 = flat, positive = long, negative = short
        let is_flat = current_position == Decimal::ZERO;
        let is_long = current_position > Decimal::ZERO;
        let is_short = current_position < Decimal::ZERO;

        match (is_flat, is_long, is_short, is_above_upper, is_below_lower) {
            // Flat position
            (true, _, _, true, _) => Some(self.config.position_size), // Go long
            (true, _, _, _, true) => Some(-self.config.position_size), // Go short

            // Long position
            (_, true, _, true, _) => None, // Hold long
            (_, true, _, _, true) => Some(-self.config.position_size), // Reverse to short

            // Short position
            (_, _, true, true, _) => Some(self.config.position_size), // Reverse to long
            (_, _, true, _, true) => None,                            // Hold short

            // No signal
            _ => None,
        }
    }
}

#[async_trait]
impl CtaStrategy for DualThrustStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn on_init(&mut self, ctx: &dyn CtaStrategyContext) -> Result<(), StrategyError> {
        // Validate configuration
        self.config
            .validate()
            .map_err(|e| StrategyError::InitializationFailed {
                strategy: self.name.clone(),
                reason: e.to_string(),
            })?;

        ctx.log(
            LogLevel::Info,
            &format!(
                "DualThrust strategy initialized: symbol={}, lookback={}, k1={}, k2={}, size={}",
                self.symbol,
                self.config.lookback_days,
                self.config.k1,
                self.config.k2,
                self.config.position_size
            ),
        );

        self.initialized = true;
        Ok(())
    }

    async fn on_tick(
        &mut self,
        ctx: &dyn CtaStrategyContext,
        tick: &TickData,
    ) -> Result<(), StrategyError> {
        // Skip if not our symbol
        if tick.symbol != self.symbol {
            return Ok(());
        }

        // Skip if boundaries not calculated yet
        let boundaries = match &self.boundaries {
            Some(b) => b.clone(),
            None => {
                debug!(
                    strategy = %self.name,
                    "Boundaries not calculated yet, skipping tick"
                );
                return Ok(());
            }
        };

        let current_price = tick.price.as_decimal();
        let current_position = ctx.get_position(&self.symbol).as_decimal();

        // Check for position change
        if let Some(target) = self.determine_position(current_price, current_position, &boundaries)
        {
            let tag = if target > Decimal::ZERO {
                "long_breakout"
            } else {
                "short_breakout"
            };

            info!(
                strategy = %self.name,
                symbol = %self.symbol,
                price = %current_price,
                upper = %boundaries.upper,
                lower = %boundaries.lower,
                from_position = %current_position,
                to_position = %target,
                tag = %tag,
                "DualThrust signal triggered"
            );

            let qty = Quantity::new(target).map_err(|e| StrategyError::CallbackError {
                strategy: self.name.clone(),
                reason: format!("Invalid quantity: {e}"),
            })?;

            ctx.set_position(&self.symbol, qty, tag).await?;
        }

        Ok(())
    }

    async fn on_bar(
        &mut self,
        ctx: &dyn CtaStrategyContext,
        bar: &KlineData,
    ) -> Result<(), StrategyError> {
        // Skip if not our symbol
        if bar.symbol != self.symbol {
            return Ok(());
        }

        // Check if new day started
        if self.is_new_day(bar) {
            // Get historical daily bars for range calculation
            let bars = ctx.get_bars(
                &self.symbol,
                KlinePeriod::Day1,
                self.config.lookback_days + 2,
            );

            if bars.len() > self.config.lookback_days {
                // Calculate range from historical data
                if let Some(range_data) = self.calculate_range(bars) {
                    // Use today's open as base price
                    let today_open = bar.open.as_decimal();
                    self.today_open = Some(bar.open);

                    // Calculate boundaries
                    let boundaries = self.calculate_boundaries(today_open, &range_data);

                    info!(
                        strategy = %self.name,
                        symbol = %self.symbol,
                        open = %boundaries.open,
                        upper = %boundaries.upper,
                        lower = %boundaries.lower,
                        range = %boundaries.range,
                        hh = %range_data.hh,
                        ll = %range_data.ll,
                        hc = %range_data.hc,
                        lc = %range_data.lc,
                        "New day boundaries calculated"
                    );

                    self.boundaries = Some(boundaries);
                } else {
                    warn!(
                        strategy = %self.name,
                        "Insufficient data for range calculation"
                    );
                }
            } else {
                warn!(
                    strategy = %self.name,
                    bars_available = bars.len(),
                    bars_needed = self.config.lookback_days + 1,
                    "Insufficient historical bars"
                );
            }

            self.last_bar_day = Some(self.get_day(bar));
        }

        // Also check for breakout on bar close
        let boundaries = match &self.boundaries {
            Some(b) => b.clone(),
            None => return Ok(()),
        };

        let current_price = bar.close.as_decimal();
        let current_position = ctx.get_position(&self.symbol).as_decimal();

        if let Some(target) = self.determine_position(current_price, current_position, &boundaries)
        {
            let tag = if target > Decimal::ZERO {
                "long_breakout_bar"
            } else {
                "short_breakout_bar"
            };

            info!(
                strategy = %self.name,
                symbol = %self.symbol,
                price = %current_price,
                upper = %boundaries.upper,
                lower = %boundaries.lower,
                from_position = %current_position,
                to_position = %target,
                tag = %tag,
                "DualThrust bar signal triggered"
            );

            let qty = Quantity::new(target).map_err(|e| StrategyError::CallbackError {
                strategy: self.name.clone(),
                reason: format!("Invalid quantity: {e}"),
            })?;

            ctx.set_position(&self.symbol, qty, tag).await?;
        }

        Ok(())
    }

    async fn on_stop(&mut self, ctx: &dyn CtaStrategyContext) {
        ctx.log(
            LogLevel::Info,
            &format!("DualThrust strategy {} stopped", self.name),
        );
        self.initialized = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_config_default() {
        let config = DualThrustConfig::default();
        assert_eq!(config.lookback_days, 4);
        assert_eq!(config.k1, dec!(0.5));
        assert_eq!(config.k2, dec!(0.5));
        assert_eq!(config.position_size, dec!(1.0));
    }

    #[test]
    fn test_config_validation() {
        let valid = DualThrustConfig::default();
        assert!(valid.validate().is_ok());

        let invalid_lookback = DualThrustConfig {
            lookback_days: 0,
            ..Default::default()
        };
        assert!(invalid_lookback.validate().is_err());

        let invalid_k1 = DualThrustConfig {
            k1: dec!(-0.5),
            ..Default::default()
        };
        assert!(invalid_k1.validate().is_err());
    }

    #[test]
    fn test_range_calculation() {
        let strategy = DualThrustStrategy::with_defaults("test", Symbol::new("BTC-USDT").unwrap());

        // Create mock bars
        let bars = create_test_bars();
        let range_data = strategy.calculate_range(&bars);

        assert!(range_data.is_some());
        let rd = range_data.unwrap();

        // Verify range calculation: MAX(HH-LC, HC-LL)
        let expected_range = (rd.hh - rd.lc).max(rd.hc - rd.ll);
        assert_eq!(rd.range, expected_range);
    }

    #[test]
    fn test_boundary_calculation() {
        let config = DualThrustConfig {
            lookback_days: 4,
            k1: dec!(0.5),
            k2: dec!(0.4),
            position_size: dec!(1.0),
        };
        let strategy = DualThrustStrategy::new("test", Symbol::new("BTC-USDT").unwrap(), config);

        let range_data = RangeData {
            hh: dec!(100),
            ll: dec!(80),
            hc: dec!(95),
            lc: dec!(85),
            range: dec!(15), // MAX(100-85, 95-80) = MAX(15, 15) = 15
        };

        let open = dec!(90);
        let boundaries = strategy.calculate_boundaries(open, &range_data);

        // Upper = 90 + 0.5 * 15 = 97.5
        assert_eq!(boundaries.upper, dec!(97.5));
        // Lower = 90 - 0.4 * 15 = 84
        assert_eq!(boundaries.lower, dec!(84));
    }

    #[test]
    fn test_position_determination_flat() {
        let strategy = DualThrustStrategy::with_defaults("test", Symbol::new("BTC-USDT").unwrap());

        let boundaries = Boundaries {
            upper: dec!(100),
            lower: dec!(80),
            open: dec!(90),
            range: dec!(20),
        };

        // Flat + above upper = go long
        let pos = strategy.determine_position(dec!(101), dec!(0), &boundaries);
        assert_eq!(pos, Some(dec!(1.0)));

        // Flat + below lower = go short
        let pos = strategy.determine_position(dec!(79), dec!(0), &boundaries);
        assert_eq!(pos, Some(dec!(-1.0)));

        // Flat + in range = no signal
        let pos = strategy.determine_position(dec!(90), dec!(0), &boundaries);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_position_determination_long() {
        let strategy = DualThrustStrategy::with_defaults("test", Symbol::new("BTC-USDT").unwrap());

        let boundaries = Boundaries {
            upper: dec!(100),
            lower: dec!(80),
            open: dec!(90),
            range: dec!(20),
        };

        // Long + above upper = hold (no change)
        let pos = strategy.determine_position(dec!(101), dec!(1), &boundaries);
        assert_eq!(pos, None);

        // Long + below lower = reverse to short
        let pos = strategy.determine_position(dec!(79), dec!(1), &boundaries);
        assert_eq!(pos, Some(dec!(-1.0)));

        // Long + in range = no signal
        let pos = strategy.determine_position(dec!(90), dec!(1), &boundaries);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_position_determination_short() {
        let strategy = DualThrustStrategy::with_defaults("test", Symbol::new("BTC-USDT").unwrap());

        let boundaries = Boundaries {
            upper: dec!(100),
            lower: dec!(80),
            open: dec!(90),
            range: dec!(20),
        };

        // Short + above upper = reverse to long
        let pos = strategy.determine_position(dec!(101), dec!(-1), &boundaries);
        assert_eq!(pos, Some(dec!(1.0)));

        // Short + below lower = hold (no change)
        let pos = strategy.determine_position(dec!(79), dec!(-1), &boundaries);
        assert_eq!(pos, None);

        // Short + in range = no signal
        let pos = strategy.determine_position(dec!(90), dec!(-1), &boundaries);
        assert_eq!(pos, None);
    }

    fn create_test_bars() -> Vec<KlineData> {
        use zephyr_core::types::{Amount, Timestamp};

        let symbol = Symbol::new("BTC-USDT").unwrap();
        let base_ts = 1_704_067_200_000i64; // 2024-01-01 00:00:00 UTC
        let day_ms = 24 * 60 * 60 * 1000i64;

        (0..6)
            .map(|i| {
                let ts = base_ts + i * day_ms;
                let base_price = dec!(40000) + Decimal::from(i) * dec!(500);

                KlineData {
                    symbol: symbol.clone(),
                    timestamp: Timestamp::new_unchecked(ts),
                    period: KlinePeriod::Day1,
                    open: Price::new(base_price).unwrap(),
                    high: Price::new(base_price + dec!(1000)).unwrap(),
                    low: Price::new(base_price - dec!(500)).unwrap(),
                    close: Price::new(base_price + dec!(200)).unwrap(),
                    volume: Quantity::new(dec!(100)).unwrap(),
                    turnover: Amount::new(dec!(4000000)).unwrap(),
                }
            })
            .collect()
    }
}
