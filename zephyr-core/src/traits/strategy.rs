//! Strategy trait definitions.
//!
//! This module provides trait definitions for trading strategies,
//! including CTA (Commodity Trading Advisor) and HFT (High-Frequency Trading) strategies.
//!
//! # Architecture
//!
//! The strategy system provides two main strategy types:
//! - [`CtaStrategy`] - For medium/low frequency trend-following strategies
//! - [`HftStrategy`] - For high-frequency market-making and arbitrage strategies
//!
//! Each strategy type has a corresponding context trait:
//! - [`CtaStrategyContext`] - Provides data access and position management for CTA strategies
//! - [`HftStrategyContext`] - Provides direct order submission for HFT strategies

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::data::{KlineData, KlinePeriod, Order, TickData};
use crate::error::StrategyError;
use crate::types::{OrderId, Price, Quantity, Symbol, Timestamp};

use super::trader::Trade;

/// Order flags for HFT strategies.
///
/// Controls order execution behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrderFlag {
    /// Normal order execution.
    #[default]
    Normal,
    /// Fill and Kill - fill what's available immediately, cancel the rest.
    Fak,
    /// Fill or Kill - fill completely or cancel entirely.
    Fok,
    /// Post-only - only add liquidity, reject if would take.
    PostOnly,
    /// Reduce-only - only reduce existing position.
    ReduceOnly,
}

impl std::fmt::Display for OrderFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::Fak => write!(f, "fak"),
            Self::Fok => write!(f, "fok"),
            Self::PostOnly => write!(f, "post_only"),
            Self::ReduceOnly => write!(f, "reduce_only"),
        }
    }
}

/// Log level for strategy logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Trace level - most verbose.
    Trace,
    /// Debug level.
    Debug,
    /// Info level.
    Info,
    /// Warning level.
    Warn,
    /// Error level.
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "trace"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// CTA strategy context trait.
///
/// Provides data access and position management for CTA strategies.
/// CTA strategies use a "set position" model where they declare their
/// target position and the engine handles order execution.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
///
/// # Example
///
/// ```ignore
/// async fn on_bar(&mut self, ctx: &dyn CtaStrategyContext, bar: &KlineData) -> Result<(), StrategyError> {
///     let current_pos = ctx.get_position(&bar.symbol);
///     let price = ctx.get_price(&bar.symbol).unwrap();
///     
///     // Simple momentum strategy
///     if bar.close > bar.open {
///         ctx.set_position(&bar.symbol, Quantity::new(dec!(1.0)).unwrap(), "long_signal").await?;
///     } else {
///         ctx.set_position(&bar.symbol, Quantity::new(dec!(0.0)).unwrap(), "exit_signal").await?;
///     }
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait CtaStrategyContext: Send + Sync {
    /// Gets the current position for a symbol.
    ///
    /// Returns the theoretical position quantity (positive for long, negative for short).
    fn get_position(&self, symbol: &Symbol) -> Quantity;

    /// Sets the target position for a symbol.
    ///
    /// The engine will calculate the required orders to reach the target position.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `qty` - Target position quantity (positive for long, negative for short, zero to close)
    /// * `tag` - A tag for identifying this position change (for logging/tracking)
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if the position change is invalid.
    async fn set_position(
        &self,
        symbol: &Symbol,
        qty: Quantity,
        tag: &str,
    ) -> Result<(), StrategyError>;

    /// Gets the current price for a symbol.
    ///
    /// Returns the last traded price, or `None` if no price is available.
    fn get_price(&self, symbol: &Symbol) -> Option<Price>;

    /// Gets historical K-line data for a symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `period` - The K-line period
    /// * `count` - Number of bars to retrieve
    ///
    /// # Returns
    ///
    /// A vector of K-line data, most recent last.
    fn get_bars(&self, symbol: &Symbol, period: KlinePeriod, count: usize) -> Vec<KlineData>;

    /// Gets recent tick data for a symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `count` - Number of ticks to retrieve
    ///
    /// # Returns
    ///
    /// A vector of tick data, most recent last.
    fn get_ticks(&self, symbol: &Symbol, count: usize) -> Vec<TickData>;

    /// Subscribes to tick data for a symbol.
    ///
    /// After subscribing, the strategy will receive `on_tick` callbacks.
    fn subscribe_ticks(&self, symbol: &Symbol);

    /// Unsubscribes from tick data for a symbol.
    fn unsubscribe_ticks(&self, symbol: &Symbol);

    /// Gets the current timestamp.
    ///
    /// In live trading, this returns the current time.
    /// In backtesting, this returns the simulated time.
    fn current_time(&self) -> Timestamp;

    /// Logs a message with the specified level.
    fn log(&self, level: LogLevel, message: &str);

    /// Gets the strategy name.
    fn strategy_name(&self) -> &str;

    /// Gets all symbols the strategy is trading.
    fn symbols(&self) -> Vec<Symbol>;
}

