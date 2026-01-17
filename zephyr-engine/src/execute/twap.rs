//! TWAP (Time-Weighted Average Price) execution unit.
//!
//! Executes orders by splitting them into equal-sized slices
//! distributed evenly over a time period.

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::match_result_ok)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::unnecessary_literal_bound)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::bind_instead_of_map)]

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use zephyr_core::data::{Order, OrderSide, OrderStatus, TickData};
use zephyr_core::traits::Trade;
use zephyr_core::types::{OrderId, Price, Quantity, Symbol, Timestamp};

use super::config::ExecuteConfig;
use super::traits::{ExecuteContext, ExecuteUnit, ExecutionMetrics, ExecutionProgress, SliceOrder};

/// TWAP execution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwapConfig {
    /// Number of slices to divide the order into.
    #[serde(default = "default_slices")]
    pub slices: u32,

    /// Time interval between slices.
    #[serde(default = "default_interval")]
    #[serde(with = "humantime_serde")]
    pub interval: Duration,

    /// Whether to randomize slice timing (±10% jitter).
    #[serde(default)]
    pub randomize_timing: bool,

    /// Whether to use aggressive pricing (cross the spread).
    #[serde(default)]
    pub aggressive: bool,

    /// Price offset in ticks from best bid/ask.
    #[serde(default)]
    pub price_offset_ticks: i32,

    /// Whether to cancel unfilled slices before next slice.
    #[serde(default = "default_true")]
    pub cancel_before_next: bool,

    /// Minimum slice size (skip if below).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_slice_size: Option<Decimal>,
}

fn default_slices() -> u32 {
    10
}

fn default_interval() -> Duration {
    Duration::from_secs(60)
}

fn default_true() -> bool {
    true
}

impl Default for TwapConfig {
    fn default() -> Self {
        Self {
            slices: default_slices(),
            interval: default_interval(),
            randomize_timing: false,
            aggressive: false,
            price_offset_ticks: 0,
            cancel_before_next: true,
            min_slice_size: None,
        }
    }
}

impl TwapConfig {
    /// Creates a new TWAP configuration.
    #[must_use]
    pub fn new(slices: u32, interval: Duration) -> Self {
        Self {
            slices,
            interval,
            ..Default::default()
        }
    }

    /// Sets randomize timing.
    #[must_use]
    pub fn with_randomize_timing(mut self, randomize: bool) -> Self {
        self.randomize_timing = randomize;
        self
    }

    /// Sets aggressive pricing.
    #[must_use]
    pub fn with_aggressive(mut self, aggressive: bool) -> Self {
        self.aggressive = aggressive;
        self
    }

    /// Sets price offset in ticks.
    #[must_use]
    pub fn with_price_offset(mut self, ticks: i32) -> Self {
        self.price_offset_ticks = ticks;
        self
    }

    /// Sets minimum slice size.
    #[must_use]
    pub fn with_min_slice_size(mut self, size: Decimal) -> Self {
        self.min_slice_size = Some(size);
        self
    }
}

/// TWAP execution unit.
///
/// Executes orders by splitting them into equal-sized slices
/// distributed evenly over a time period.
///
/// # Algorithm
///
/// 1. Divide total quantity into N equal slices
/// 2. Schedule each slice at regular intervals
/// 3. At each interval, submit a limit order at current price
/// 4. Track fills and adjust remaining quantity
/// 5. Cancel unfilled orders before next slice (optional)
pub struct TwapExecutor {
    /// Configuration.
    config: TwapConfig,
    /// Execution context.
    ctx: Option<Arc<dyn ExecuteContext>>,
    /// Symbol being executed.
    symbol: Option<Symbol>,
    /// Execution configuration.
    exec_config: Option<ExecuteConfig>,
    /// Target position.
    target_qty: Quantity,
    /// Current position at start.
    start_position: Quantity,
    /// Order side (Buy or Sell).
    side: Option<OrderSide>,
    /// Planned slices.
    slices: Vec<SliceOrder>,
    /// Current slice index.
    current_slice: u32,
    /// Execution progress.
    progress: ExecutionProgress,
    /// Execution metrics.
    metrics: ExecutionMetrics,
    /// Reference price at start.
    reference_price: Option<Price>,
    /// Active order IDs.
    active_orders: Vec<OrderId>,
    /// Whether execution is complete.
    complete: bool,
    /// Whether channel is ready.
    channel_ready: bool,
    /// Last tick received.
    last_tick: Option<TickData>,
}

