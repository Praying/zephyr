//! Core traits for the execution unit framework.
//!
//! This module defines the fundamental traits that all execution units
//! must implement, as well as the context trait for market data access
//! and order submission.

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use zephyr_core::data::{Order, TickData};
use zephyr_core::error::ExchangeError;
use zephyr_core::traits::Trade;
use zephyr_core::types::{OrderId, Price, Quantity, Symbol, Timestamp};

use super::ExecuteConfig;

/// Execution context trait.
///
/// Provides market data access and order submission capabilities
/// for execution units. This is the primary interface through which
/// execution algorithms interact with the trading system.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
///
/// # Example
///
/// ```ignore
/// async fn execute_slice(&self, ctx: &dyn ExecuteContext) -> Result<(), ExchangeError> {
///     let tick = ctx.get_last_tick(&self.symbol)?;
///     let price = tick.best_ask().unwrap();
///     let order_ids = ctx.buy(&self.symbol, price, self.slice_qty).await?;
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait ExecuteContext: Send + Sync {
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

    /// Gets the most recent tick for a symbol.
    ///
    /// # Returns
    ///
    /// The latest tick data, or `None` if no data is available.
    fn get_last_tick(&self, symbol: &Symbol) -> Option<&TickData>;

    /// Gets the current position for a symbol.
    ///
    /// Returns the actual position quantity (positive for long, negative for short).
    fn get_position(&self, symbol: &Symbol) -> Quantity;

    /// Gets the undone (pending) quantity for a symbol.
    ///
    /// This is the quantity that has been ordered but not yet filled.
    fn get_undone_qty(&self, symbol: &Symbol) -> Quantity;

    /// Submits buy orders.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `price` - Limit price
    /// * `qty` - Order quantity
    ///
    /// # Returns
    ///
    /// Returns a vector of order IDs for the submitted orders.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if order submission fails.
    async fn buy(
        &self,
        symbol: &Symbol,
        price: Price,
        qty: Quantity,
    ) -> Result<Vec<OrderId>, ExchangeError>;

    /// Submits sell orders.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `price` - Limit price
    /// * `qty` - Order quantity
    ///
    /// # Returns
    ///
    /// Returns a vector of order IDs for the submitted orders.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if order submission fails.
    async fn sell(
        &self,
        symbol: &Symbol,
        price: Price,
        qty: Quantity,
    ) -> Result<Vec<OrderId>, ExchangeError>;

    /// Cancels an order.
    ///
    /// # Returns
    ///
    /// Returns `true` if the order was successfully canceled.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if cancellation fails.
    async fn cancel(&self, order_id: &OrderId) -> Result<bool, ExchangeError>;

    /// Cancels all pending orders for a symbol.
    ///
    /// # Returns
    ///
    /// Returns the number of orders canceled.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if cancellation fails.
    async fn cancel_all(&self, symbol: &Symbol) -> Result<usize, ExchangeError>;

    /// Gets the current timestamp.
    fn current_time(&self) -> Timestamp;

    /// Logs a message.
    fn log(&self, level: tracing::Level, message: &str);
}

/// Execution unit trait.
///
/// Defines the interface for execution algorithms that handle
/// order slicing and execution. Execution units receive target
/// positions and break them down into smaller orders over time.
///
/// # Lifecycle
///
/// 1. `init()` - Initialize with context and configuration
/// 2. `set_position()` - Set target position to execute
/// 3. `on_tick()` / `on_trade()` / `on_order()` - Process events
/// 4. `on_channel_ready()` / `on_channel_lost()` - Handle connectivity
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
#[async_trait]
pub trait ExecuteUnit: Send + Sync {
    /// Returns the execution unit name.
    fn name(&self) -> &str;

    /// Returns the factory name that created this unit.
    fn factory_name(&self) -> &str;

    /// Initializes the execution unit.
    ///
    /// Called once when the execution unit is created and assigned
    /// to a symbol.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The execution context
    /// * `symbol` - The symbol this unit will execute for
    /// * `config` - Execution configuration
    async fn init(&mut self, ctx: Arc<dyn ExecuteContext>, symbol: &Symbol, config: &ExecuteConfig);

    /// Sets the target position to execute.
    ///
    /// The execution unit will work towards this target position
    /// by submitting orders over time according to its algorithm.
    ///
    /// # Arguments
    ///
    /// * `target` - Target position quantity (positive for long, negative for short)
    async fn set_position(&mut self, target: Quantity);

    /// Called when a new tick is received.
    ///
    /// Execution units should use this to update their state and
    /// potentially submit new orders.
    async fn on_tick(&mut self, tick: &TickData);