/// CTA strategy trait.
///
/// Defines the interface for CTA (Commodity Trading Advisor) strategies.
/// CTA strategies are event-driven and receive callbacks on market data events.
///
/// # Lifecycle
///
/// 1. `on_init` - Called once when the strategy is initialized
/// 2. `on_tick` / `on_bar` - Called on market data events
/// 3. Strategy uses context to set positions
/// 4. Engine handles order execution
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
#[async_trait]
pub trait CtaStrategy: Send + Sync {
    /// Returns the strategy name.
    fn name(&self) -> &str;

    /// Called when the strategy is initialized.
    ///
    /// Use this to set up initial state, subscribe to data, etc.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if initialization fails.
    async fn on_init(&mut self, ctx: &dyn CtaStrategyContext) -> Result<(), StrategyError>;

    /// Called when a new tick is received.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    async fn on_tick(
        &mut self,
        ctx: &dyn CtaStrategyContext,
        tick: &TickData,
    ) -> Result<(), StrategyError>;

    /// Called when a new bar closes.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    async fn on_bar(
        &mut self,
        ctx: &dyn CtaStrategyContext,
        bar: &KlineData,
    ) -> Result<(), StrategyError>;

    /// Called when the strategy is stopped.
    ///
    /// Use this to clean up resources.
    async fn on_stop(&mut self, _ctx: &dyn CtaStrategyContext) {
        // Default implementation does nothing
    }
}

/// HFT strategy context trait.
///
/// Provides direct order submission for HFT strategies.
/// HFT strategies have direct control over order placement and cancellation.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
#[async_trait]
pub trait HftStrategyContext: Send + Sync {
    /// Submits a buy order.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `price` - Limit price
    /// * `qty` - Order quantity
    /// * `flag` - Order execution flag
    ///
    /// # Returns
    ///
    /// Returns the order ID on success.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if order submission fails.
    async fn buy(
        &self,
        symbol: &Symbol,
        price: Price,
        qty: Quantity,
        flag: OrderFlag,
    ) -> Result<OrderId, StrategyError>;

    /// Submits a sell order.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `price` - Limit price
    /// * `qty` - Order quantity
    /// * `flag` - Order execution flag
    ///
    /// # Returns
    ///
    /// Returns the order ID on success.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if order submission fails.
    async fn sell(
        &self,
        symbol: &Symbol,
        price: Price,
        qty: Quantity,
        flag: OrderFlag,
    ) -> Result<OrderId, StrategyError>;

    /// Cancels an order.
    ///
    /// # Returns
    ///
    /// Returns `true` if the order was successfully canceled.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if cancellation fails.
    async fn cancel(&self, order_id: &OrderId) -> Result<bool, StrategyError>;

    /// Cancels all orders for a symbol.
    ///
    /// # Returns
    ///
    /// Returns the number of orders canceled.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if cancellation fails.
    async fn cancel_all(&self, symbol: &Symbol) -> Result<usize, StrategyError>;

    /// Gets pending (unfilled) orders for a symbol.
    fn get_pending_orders(&self, symbol: &Symbol) -> Vec<&Order>;