impl TwapExecutor {
    /// Creates a new TWAP executor.
    #[must_use]
    pub fn new(config: TwapConfig) -> Self {
        Self {
            config,
            ctx: None,
            symbol: None,
            exec_config: None,
            target_qty: Quantity::ZERO,
            start_position: Quantity::ZERO,
            side: None,
            slices: Vec::new(),
            current_slice: 0,
            progress: ExecutionProgress::default(),
            metrics: ExecutionMetrics::default(),
            reference_price: None,
            active_orders: Vec::new(),
            complete: true,
            channel_ready: true,
            last_tick: None,
        }
    }

    /// Calculates slice quantities.
    fn calculate_slices(&mut self, total_qty: Quantity) {
        let num_slices = self.config.slices;
        if num_slices == 0 {
            return;
        }

        let qty_per_slice = total_qty.as_decimal() / Decimal::from(num_slices);
        let start_time = self
            .ctx
            .as_ref()
            .map(|c| c.current_time())
            .unwrap_or_else(Timestamp::now);

        self.slices.clear();

        for i in 0..num_slices {
            let scheduled_time = Timestamp::new(
                start_time.as_millis() + (i as i64 * self.config.interval.as_millis() as i64),
            )
            .unwrap_or(start_time);

            let slice_qty = if let Some(qty) = Quantity::new(qty_per_slice).ok() {
                qty
            } else {
                continue;
            };

            // Skip slices below minimum size
            if let Some(min_size) = self.config.min_slice_size {
                if slice_qty.as_decimal() < min_size {
                    continue;
                }
            }

            self.slices
                .push(SliceOrder::new(i, slice_qty, scheduled_time));
        }

        self.progress = ExecutionProgress::new(total_qty, self.slices.len() as u32);
        self.progress.start_time = Some(start_time);

        if let Some(last_slice) = self.slices.last() {
            self.progress.expected_end_time = Some(last_slice.scheduled_time);
        }
    }

    /// Gets the price for the current slice.
    fn get_slice_price(&self) -> Option<Price> {
        let tick = self.last_tick.as_ref()?;
        let side = self.side?;

        let base_price = if self.config.aggressive {
            // Cross the spread
            match side {
                OrderSide::Buy => tick.best_ask()?,
                OrderSide::Sell => tick.best_bid()?,
            }
        } else {
            // Join the spread
            match side {
                OrderSide::Buy => tick.best_bid()?,
                OrderSide::Sell => tick.best_ask()?,
            }
        };

        // Apply price offset
        if self.config.price_offset_ticks != 0 {
            if let Some(exec_config) = &self.exec_config {
                if let Some(tick_size) = exec_config.tick_size {
                    let offset = tick_size * Decimal::from(self.config.price_offset_ticks);
                    let adjusted = match side {
                        OrderSide::Buy => base_price.as_decimal() + offset,
                        OrderSide::Sell => base_price.as_decimal() - offset,
                    };
                    return Price::new(adjusted).ok();
                }
            }
        }

        Some(base_price)
    }

    /// Executes the current slice.
    async fn execute_slice(&mut self) -> Result<(), String> {
        let ctx = self.ctx.as_ref().ok_or("Context not initialized")?;
        let symbol = self.symbol.as_ref().ok_or("Symbol not set")?;
        let side = self.side.ok_or("Side not determined")?;

        let slice_idx = self.current_slice as usize;
        if slice_idx >= self.slices.len() {
            return Ok(());
        }

        let slice = &self.slices[slice_idx];
        if slice.executed {
            return Ok(());
        }

        let price = self.get_slice_price().ok_or("Cannot determine price")?;
        let qty = slice.quantity;

        // Apply quantity rounding if configured
        let qty = if let Some(exec_config) = &self.exec_config {
            let rounded = exec_config.round_quantity(qty.as_decimal());
            let clamped = exec_config.clamp_quantity(rounded);
            Quantity::new(clamped).unwrap_or(qty)
        } else {
            qty
        };

        if qty.is_zero() {
            return Ok(());
        }

        // Submit order
        let result = match side {
            OrderSide::Buy => ctx.buy(symbol, price, qty).await,
            OrderSide::Sell => ctx.sell(symbol, price, qty).await,
        };

        match result {
            Ok(order_ids) => {
                self.active_orders.extend(order_ids.clone());
                self.metrics.record_order_submitted();

                // Mark slice as executed
                if let Some(slice) = self.slices.get_mut(slice_idx) {
                    slice.mark_executed(ctx.current_time(), order_ids);
                }

                self.progress.complete_slice();
                self.current_slice += 1;

                ctx.log(
                    tracing::Level::INFO,
                    &format!(
                        "TWAP slice {}/{} executed: {} @ {}",
                        self.current_slice,
                        self.slices.len(),
                        qty,
                        price
                    ),
                );
            }
            Err(e) => {
                self.metrics.record_order_rejected();
                ctx.log(
                    tracing::Level::WARN,
                    &format!("TWAP slice {} failed: {}", self.current_slice, e),
                );
            }
        }

        Ok(())
    }

