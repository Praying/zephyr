//! Core strategy trait definition.
//!
//! This module defines the `Strategy` trait that all trading strategies must implement,
//! along with supporting types for subscriptions and data types.

use crate::context::StrategyContext;
use crate::types::{Bar, Tick};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use zephyr_core::data::OrderStatus;
use zephyr_core::types::Symbol;

/// Timeframe for bar/candlestick data.
///
/// Represents the aggregation period for OHLCV data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    /// 1 minute bars
    M1,
    /// 5 minute bars
    M5,
    /// 15 minute bars
    M15,
    /// 30 minute bars
    M30,
    /// 1 hour bars
    H1,
    /// 4 hour bars
    H4,
    /// 1 day bars
    D1,
    /// 1 week bars
    W1,
}

impl Timeframe {
    /// Returns the duration of this timeframe.
    #[must_use]
    pub const fn duration(&self) -> Duration {
        match self {
            Self::M1 => Duration::from_secs(60),
            Self::M5 => Duration::from_secs(5 * 60),
            Self::M15 => Duration::from_secs(15 * 60),
            Self::M30 => Duration::from_secs(30 * 60),
            Self::H1 => Duration::from_secs(60 * 60),
            Self::H4 => Duration::from_secs(4 * 60 * 60),
            Self::D1 => Duration::from_secs(24 * 60 * 60),
            Self::W1 => Duration::from_secs(7 * 24 * 60 * 60),
        }
    }

    /// Returns the duration in milliseconds.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn millis(&self) -> i64 {
        self.duration().as_millis() as i64
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::M1 => write!(f, "1m"),
            Self::M5 => write!(f, "5m"),
            Self::M15 => write!(f, "15m"),
            Self::M30 => write!(f, "30m"),
            Self::H1 => write!(f, "1h"),
            Self::H4 => write!(f, "4h"),
            Self::D1 => write!(f, "1d"),
            Self::W1 => write!(f, "1w"),
        }
    }
}

/// Type of market data a strategy can subscribe to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// Real-time tick/trade data
    Tick,
    /// Aggregated bar/candlestick data with specified timeframe
    Bar(Timeframe),
    /// Order book depth data
    OrderBook,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tick => write!(f, "tick"),
            Self::Bar(tf) => write!(f, "bar_{tf}"),
            Self::OrderBook => write!(f, "orderbook"),
        }
    }
}

/// Market data subscription specification.
///
/// Defines what data a strategy wants to receive.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Subscription {
    /// Trading pair symbol
    pub symbol: Symbol,
    /// Type of data to receive
    pub data_type: DataType,
}

impl Subscription {
    /// Creates a new subscription.
    #[must_use]
    pub fn new(symbol: Symbol, data_type: DataType) -> Self {
        Self { symbol, data_type }
    }

    /// Creates a tick subscription for a symbol.
    #[must_use]
    pub fn tick(symbol: Symbol) -> Self {
        Self::new(symbol, DataType::Tick)
    }

    /// Creates a bar subscription for a symbol with specified timeframe.
    #[must_use]
    pub fn bar(symbol: Symbol, timeframe: Timeframe) -> Self {
        Self::new(symbol, DataType::Bar(timeframe))
    }

    /// Creates an order book subscription for a symbol.
    #[must_use]
    pub fn orderbook(symbol: Symbol) -> Self {
        Self::new(symbol, DataType::OrderBook)
    }
}

impl fmt::Display for Subscription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.symbol, self.data_type)
    }
}

/// Core strategy trait.
///
/// All trading strategies must implement this trait. The trait is designed to be:
/// - **Synchronous**: Avoids async overhead and maintains Python compatibility
/// - **Thread-safe**: Requires `Send + Sync` bounds for concurrent execution
/// - **Minimal**: Only essential methods, keeping implementation simple
///
/// # Lifecycle
///
/// 1. `on_init()` - Called once when strategy is loaded
/// 2. `on_tick()` / `on_bar()` - Called for each market data update
/// 3. `on_order_status()` - Called when order status changes (optional)
/// 4. `on_stop()` - Called before strategy is unloaded
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::prelude::*;
///
/// struct SimpleStrategy {
///     name: String,
///     position: Decimal,
/// }
///
/// impl Strategy for SimpleStrategy {
///     fn name(&self) -> &str {
///         &self.name
///     }
///
///     fn required_subscriptions(&self) -> Vec<Subscription> {
///         vec![Subscription::tick(Symbol::new_unchecked("BTC-USDT"))]
///     }
///
///     fn on_init(&mut self, _ctx: &StrategyContext) -> anyhow::Result<()> {
///         self.position = Decimal::ZERO;
///         Ok(())
///     }
///
///     fn on_tick(&mut self, tick: &Tick, ctx: &mut StrategyContext) {
///         // Trading logic here
///     }
///
///     fn on_bar(&mut self, _bar: &Bar, _ctx: &mut StrategyContext) {}
///
///     fn on_stop(&mut self, _ctx: &mut StrategyContext) -> anyhow::Result<()> {
///         Ok(())
///     }
/// }
/// ```
pub trait Strategy: Send + Sync {
    /// Returns the unique identifier for this strategy instance.
    ///
    /// This name is used for logging, metrics, and configuration.
    fn name(&self) -> &str;