    /// Gets the current position for a symbol.
    fn get_position(&self, symbol: &Symbol) -> Quantity;

    /// Gets the current price for a symbol.
    fn get_price(&self, symbol: &Symbol) -> Option<Price>;

    /// Gets the current timestamp.
    fn current_time(&self) -> Timestamp;

    /// Logs a message with the specified level.
    fn log(&self, level: LogLevel, message: &str);

    /// Gets the strategy name.
    fn strategy_name(&self) -> &str;
}

/// HFT strategy trait.
///
/// Defines the interface for HFT (High-Frequency Trading) strategies.
/// HFT strategies have direct control over order placement and receive
/// immediate callbacks on order and trade events.
///
/// # Lifecycle
///
/// 1. `on_init` - Called once when the strategy is initialized
/// 2. `on_tick` - Called on market data events
/// 3. `on_order` / `on_trade` - Called on order/trade events
/// 4. Strategy directly submits/cancels orders via context
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
#[async_trait]
pub trait HftStrategy: Send + Sync {
    /// Returns the strategy name.
    fn name(&self) -> &str;

    /// Called when the strategy is initialized.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if initialization fails.
    async fn on_init(&mut self, ctx: &dyn HftStrategyContext) -> Result<(), StrategyError>;

    /// Called when a new tick is received.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    async fn on_tick(
        &mut self,
        ctx: &dyn HftStrategyContext,
        tick: &TickData,
    ) -> Result<(), StrategyError>;

    /// Called when an order update is received.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    async fn on_order(
        &mut self,
        ctx: &dyn HftStrategyContext,
        order: &Order,
    ) -> Result<(), StrategyError>;

    /// Called when a trade execution is received.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    async fn on_trade(
        &mut self,
        ctx: &dyn HftStrategyContext,
        trade: &Trade,
    ) -> Result<(), StrategyError>;

    /// Called when the strategy is stopped.
    async fn on_stop(&mut self, _ctx: &dyn HftStrategyContext) {
        // Default implementation does nothing
    }
}

// Allow HashMap in trait definitions - these are interface contracts
// and the actual implementations can use thread-safe alternatives
use chrono::Weekday;
use rust_decimal::Decimal;
#[allow(clippy::disallowed_types)]
use std::collections::HashMap;

use crate::types::Amount;

/// SEL strategy context trait.
///
/// Provides batch position management and portfolio-level operations for SEL
/// (Selection/Scheduled) strategies. SEL strategies are designed for periodic
/// rebalancing and multi-asset portfolio management.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
///
/// # Example
///
/// ```ignore
/// async fn on_schedule(&mut self, ctx: &dyn SelStrategyContext) -> Result<(), StrategyError> {
///     let universe = ctx.get_universe();
///     let mut target_positions = HashMap::new();
///     
///     // Equal weight allocation
///     let weight = Decimal::ONE / Decimal::from(universe.len());
///     let portfolio_value = ctx.get_portfolio_value();
///     
///     for symbol in universe {
///         let price = ctx.get_price(symbol).unwrap();
///         let qty = (portfolio_value.as_decimal() * weight) / price.as_decimal();
///         target_positions.insert(symbol.clone(), Quantity::new(qty).unwrap());
///     }
///     
///     ctx.set_positions_batch(target_positions, "rebalance").await?;
///     Ok(())
/// }
/// ```
#[async_trait]
#[allow(clippy::disallowed_types)]
pub trait SelStrategyContext: Send + Sync {
    /// Gets all current positions.
    ///
    /// Returns a map of symbol to position quantity.
    fn get_all_positions(&self) -> HashMap<Symbol, Quantity>;

    /// Sets target positions for multiple symbols atomically.
    ///
    /// The engine will calculate the required orders to reach the target positions
    /// and execute them as a batch.
    ///
    /// # Arguments
    ///
    /// * `positions` - Map of symbol to target position quantity
    /// * `tag` - A tag for identifying this batch operation
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if the batch operation fails.
    async fn set_positions_batch(
        &self,
        positions: HashMap<Symbol, Quantity>,
        tag: &str,
    ) -> Result<(), StrategyError>;