    /// Checks if it's time to execute the next slice.
    fn should_execute_slice(&self) -> bool {
        if self.complete || !self.channel_ready {
            return false;
        }

        let slice_idx = self.current_slice as usize;
        if slice_idx >= self.slices.len() {
            return false;
        }

        let slice = &self.slices[slice_idx];
        if slice.executed {
            return false;
        }

        let current_time = self
            .ctx
            .as_ref()
            .map(|c| c.current_time())
            .unwrap_or_else(Timestamp::now);

        current_time.as_millis() >= slice.scheduled_time.as_millis()
    }

    /// Cancels all active orders.
    async fn cancel_active_orders(&mut self) {
        if let Some(ctx) = &self.ctx {
            for order_id in self.active_orders.drain(..) {
                if let Ok(canceled) = ctx.cancel(&order_id).await {
                    if canceled {
                        self.metrics.record_order_canceled();
                    }
                }
            }
        }
    }
}

#[async_trait]
impl ExecuteUnit for TwapExecutor {
    fn name(&self) -> &str {
        "twap"
    }

    fn factory_name(&self) -> &str {
        "default"
    }

    async fn init(
        &mut self,
        ctx: Arc<dyn ExecuteContext>,
        symbol: &Symbol,
        config: &ExecuteConfig,
    ) {
        self.ctx = Some(ctx.clone());
        self.symbol = Some(symbol.clone());
        self.exec_config = Some(config.clone());
        self.start_position = ctx.get_position(symbol);
        self.channel_ready = true;
    }

    async fn set_position(&mut self, target: Quantity) {
        let symbol = match &self.symbol {
            Some(s) => s.clone(),
            None => return,
        };

        let ctx = match &self.ctx {
            Some(c) => c.clone(),
            None => return,
        };

        // Calculate the quantity to execute
        let current_position = ctx.get_position(&symbol);
        let diff = target - current_position;

        if diff.is_zero() {
            self.complete = true;
            return;
        }

        // Determine side
        self.side = Some(if diff.is_positive() {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        });

        // Apply multiplier
        let qty_to_execute = if let Some(exec_config) = &self.exec_config {
            let scaled = diff.as_decimal().abs() * exec_config.multiplier;
            Quantity::new(scaled).unwrap_or(diff.abs())
        } else {
            diff.abs()
        };

        self.target_qty = qty_to_execute;
        self.complete = false;

        // Store reference price
        self.reference_price = self.last_tick.as_ref().and_then(|t| Some(t.price));

        // Calculate slices
        self.calculate_slices(qty_to_execute);

        ctx.log(
            tracing::Level::INFO,
            &format!(
                "TWAP execution started: {} {} in {} slices over {:?}",
                qty_to_execute,
                self.side.map(|s| s.to_string()).unwrap_or_default(),
                self.slices.len(),
                Duration::from_millis(
                    (self.config.interval.as_millis() as u64) * (self.slices.len() as u64)
                )
            ),
        );
    }

    async fn on_tick(&mut self, tick: &TickData) {
        // Update last tick
        if self.symbol.as_ref() == Some(&tick.symbol) {
            self.last_tick = Some(tick.clone());
        }

        // Check if we should execute next slice
        if self.should_execute_slice() {
            // Cancel previous unfilled orders if configured
            if self.config.cancel_before_next && !self.active_orders.is_empty() {
                self.cancel_active_orders().await;
            }

            let _ = self.execute_slice().await;
        }

        // Check if execution is complete
        if self.current_slice as usize >= self.slices.len() && self.active_orders.is_empty() {
            self.complete = true;

            // Calculate final slippage
            if let Some(ref_price) = self.reference_price {
                self.metrics.calculate_slippage(ref_price);
            }

            if let Some(ctx) = &self.ctx {
                ctx.log(
                    tracing::Level::INFO,
                    &format!(
                        "TWAP execution complete: executed {}, slippage {} bps",
                        self.metrics.total_executed, self.metrics.slippage_bps
                    ),
                );
            }
        }
    }