    /// Called when a trade execution is received.
    ///
    /// Execution units should use this to track fill progress.
    async fn on_trade(&mut self, trade: &Trade);

    /// Called when an order update is received.
    ///
    /// Execution units should use this to track order status.
    async fn on_order(&mut self, order: &Order);

    /// Called when the trading channel becomes ready.
    ///
    /// Execution units can resume order submission after this.
    async fn on_channel_ready(&mut self);

    /// Called when the trading channel is lost.
    ///
    /// Execution units should pause order submission until
    /// `on_channel_ready` is called.
    async fn on_channel_lost(&mut self);

    /// Returns the current execution progress.
    fn progress(&self) -> ExecutionProgress;

    /// Returns execution metrics.
    fn metrics(&self) -> ExecutionMetrics;

    /// Returns whether execution is complete.
    fn is_complete(&self) -> bool;

    /// Cancels the current execution.
    ///
    /// Cancels all pending orders and stops execution.
    async fn cancel(&mut self);
}

/// Execution progress information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionProgress {
    /// Target position quantity.
    pub target_qty: Quantity,
    /// Current executed quantity.
    pub executed_qty: Quantity,
    /// Pending (ordered but not filled) quantity.
    pub pending_qty: Quantity,
    /// Remaining quantity to execute.
    pub remaining_qty: Quantity,
    /// Number of slices completed.
    pub slices_completed: u32,
    /// Total number of slices planned.
    pub total_slices: u32,
    /// Execution start time.
    pub start_time: Option<Timestamp>,
    /// Expected completion time.
    pub expected_end_time: Option<Timestamp>,
    /// Progress percentage (0.0 to 1.0).
    pub progress_pct: Decimal,
}

impl ExecutionProgress {
    /// Creates a new execution progress.
    #[must_use]
    pub fn new(target_qty: Quantity, total_slices: u32) -> Self {
        Self {
            target_qty,
            executed_qty: Quantity::ZERO,
            pending_qty: Quantity::ZERO,
            remaining_qty: target_qty,
            slices_completed: 0,
            total_slices,
            start_time: None,
            expected_end_time: None,
            progress_pct: Decimal::ZERO,
        }
    }

    /// Updates progress after a fill.
    pub fn record_fill(&mut self, filled_qty: Quantity) {
        self.executed_qty = self.executed_qty + filled_qty;
        self.remaining_qty = self.target_qty - self.executed_qty;
        if !self.target_qty.is_zero() {
            self.progress_pct = self.executed_qty.as_decimal() / self.target_qty.as_decimal().abs();
        }
    }

    /// Updates pending quantity.
    pub fn set_pending(&mut self, pending_qty: Quantity) {
        self.pending_qty = pending_qty;
    }

    /// Increments completed slices.
    pub fn complete_slice(&mut self) {
        self.slices_completed += 1;
    }
}

/// Execution metrics for performance tracking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// Total quantity executed.
    pub total_executed: Quantity,
    /// Total value executed.
    pub total_value: Decimal,
    /// Average execution price.
    pub avg_price: Option<Price>,
    /// Volume-weighted average price.
    pub vwap: Option<Price>,
    /// Slippage from initial price (in basis points).
    pub slippage_bps: Decimal,
    /// Number of orders submitted.
    pub orders_submitted: u32,
    /// Number of orders filled.
    pub orders_filled: u32,
    /// Number of orders canceled.
    pub orders_canceled: u32,
    /// Number of orders rejected.
    pub orders_rejected: u32,
    /// Total execution time in milliseconds.
    pub execution_time_ms: u64,
    /// Market participation rate (our volume / total volume).
    pub participation_rate: Decimal,
}

impl ExecutionMetrics {
    /// Records a fill.
    pub fn record_fill(&mut self, qty: Quantity, price: Price) {
        let value = qty.as_decimal() * price.as_decimal();
        self.total_executed = self.total_executed + qty;
        self.total_value += value;

        // Update VWAP
        if !self.total_executed.is_zero() {
            let vwap_value = self.total_value / self.total_executed.as_decimal();
            self.vwap = Price::new(vwap_value).ok();
            self.avg_price = self.vwap;
        }

        self.orders_filled += 1;
    }

    /// Records an order submission.
    pub fn record_order_submitted(&mut self) {
        self.orders_submitted += 1;
    }

    /// Records an order cancellation.
    pub fn record_order_canceled(&mut self) {
        self.orders_canceled += 1;
    }