    /// Gets the investment universe (tradeable symbols).
    fn get_universe(&self) -> &[Symbol];

    /// Sets the investment universe.
    ///
    /// # Arguments
    ///
    /// * `symbols` - List of symbols to include in the universe
    fn set_universe(&self, symbols: Vec<Symbol>);

    /// Gets historical K-line data for a symbol.
    fn get_bars(&self, symbol: &Symbol, period: KlinePeriod, count: usize) -> Vec<KlineData>;

    /// Gets historical K-line data for multiple symbols.
    ///
    /// More efficient than calling `get_bars` multiple times.
    fn get_bars_batch(
        &self,
        symbols: &[Symbol],
        period: KlinePeriod,
        count: usize,
    ) -> HashMap<Symbol, Vec<KlineData>>;

    /// Gets the current price for a symbol.
    fn get_price(&self, symbol: &Symbol) -> Option<Price>;

    /// Gets the current timestamp.
    fn current_time(&self) -> Timestamp;

    /// Gets the total portfolio value (positions + cash).
    fn get_portfolio_value(&self) -> Amount;

    /// Gets the available cash balance.
    fn get_available_cash(&self) -> Amount;

    /// Logs a message with the specified level.
    fn log(&self, level: LogLevel, message: &str);

    /// Gets the strategy name.
    fn strategy_name(&self) -> &str;

    /// Gets the current position weights (position value / portfolio value).
    fn get_position_weights(&self) -> HashMap<Symbol, Decimal>;

    /// Calculates the deviation from target weights.
    ///
    /// # Arguments
    ///
    /// * `target_weights` - Map of symbol to target weight (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// Map of symbol to deviation (current weight - target weight)
    fn calculate_weight_deviation(
        &self,
        target_weights: &HashMap<Symbol, Decimal>,
    ) -> HashMap<Symbol, Decimal>;
}

/// SEL strategy trait.
///
/// Defines the interface for SEL (Selection/Scheduled) strategies.
/// SEL strategies are triggered on a schedule and manage portfolios
/// of multiple assets.
///
/// # Lifecycle
///
/// 1. `on_init` - Called once when the strategy is initialized
/// 2. `on_schedule` - Called at scheduled intervals (daily, weekly, etc.)
/// 3. `on_bar` - Optionally called on bar events for monitoring
/// 4. Strategy uses context to set batch positions
/// 5. Engine handles order execution
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
#[async_trait]
pub trait SelStrategy: Send + Sync {
    /// Returns the strategy name.
    fn name(&self) -> &str;

    /// Called when the strategy is initialized.
    ///
    /// Use this to set up initial state, define universe, etc.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if initialization fails.
    async fn on_init(&mut self, ctx: &dyn SelStrategyContext) -> Result<(), StrategyError>;

    /// Called at scheduled intervals.
    ///
    /// This is the main entry point for SEL strategies to perform
    /// portfolio rebalancing and position adjustments.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    async fn on_schedule(&mut self, ctx: &dyn SelStrategyContext) -> Result<(), StrategyError>;

    /// Called when a new bar closes (optional).
    ///
    /// Can be used for real-time monitoring between scheduled events.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    async fn on_bar(
        &mut self,
        _ctx: &dyn SelStrategyContext,
        _bar: &KlineData,
    ) -> Result<(), StrategyError> {
        Ok(()) // Default implementation does nothing
    }

    /// Called when the strategy is stopped.
    async fn on_stop(&mut self, _ctx: &dyn SelStrategyContext) {
        // Default implementation does nothing
    }
}

/// SEL schedule configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelScheduleConfig {
    /// Schedule frequency.
    pub frequency: ScheduleFrequency,
    /// Timezone for schedule (e.g., "UTC", "America/New\_York").
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Tolerance threshold for triggering rebalance (0.0 to 1.0).
    /// If any position deviates more than this from target, rebalance is triggered.
    #[serde(default = "default_rebalance_tolerance")]
    pub rebalance_tolerance: Decimal,
}

fn default_timezone() -> String {
    "UTC".to_string()
}