    async fn on_trade(&mut self, trade: &Trade) {
        // Check if this trade is for one of our orders
        if !self.active_orders.contains(&trade.order_id) {
            return;
        }

        // Record the fill
        self.metrics.record_fill(trade.quantity, trade.price);
        self.progress.record_fill(trade.quantity);

        if let Some(ctx) = &self.ctx {
            ctx.log(
                tracing::Level::DEBUG,
                &format!(
                    "TWAP fill: {} @ {}, progress: {:.1}%",
                    trade.quantity,
                    trade.price,
                    self.progress.progress_pct * Decimal::from(100)
                ),
            );
        }
    }

    async fn on_order(&mut self, order: &Order) {
        // Check if this is one of our orders
        if !self.active_orders.contains(&order.order_id) {
            return;
        }

        // Remove from active if completed
        if order.status.is_final() {
            self.active_orders.retain(|id| id != &order.order_id);

            match order.status {
                OrderStatus::Filled => {
                    // Already handled in on_trade
                }
                OrderStatus::Cancelled => {
                    self.metrics.record_order_canceled();
                }
                _ => {}
            }
        }

        // Update pending quantity
        let pending: Quantity = self
            .active_orders
            .iter()
            .filter_map(|_| {
                // In a real implementation, we'd track order quantities
                None::<Quantity>
            })
            .fold(Quantity::ZERO, |acc, q| acc + q);
        self.progress.set_pending(pending);
    }

    async fn on_channel_ready(&mut self) {
        self.channel_ready = true;
        if let Some(ctx) = &self.ctx {
            ctx.log(tracing::Level::INFO, "TWAP: Trading channel ready");
        }
    }

    async fn on_channel_lost(&mut self) {
        self.channel_ready = false;
        if let Some(ctx) = &self.ctx {
            ctx.log(tracing::Level::WARN, "TWAP: Trading channel lost");
        }
    }

    fn progress(&self) -> ExecutionProgress {
        self.progress.clone()
    }

    fn metrics(&self) -> ExecutionMetrics {
        self.metrics.clone()
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    async fn cancel(&mut self) {
        self.cancel_active_orders().await;
        self.complete = true;

        if let Some(ctx) = &self.ctx {
            ctx.log(tracing::Level::INFO, "TWAP execution canceled");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_twap_config_default() {
        let config = TwapConfig::default();
        assert_eq!(config.slices, 10);
        assert_eq!(config.interval, Duration::from_secs(60));
        assert!(!config.aggressive);
    }

    #[test]
    fn test_twap_config_builder() {
        let config = TwapConfig::new(20, Duration::from_secs(30))
            .with_aggressive(true)
            .with_randomize_timing(true)
            .with_price_offset(2);

        assert_eq!(config.slices, 20);
        assert_eq!(config.interval, Duration::from_secs(30));
        assert!(config.aggressive);
        assert!(config.randomize_timing);
        assert_eq!(config.price_offset_ticks, 2);
    }

    #[test]
    fn test_twap_executor_new() {
        let config = TwapConfig::default();
        let executor = TwapExecutor::new(config);

        assert_eq!(executor.name(), "twap");
        assert_eq!(executor.factory_name(), "default");
        assert!(executor.is_complete());
    }

    #[test]
    fn test_twap_calculate_slices() {
        let config = TwapConfig::new(5, Duration::from_secs(60));
        let mut executor = TwapExecutor::new(config);

        let total_qty = Quantity::new(dec!(100)).unwrap();
        executor.calculate_slices(total_qty);

        assert_eq!(executor.slices.len(), 5);

        // Each slice should be 20
        for slice in &executor.slices {
            assert_eq!(slice.quantity.as_decimal(), dec!(20));
        }
    }

    #[test]
    fn test_twap_calculate_slices_with_min_size() {
        let config = TwapConfig::new(100, Duration::from_secs(1)).with_min_slice_size(dec!(5));
        let mut executor = TwapExecutor::new(config);

        // 100 qty / 100 slices = 1 per slice, but min is 5
        let total_qty = Quantity::new(dec!(100)).unwrap();
        executor.calculate_slices(total_qty);

        // All slices should be skipped because 1 < 5
        assert!(executor.slices.is_empty());
    }

    #[test]
    fn test_execution_progress_tracking() {
        let config = TwapConfig::new(4, Duration::from_secs(60));
        let mut executor = TwapExecutor::new(config);

        let total_qty = Quantity::new(dec!(100)).unwrap();
        executor.calculate_slices(total_qty);

        assert_eq!(executor.progress.total_slices, 4);
        assert_eq!(executor.progress.slices_completed, 0);
        assert_eq!(executor.progress.progress_pct, Decimal::ZERO);
    }
}