    /// Returns the list of market data subscriptions this strategy requires.
    ///
    /// Called immediately after initialization to set up data routing.
    /// Strategies only receive data for symbols and types they subscribe to.
    fn required_subscriptions(&self) -> Vec<Subscription>;

    /// Initializes the strategy.
    ///
    /// Called once when the strategy is loaded, before any market data is received.
    /// Use this to set up initial state, load historical data, etc.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails. The strategy will not be started.
    fn on_init(&mut self, ctx: &StrategyContext) -> Result<()>;

    /// Processes a tick (real-time trade) update.
    ///
    /// Called for each tick matching the strategy's subscriptions.
    /// This is the primary entry point for high-frequency strategies.
    fn on_tick(&mut self, tick: &Tick, ctx: &mut StrategyContext);

    /// Processes a bar (candlestick/OHLCV) update.
    ///
    /// Called when a new bar closes for subscribed symbols and timeframes.
    /// This is the primary entry point for bar-based strategies.
    fn on_bar(&mut self, bar: &Bar, ctx: &mut StrategyContext);

    /// Handles order status updates.
    ///
    /// Called when an order's status changes (filled, cancelled, etc.).
    /// Override this to track order lifecycle and update strategy state.
    ///
    /// Default implementation does nothing.
    fn on_order_status(&mut self, _status: &OrderStatus, _ctx: &mut StrategyContext) {}

    /// Performs cleanup before the strategy is unloaded.
    ///
    /// Called when the strategy is stopped or the system shuts down.
    /// Use this to cancel open orders, save state, etc.
    ///
    /// This method is always called, even if errors occurred during execution.
    ///
    /// # Errors
    ///
    /// Returns an error if cleanup fails. The error is logged but doesn't
    /// prevent shutdown.
    fn on_stop(&mut self, ctx: &mut StrategyContext) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeframe_duration() {
        assert_eq!(Timeframe::M1.duration(), Duration::from_secs(60));
        assert_eq!(Timeframe::H1.duration(), Duration::from_secs(3600));
        assert_eq!(Timeframe::D1.duration(), Duration::from_secs(86400));
    }

    #[test]
    fn test_timeframe_display() {
        assert_eq!(format!("{}", Timeframe::M1), "1m");
        assert_eq!(format!("{}", Timeframe::H4), "4h");
        assert_eq!(format!("{}", Timeframe::W1), "1w");
    }

    #[test]
    fn test_data_type_display() {
        assert_eq!(format!("{}", DataType::Tick), "tick");
        assert_eq!(format!("{}", DataType::Bar(Timeframe::H1)), "bar_1h");
        assert_eq!(format!("{}", DataType::OrderBook), "orderbook");
    }

    #[test]
    fn test_subscription_creation() {
        let symbol = Symbol::new_unchecked("BTC-USDT");

        let tick_sub = Subscription::tick(symbol.clone());
        assert_eq!(tick_sub.data_type, DataType::Tick);

        let bar_sub = Subscription::bar(symbol.clone(), Timeframe::H1);
        assert_eq!(bar_sub.data_type, DataType::Bar(Timeframe::H1));

        let ob_sub = Subscription::orderbook(symbol);
        assert_eq!(ob_sub.data_type, DataType::OrderBook);
    }

    #[test]
    fn test_subscription_display() {
        let sub = Subscription::tick(Symbol::new_unchecked("ETH-USDT"));
        assert_eq!(format!("{sub}"), "ETH-USDT:tick");
    }
}