fn default_rebalance_tolerance() -> Decimal {
    Decimal::new(5, 2) // 0.05 = 5%
}

impl Default for SelScheduleConfig {
    fn default() -> Self {
        Self {
            frequency: ScheduleFrequency::Daily { hour: 0, minute: 0 },
            timezone: default_timezone(),
            rebalance_tolerance: default_rebalance_tolerance(),
        }
    }
}

/// Schedule frequency for SEL strategies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduleFrequency {
    /// Daily at specified time.
    Daily {
        /// Hour (0-23).
        hour: u8,
        /// Minute (0-59).
        minute: u8,
    },
    /// Weekly on specified day and time.
    Weekly {
        /// Day of week.
        day: Weekday,
        /// Hour (0-23).
        hour: u8,
        /// Minute (0-59).
        minute: u8,
    },
    /// Monthly on specified day and time.
    Monthly {
        /// Day of month (1-31).
        day: u8,
        /// Hour (0-23).
        hour: u8,
        /// Minute (0-59).
        minute: u8,
    },
    /// Custom cron expression.
    Cron(String),
}

impl ScheduleFrequency {
    /// Creates a daily schedule.
    #[must_use]
    pub fn daily(hour: u8, minute: u8) -> Self {
        Self::Daily { hour, minute }
    }

    /// Creates a weekly schedule.
    #[must_use]
    pub fn weekly(day: Weekday, hour: u8, minute: u8) -> Self {
        Self::Weekly { day, hour, minute }
    }

    /// Creates a monthly schedule.
    #[must_use]
    pub fn monthly(day: u8, hour: u8, minute: u8) -> Self {
        Self::Monthly { day, hour, minute }
    }

    /// Creates a cron schedule.
    #[must_use]
    pub fn cron(expression: impl Into<String>) -> Self {
        Self::Cron(expression.into())
    }

    /// Validates the schedule frequency.
    ///
    /// # Errors
    ///
    /// Returns an error message if the schedule is invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Daily { hour, minute } | Self::Weekly { hour, minute, .. } => {
                if *hour > 23 {
                    return Err("Hour must be 0-23");
                }
                if *minute > 59 {
                    return Err("Minute must be 0-59");
                }
            }
            Self::Monthly { day, hour, minute } => {
                if *day == 0 || *day > 31 {
                    return Err("Day must be 1-31");
                }
                if *hour > 23 {
                    return Err("Hour must be 0-23");
                }
                if *minute > 59 {
                    return Err("Minute must be 0-59");
                }
            }
            Self::Cron(expr) => {
                if expr.is_empty() {
                    return Err("Cron expression cannot be empty");
                }
                // Basic validation - should have 5 or 6 fields
                let fields: Vec<&str> = expr.split_whitespace().collect();
                if fields.len() < 5 || fields.len() > 6 {
                    return Err("Cron expression must have 5 or 6 fields");
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for ScheduleFrequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Daily { hour, minute } => write!(f, "daily at {hour:02}:{minute:02}"),
            Self::Weekly { day, hour, minute } => {
                write!(f, "weekly on {day:?} at {hour:02}:{minute:02}")
            }
            Self::Monthly { day, hour, minute } => {
                write!(f, "monthly on day {day} at {hour:02}:{minute:02}")
            }
            Self::Cron(expr) => write!(f, "cron: {expr}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_flag_display() {
        assert_eq!(format!("{}", OrderFlag::Normal), "normal");
        assert_eq!(format!("{}", OrderFlag::Fak), "fak");
        assert_eq!(format!("{}", OrderFlag::Fok), "fok");
        assert_eq!(format!("{}", OrderFlag::PostOnly), "post_only");
        assert_eq!(format!("{}", OrderFlag::ReduceOnly), "reduce_only");
    }

    #[test]
    fn test_order_flag_default() {
        let flag = OrderFlag::default();
        assert_eq!(flag, OrderFlag::Normal);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(format!("{}", LogLevel::Trace), "trace");
        assert_eq!(format!("{}", LogLevel::Debug), "debug");
        assert_eq!(format!("{}", LogLevel::Info), "info");
        assert_eq!(format!("{}", LogLevel::Warn), "warn");
        assert_eq!(format!("{}", LogLevel::Error), "error");
    }

    #[test]
    fn test_order_flag_serde() {
        let flag = OrderFlag::Fok;
        let json = serde_json::to_string(&flag).unwrap();
        assert_eq!(json, "\"fok\"");

        let parsed: OrderFlag = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, OrderFlag::Fok);
    }

    #[test]
    fn test_log_level_serde() {
        let level = LogLevel::Warn;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"warn\"");

        let parsed: LogLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, LogLevel::Warn);
    }
}