    /// Records an order rejection.
    pub fn record_order_rejected(&mut self) {
        self.orders_rejected += 1;
    }

    /// Calculates slippage from a reference price.
    pub fn calculate_slippage(&mut self, reference_price: Price) {
        if let Some(avg) = self.avg_price {
            let diff = avg.as_decimal() - reference_price.as_decimal();
            self.slippage_bps = (diff / reference_price.as_decimal()) * Decimal::from(10000);
        }
    }
}

/// A sliced order for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceOrder {
    /// Slice index (0-based).
    pub slice_index: u32,
    /// Order quantity for this slice.
    pub quantity: Quantity,
    /// Target price for this slice.
    pub price: Option<Price>,
    /// Scheduled execution time.
    pub scheduled_time: Timestamp,
    /// Whether this slice has been executed.
    pub executed: bool,
    /// Actual execution time.
    pub execution_time: Option<Timestamp>,
    /// Order IDs for this slice.
    pub order_ids: Vec<OrderId>,
}

impl SliceOrder {
    /// Creates a new slice order.
    #[must_use]
    pub fn new(slice_index: u32, quantity: Quantity, scheduled_time: Timestamp) -> Self {
        Self {
            slice_index,
            quantity,
            price: None,
            scheduled_time,
            executed: false,
            execution_time: None,
            order_ids: Vec::new(),
        }
    }

    /// Sets the target price.
    #[must_use]
    pub fn with_price(mut self, price: Price) -> Self {
        self.price = Some(price);
        self
    }

    /// Marks the slice as executed.
    pub fn mark_executed(&mut self, time: Timestamp, order_ids: Vec<OrderId>) {
        self.executed = true;
        self.execution_time = Some(time);
        self.order_ids = order_ids;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_execution_progress_new() {
        let target = Quantity::new(dec!(100)).unwrap();
        let progress = ExecutionProgress::new(target, 10);

        assert_eq!(progress.target_qty, target);
        assert_eq!(progress.executed_qty, Quantity::ZERO);
        assert_eq!(progress.total_slices, 10);
        assert_eq!(progress.slices_completed, 0);
    }

    #[test]
    fn test_execution_progress_record_fill() {
        let target = Quantity::new(dec!(100)).unwrap();
        let mut progress = ExecutionProgress::new(target, 10);

        let fill = Quantity::new(dec!(25)).unwrap();
        progress.record_fill(fill);

        assert_eq!(progress.executed_qty.as_decimal(), dec!(25));
        assert_eq!(progress.remaining_qty.as_decimal(), dec!(75));
        assert_eq!(progress.progress_pct, dec!(0.25));
    }

    #[test]
    fn test_execution_metrics_record_fill() {
        let mut metrics = ExecutionMetrics::default();

        let qty = Quantity::new(dec!(10)).unwrap();
        let price = Price::new(dec!(100)).unwrap();
        metrics.record_fill(qty, price);

        assert_eq!(metrics.total_executed.as_decimal(), dec!(10));
        assert_eq!(metrics.total_value, dec!(1000));
        assert_eq!(metrics.orders_filled, 1);
        assert!(metrics.vwap.is_some());
    }

    #[test]
    fn test_execution_metrics_slippage() {
        let mut metrics = ExecutionMetrics::default();

        // Execute at 101 when reference was 100
        let qty = Quantity::new(dec!(10)).unwrap();
        let price = Price::new(dec!(101)).unwrap();
        metrics.record_fill(qty, price);

        let reference = Price::new(dec!(100)).unwrap();
        metrics.calculate_slippage(reference);

        // Slippage should be 100 bps (1%)
        assert_eq!(metrics.slippage_bps, dec!(100));
    }

    #[test]
    fn test_slice_order_new() {
        let qty = Quantity::new(dec!(10)).unwrap();
        let time = Timestamp::now();
        let slice = SliceOrder::new(0, qty, time);

        assert_eq!(slice.slice_index, 0);
        assert_eq!(slice.quantity, qty);
        assert!(!slice.executed);
        assert!(slice.order_ids.is_empty());
    }

    #[test]
    fn test_slice_order_mark_executed() {
        let qty = Quantity::new(dec!(10)).unwrap();
        let time = Timestamp::now();
        let mut slice = SliceOrder::new(0, qty, time);

        let exec_time = Timestamp::now();
        let order_ids = vec![OrderId::generate()];
        slice.mark_executed(exec_time, order_ids.clone());

        assert!(slice.executed);
        assert_eq!(slice.execution_time, Some(exec_time));
        assert_eq!(slice.order_ids.len(), 1);
    }
}