// =============================================================================
// UFT (Ultra-High Frequency Trading) Strategy Types
// =============================================================================

use crate::data::OrderBook;

/// UFT strategy context trait.
///
/// Provides lock-free, zero-copy access for ultra-high frequency trading strategies.
/// UFT strategies are designed for microsecond-level latency with direct order submission.
///
/// # Design Principles
///
/// - **Lock-free operations**: All methods are synchronous and avoid locks
/// - **Zero-copy data access**: Data is accessed by reference without copying
/// - **Hardware timestamps**: Nanosecond precision timing support
/// - **Pre-allocated resources**: Order pools and buffers are pre-allocated
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
/// Internal synchronization should use lock-free data structures.
///
/// # Example
///
/// ```ignore
/// fn on_tick(&mut self, ctx: &dyn UftStrategyContext, tick: &TickData) {
///     let position = ctx.get_position(&tick.symbol);
///     
///     // Ultra-low latency market making
///     if let Some(orderbook) = ctx.get_orderbook(&tick.symbol) {
///         if let (Some(bid), Some(ask)) = (orderbook.best_bid(), orderbook.best_ask()) {
///             let spread = ask.price - bid.price;
///             if spread > min_spread {
///                 // Place orders directly without async overhead
///                 ctx.buy(&tick.symbol, bid.price, qty, OrderFlag::PostOnly);
///                 ctx.sell(&tick.symbol, ask.price, qty, OrderFlag::PostOnly);
///             }
///         }
///     }
/// }
/// ```
pub trait UftStrategyContext: Send + Sync {
    /// Submits a buy order - lock-free direct submission.
    ///
    /// This method is synchronous and returns immediately with an order ID.
    /// The order is submitted to a lock-free queue for processing.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `price` - Limit price
    /// * `qty` - Order quantity
    /// * `flag` - Order execution flag
    ///
    /// # Returns
    ///
    /// Returns the order ID immediately (pre-generated).
    fn buy(&self, symbol: &Symbol, price: Price, qty: Quantity, flag: OrderFlag) -> OrderId;

    /// Submits a sell order - lock-free direct submission.
    ///
    /// This method is synchronous and returns immediately with an order ID.
    /// The order is submitted to a lock-free queue for processing.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `price` - Limit price
    /// * `qty` - Order quantity
    /// * `flag` - Order execution flag
    ///
    /// # Returns
    ///
    /// Returns the order ID immediately (pre-generated).
    fn sell(&self, symbol: &Symbol, price: Price, qty: Quantity, flag: OrderFlag) -> OrderId;

    /// Cancels an order - lock-free direct submission.
    ///
    /// # Returns
    ///
    /// Returns `true` if the cancel request was submitted successfully.
    fn cancel(&self, order_id: &OrderId) -> bool;

    /// Cancels all orders for a symbol.
    ///
    /// # Returns
    ///
    /// Returns the number of cancel requests submitted.
    fn cancel_all(&self, symbol: &Symbol) -> usize;

    /// Gets the current position for a symbol - zero-copy access.
    fn get_position(&self, symbol: &Symbol) -> Quantity;

    /// Gets the latest tick data - zero-copy access.
    ///
    /// Returns a reference to avoid copying tick data.
    fn get_last_tick(&self, symbol: &Symbol) -> Option<&TickData>;

    /// Gets the current order book - zero-copy access.
    ///
    /// Returns a reference to avoid copying order book data.
    fn get_orderbook(&self, symbol: &Symbol) -> Option<&OrderBook>;

    /// Gets pending orders for a symbol - zero-copy access.
    fn get_pending_orders(&self, symbol: &Symbol) -> &[Order];

    /// Gets the current timestamp in nanoseconds - hardware timestamp support.
    ///
    /// Uses high-resolution clock (TSC or similar) when available.
    fn current_time_ns(&self) -> i64;

    /// Gets the current price for a symbol.
    fn get_price(&self, symbol: &Symbol) -> Option<Price>;

    /// Logs a message with the specified level.
    ///
    /// Note: Logging in UFT context should be minimal to avoid latency impact.
    fn log(&self, level: LogLevel, message: &str);

    /// Gets the strategy name.
    fn strategy_name(&self) -> &str;
}

/// UFT strategy trait.
///
/// Defines the interface for UFT (Ultra-High Frequency Trading) strategies.
/// UFT strategies have synchronous callbacks for minimum latency.
///
/// # Performance Requirements
///
/// - `on_tick` must complete within 10 microseconds
/// - `on_orderbook` must complete within 10 microseconds
/// - All callbacks are synchronous (no async overhead)
///
/// # Lifecycle
///
/// 1. `on_init` - Called once when the strategy is initialized
/// 2. `on_tick` / `on_orderbook` - Called on market data events
/// 3. `on_order` / `on_trade` - Called on order/trade events
/// 4. Strategy directly submits/cancels orders via context
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
pub trait UftStrategy: Send + Sync {
    /// Returns the strategy name.
    fn name(&self) -> &str;

    /// Called when the strategy is initialized.
    ///
    /// Use this to set up initial state, pre-allocate resources, etc.
    fn on_init(&mut self, ctx: &dyn UftStrategyContext);

    /// Called when a new tick is received.
    ///
    /// **CRITICAL**: This callback must complete within 10 microseconds.
    fn on_tick(&mut self, ctx: &dyn UftStrategyContext, tick: &TickData);

    /// Called when an order update is received.
    fn on_order(&mut self, ctx: &dyn UftStrategyContext, order: &Order);

    /// Called when a trade execution is received.
    fn on_trade(&mut self, ctx: &dyn UftStrategyContext, trade: &Trade);

    /// Called when the order book is updated.
    ///
    /// **CRITICAL**: This callback must complete within 10 microseconds.
    fn on_orderbook(&mut self, ctx: &dyn UftStrategyContext, orderbook: &OrderBook);

    /// Called when the strategy is stopped.
    fn on_stop(&mut self, _ctx: &dyn UftStrategyContext) {
        // Default implementation does nothing
    }
}

/// UFT engine configuration.
///
/// Configuration for the ultra-high frequency trading engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UftEngineConfig {
    /// CPU cores to bind strategy threads to.
    /// Empty means no CPU affinity.
    #[serde(default)]
    pub cpu_affinity: Vec<usize>,

    /// Whether to use kernel bypass networking (`DPDK`/`io_uring`).
    /// Requires special system configuration.
    #[serde(default)]
    pub use_kernel_bypass: bool,

    /// Number of pre-allocated orders in the order pool.
    #[serde(default = "default_preallocated_orders")]
    pub preallocated_orders: usize,

    /// Size of the tick data buffer per symbol.
    #[serde(default = "default_tick_buffer_size")]
    pub tick_buffer_size: usize,

    /// Size of the order book buffer per symbol.
    #[serde(default = "default_orderbook_buffer_size")]
    pub orderbook_buffer_size: usize,

    /// Maximum pending orders per symbol.
    #[serde(default = "default_max_pending_orders")]
    pub max_pending_orders: usize,

    /// Enable hardware timestamp support.
    #[serde(default = "default_hardware_timestamp")]
    pub hardware_timestamp: bool,
}

fn default_preallocated_orders() -> usize {
    10_000
}

fn default_tick_buffer_size() -> usize {
    1_000
}

fn default_orderbook_buffer_size() -> usize {
    100
}

fn default_max_pending_orders() -> usize {
    1_000
}

fn default_hardware_timestamp() -> bool {
    true
}

impl Default for UftEngineConfig {
    fn default() -> Self {
        Self {
            cpu_affinity: Vec::new(),
            use_kernel_bypass: false,
            preallocated_orders: default_preallocated_orders(),
            tick_buffer_size: default_tick_buffer_size(),
            orderbook_buffer_size: default_orderbook_buffer_size(),
            max_pending_orders: default_max_pending_orders(),
            hardware_timestamp: default_hardware_timestamp(),
        }
    }
}

impl UftEngineConfig {
    /// Creates a new builder for `UftEngineConfig`.
    #[must_use]
    pub fn builder() -> UftEngineConfigBuilder {
        UftEngineConfigBuilder::default()
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error message if the configuration is invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.preallocated_orders == 0 {
            return Err("preallocated_orders must be greater than 0");
        }
        if self.tick_buffer_size == 0 {
            return Err("tick_buffer_size must be greater than 0");
        }
        if self.orderbook_buffer_size == 0 {
            return Err("orderbook_buffer_size must be greater than 0");
        }
        if self.max_pending_orders == 0 {
            return Err("max_pending_orders must be greater than 0");
        }
        Ok(())
    }
}

/// Builder for `UftEngineConfig`.
#[derive(Debug, Default)]
pub struct UftEngineConfigBuilder {
    cpu_affinity: Vec<usize>,
    use_kernel_bypass: bool,
    preallocated_orders: Option<usize>,
    tick_buffer_size: Option<usize>,
    orderbook_buffer_size: Option<usize>,
    max_pending_orders: Option<usize>,
    hardware_timestamp: Option<bool>,
}

impl UftEngineConfigBuilder {
    /// Sets the CPU affinity.
    #[must_use]
    pub fn cpu_affinity(mut self, cores: Vec<usize>) -> Self {
        self.cpu_affinity = cores;
        self
    }

    /// Enables kernel bypass networking.
    #[must_use]
    pub fn use_kernel_bypass(mut self, enabled: bool) -> Self {
        self.use_kernel_bypass = enabled;
        self
    }

    /// Sets the number of pre-allocated orders.
    #[must_use]
    pub fn preallocated_orders(mut self, count: usize) -> Self {
        self.preallocated_orders = Some(count);
        self
    }

    /// Sets the tick buffer size.
    #[must_use]
    pub fn tick_buffer_size(mut self, size: usize) -> Self {
        self.tick_buffer_size = Some(size);
        self
    }

    /// Sets the order book buffer size.
    #[must_use]
    pub fn orderbook_buffer_size(mut self, size: usize) -> Self {
        self.orderbook_buffer_size = Some(size);
        self
    }

    /// Sets the maximum pending orders.
    #[must_use]
    pub fn max_pending_orders(mut self, count: usize) -> Self {
        self.max_pending_orders = Some(count);
        self
    }

    /// Enables hardware timestamp support.
    #[must_use]
    pub fn hardware_timestamp(mut self, enabled: bool) -> Self {
        self.hardware_timestamp = Some(enabled);
        self
    }

    /// Builds the `UftEngineConfig`.
    #[must_use]
    pub fn build(self) -> UftEngineConfig {
        UftEngineConfig {
            cpu_affinity: self.cpu_affinity,
            use_kernel_bypass: self.use_kernel_bypass,
            preallocated_orders: self
                .preallocated_orders
                .unwrap_or_else(default_preallocated_orders),
            tick_buffer_size: self
                .tick_buffer_size
                .unwrap_or_else(default_tick_buffer_size),
            orderbook_buffer_size: self
                .orderbook_buffer_size
                .unwrap_or_else(default_orderbook_buffer_size),
            max_pending_orders: self
                .max_pending_orders
                .unwrap_or_else(default_max_pending_orders),
            hardware_timestamp: self
                .hardware_timestamp
                .unwrap_or_else(default_hardware_timestamp),
        }
    }
}
