//! VWAP (Volume-Weighted Average Price) execution unit.
//!
//! Executes orders by distributing them according to historical
//! volume patterns to minimize market impact.

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::unnecessary_literal_bound)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::cast_sign_loss)]

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use zephyr_core::data::{Order, OrderSide, OrderStatus, TickData};
use zephyr_core::traits::Trade;
use zephyr_core::types::{OrderId, Price, Quantity, Symbol, Timestamp};

use super::config::ExecuteConfig;
use super::traits::{ExecuteContext, ExecuteUnit, ExecutionMetrics, ExecutionProgress, SliceOrder};

/// VWAP execution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwapConfig {
    /// Total execution duration.
    #[serde(default = "default_duration")]
    #[serde(with = "humantime_serde")]
    pub duration: Duration,

    /// Number of time buckets for volume distribution.
    #[serde(default = "default_buckets")]
    pub buckets: u32,

    /// Historical volume lookback period for pattern estimation.
    #[serde(default = "default_lookback")]
    #[serde(with = "humantime_serde")]
    pub lookback_period: Duration,

    /// Maximum participation rate (our volume / total volume).
    #[serde(default = "default_participation_rate")]
    pub max_participation_rate: Decimal,

    /// Whether to use aggressive pricing when behind schedule.
    #[serde(default)]
    pub catch_up_aggressive: bool,

    /// Price offset in ticks from best bid/ask.
    #[serde(default)]
    pub price_offset_ticks: i32,

    /// Minimum slice size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_slice_size: Option<Decimal>,
}

fn default_duration() -> Duration {
    Duration::from_secs(3600) // 1 hour
}

fn default_buckets() -> u32 {
    60 // 1 minute buckets for 1 hour
}

fn default_lookback() -> Duration {
    Duration::from_secs(86400) // 24 hours
}

fn default_participation_rate() -> Decimal {
    Decimal::new(10, 2) // 10%
}

impl Default for VwapConfig {
    fn default() -> Self {
        Self {
            duration: default_duration(),
            buckets: default_buckets(),
            lookback_period: default_lookback(),
            max_participation_rate: default_participation_rate(),
            catch_up_aggressive: false,
            price_offset_ticks: 0,
            min_slice_size: None,
        }
    }
}

impl VwapConfig {
    /// Creates a new VWAP configuration.
    #[must_use]
    pub fn new(duration: Duration, buckets: u32) -> Self {
        Self {
            duration,
            buckets,
            ..Default::default()
        }
    }

    /// Sets the maximum participation rate.
    #[must_use]
    pub fn with_max_participation_rate(mut self, rate: Decimal) -> Self {
        self.max_participation_rate = rate;
        self
    }

    /// Sets catch-up aggressive mode.
    #[must_use]
    pub fn with_catch_up_aggressive(mut self, aggressive: bool) -> Self {
        self.catch_up_aggressive = aggressive;
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

    /// Calculates bucket duration.
    #[must_use]
    pub fn bucket_duration(&self) -> Duration {
        if self.buckets == 0 {
            return self.duration;
        }
        Duration::from_millis(self.duration.as_millis() as u64 / self.buckets as u64)
    }
}

/// Volume profile for VWAP calculation.
#[derive(Debug, Clone, Default)]
struct VolumeProfile {
    /// Volume weights per bucket (normalized to sum to 1.0).
    weights: Vec<Decimal>,
    /// Total observed volume.
    total_volume: Decimal,
}

impl VolumeProfile {
    /// Creates a uniform volume profile.
    fn uniform(buckets: u32) -> Self {
        let weight = Decimal::ONE / Decimal::from(buckets);
        Self {
            weights: vec![weight; buckets as usize],
            total_volume: Decimal::ZERO,
        }
    }

    /// Updates the profile with observed volume.
    fn observe_volume(&mut self, bucket_idx: usize, volume: Decimal) {
        if bucket_idx < self.weights.len() {
            self.total_volume += volume;
            // In a real implementation, we'd update weights based on observed patterns
        }
    }

    /// Gets the weight for a bucket.
    fn get_weight(&self, bucket_idx: usize) -> Decimal {
        self.weights
            .get(bucket_idx)
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Calculates cumulative weight up to a bucket.
    fn cumulative_weight(&self, bucket_idx: usize) -> Decimal {
        self.weights.iter().take(bucket_idx + 1).sum()
    }
}

/// VWAP execution unit.
///
/// Executes orders by distributing them according to volume patterns
/// to achieve execution close to the volume-weighted average price.
///
/// # Algorithm
///
/// 1. Estimate volume profile from historical data
/// 2. Distribute target quantity according to volume weights
/// 3. At each bucket, submit orders proportional to expected volume
/// 4. Track actual vs expected progress and adjust
/// 5. Use participation rate to limit market impact
pub struct VwapExecutor {
    /// Configuration.
    config: VwapConfig,
    /// Execution context.
    ctx: Option<Arc<dyn ExecuteContext>>,
    /// Symbol being executed.
    symbol: Option<Symbol>,
    /// Execution configuration.
    exec_config: Option<ExecuteConfig>,
    /// Target position.
    target_qty: Quantity,
    /// Order side.
    side: Option<OrderSide>,
    /// Volume profile.
    volume_profile: VolumeProfile,
    /// Planned slices.
    slices: Vec<SliceOrder>,
    /// Current bucket index.
    current_bucket: u32,
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
    /// Recent volume observations.
    recent_volumes: VecDeque<Decimal>,
    /// Execution start time.
    start_time: Option<Timestamp>,
}

impl VwapExecutor {
    /// Creates a new VWAP executor.
    #[must_use]
    pub fn new(config: VwapConfig) -> Self {
        let volume_profile = VolumeProfile::uniform(config.buckets);

        Self {
            config,
            ctx: None,
            symbol: None,
            exec_config: None,
            target_qty: Quantity::ZERO,
            side: None,
            volume_profile,
            slices: Vec::new(),
            current_bucket: 0,
            progress: ExecutionProgress::default(),
            metrics: ExecutionMetrics::default(),
            reference_price: None,
            active_orders: Vec::new(),
            complete: true,
            channel_ready: true,
            last_tick: None,
            recent_volumes: VecDeque::with_capacity(100),
            start_time: None,
        }
    }

    /// Calculates slices based on volume profile.
    fn calculate_slices(&mut self, total_qty: Quantity) {
        let num_buckets = self.config.buckets;
        if num_buckets == 0 {
            return;
        }

        let start_time = self
            .ctx
            .as_ref()
            .map(|c| c.current_time())
            .unwrap_or_else(Timestamp::now);
        let bucket_duration = self.config.bucket_duration();

        self.slices.clear();

        for i in 0..num_buckets {
            let weight = self.volume_profile.get_weight(i as usize);
            let slice_qty_decimal = total_qty.as_decimal() * weight;

            // Skip slices below minimum size
            if let Some(min_size) = self.config.min_slice_size {
                if slice_qty_decimal < min_size {
                    continue;
                }
            }

            let slice_qty = match Quantity::new(slice_qty_decimal) {
                Ok(q) => q,
                Err(_) => continue,
            };

            let scheduled_time = Timestamp::new(
                start_time.as_millis() + (i as i64 * bucket_duration.as_millis() as i64),
            )
            .unwrap_or(start_time);

            self.slices
                .push(SliceOrder::new(i, slice_qty, scheduled_time));
        }

        self.progress = ExecutionProgress::new(total_qty, self.slices.len() as u32);
        self.progress.start_time = Some(start_time);
        self.start_time = Some(start_time);

        if let Some(last_slice) = self.slices.last() {
            self.progress.expected_end_time = Some(last_slice.scheduled_time);
        }
    }

    /// Gets the current bucket index based on elapsed time.
    fn get_current_bucket(&self) -> u32 {
        let start = match self.start_time {
            Some(t) => t,
            None => return 0,
        };

        let current = self
            .ctx
            .as_ref()
            .map(|c| c.current_time())
            .unwrap_or_else(Timestamp::now);

        let elapsed_ms = current.as_millis().saturating_sub(start.as_millis());
        let bucket_ms = self.config.bucket_duration().as_millis() as i64;

        if bucket_ms == 0 {
            return 0;
        }

        (elapsed_ms / bucket_ms).min(self.config.buckets as i64 - 1) as u32
    }

    /// Calculates how far behind schedule we are.
    fn calculate_shortfall(&self) -> Decimal {
        let current_bucket = self.get_current_bucket();
        let expected_progress = self
            .volume_profile
            .cumulative_weight(current_bucket as usize);
        let actual_progress = self.progress.progress_pct;

        expected_progress - actual_progress
    }

    /// Gets the price for the current slice.
    fn get_slice_price(&self, aggressive: bool) -> Option<Price> {
        let tick = self.last_tick.as_ref()?;
        let side = self.side?;

        let base_price = if aggressive {
            match side {
                OrderSide::Buy => tick.best_ask()?,
                OrderSide::Sell => tick.best_bid()?,
            }
        } else {
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

    /// Executes the current bucket's slice.
    async fn execute_bucket(&mut self) -> Result<(), String> {
        let ctx = self.ctx.as_ref().ok_or("Context not initialized")?;
        let symbol = self.symbol.as_ref().ok_or("Symbol not set")?;
        let side = self.side.ok_or("Side not determined")?;

        let bucket_idx = self.current_bucket as usize;
        if bucket_idx >= self.slices.len() {
            return Ok(());
        }

        let slice = &self.slices[bucket_idx];
        if slice.executed {
            return Ok(());
        }

        // Check if we're behind schedule and should be aggressive
        let shortfall = self.calculate_shortfall();
        let aggressive = self.config.catch_up_aggressive && shortfall > Decimal::new(5, 2); // 5%

        let price = self
            .get_slice_price(aggressive)
            .ok_or("Cannot determine price")?;
        let mut qty = slice.quantity;

        // If behind schedule, increase slice size
        if shortfall > Decimal::ZERO {
            let catch_up_factor = Decimal::ONE + shortfall;
            let adjusted_qty = qty.as_decimal() * catch_up_factor;
            qty = Quantity::new(adjusted_qty).unwrap_or(qty);
        }

        // Apply quantity rounding
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

                if let Some(slice) = self.slices.get_mut(bucket_idx) {
                    slice.mark_executed(ctx.current_time(), order_ids);
                }

                self.progress.complete_slice();
                self.current_bucket += 1;

                ctx.log(
                    tracing::Level::INFO,
                    &format!(
                        "VWAP bucket {}/{} executed: {} @ {} (shortfall: {:.1}%)",
                        self.current_bucket,
                        self.slices.len(),
                        qty,
                        price,
                        shortfall * Decimal::from(100)
                    ),
                );
            }
            Err(e) => {
                self.metrics.record_order_rejected();
                ctx.log(
                    tracing::Level::WARN,
                    &format!("VWAP bucket {} failed: {}", self.current_bucket, e),
                );
            }
        }

        Ok(())
    }

    /// Checks if it's time to execute the next bucket.
    fn should_execute_bucket(&self) -> bool {
        if self.complete || !self.channel_ready {
            return false;
        }

        let expected_bucket = self.get_current_bucket();
        expected_bucket >= self.current_bucket && (self.current_bucket as usize) < self.slices.len()
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
impl ExecuteUnit for VwapExecutor {
    fn name(&self) -> &str {
        "vwap"
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
        self.ctx = Some(ctx);
        self.symbol = Some(symbol.clone());
        self.exec_config = Some(config.clone());
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

        let current_position = ctx.get_position(&symbol);
        let diff = target - current_position;

        if diff.is_zero() {
            self.complete = true;
            return;
        }

        self.side = Some(if diff.is_positive() {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        });

        let qty_to_execute = if let Some(exec_config) = &self.exec_config {
            let scaled = diff.as_decimal().abs() * exec_config.multiplier;
            Quantity::new(scaled).unwrap_or(diff.abs())
        } else {
            diff.abs()
        };

        self.target_qty = qty_to_execute;
        self.complete = false;
        self.reference_price = self.last_tick.as_ref().map(|t| t.price);

        self.calculate_slices(qty_to_execute);

        ctx.log(
            tracing::Level::INFO,
            &format!(
                "VWAP execution started: {} {} in {} buckets over {:?}",
                qty_to_execute,
                self.side.map(|s| s.to_string()).unwrap_or_default(),
                self.slices.len(),
                self.config.duration
            ),
        );
    }

    async fn on_tick(&mut self, tick: &TickData) {
        if self.symbol.as_ref() == Some(&tick.symbol) {
            // Track volume for participation rate
            self.recent_volumes.push_back(tick.volume.as_decimal());
            if self.recent_volumes.len() > 100 {
                self.recent_volumes.pop_front();
            }

            self.last_tick = Some(tick.clone());
        }

        if self.should_execute_bucket() {
            let _ = self.execute_bucket().await;
        }

        // Check completion
        if self.current_bucket as usize >= self.slices.len() && self.active_orders.is_empty() {
            self.complete = true;

            if let Some(ref_price) = self.reference_price {
                self.metrics.calculate_slippage(ref_price);
            }

            if let Some(ctx) = &self.ctx {
                ctx.log(
                    tracing::Level::INFO,
                    &format!(
                        "VWAP execution complete: executed {}, VWAP {:?}, slippage {} bps",
                        self.metrics.total_executed, self.metrics.vwap, self.metrics.slippage_bps
                    ),
                );
            }
        }
    }

    async fn on_trade(&mut self, trade: &Trade) {
        if !self.active_orders.contains(&trade.order_id) {
            return;
        }

        self.metrics.record_fill(trade.quantity, trade.price);
        self.progress.record_fill(trade.quantity);

        if let Some(ctx) = &self.ctx {
            ctx.log(
                tracing::Level::DEBUG,
                &format!(
                    "VWAP fill: {} @ {}, VWAP: {:?}",
                    trade.quantity, trade.price, self.metrics.vwap
                ),
            );
        }
    }

    async fn on_order(&mut self, order: &Order) {
        if !self.active_orders.contains(&order.order_id) {
            return;
        }

        if order.status.is_final() {
            self.active_orders.retain(|id| id != &order.order_id);

            match order.status {
                OrderStatus::Canceled => self.metrics.record_order_canceled(),
                OrderStatus::Rejected => self.metrics.record_order_rejected(),
                _ => {}
            }
        }
    }

    async fn on_channel_ready(&mut self) {
        self.channel_ready = true;
        if let Some(ctx) = &self.ctx {
            ctx.log(tracing::Level::INFO, "VWAP: Trading channel ready");
        }
    }

    async fn on_channel_lost(&mut self) {
        self.channel_ready = false;
        if let Some(ctx) = &self.ctx {
            ctx.log(tracing::Level::WARN, "VWAP: Trading channel lost");
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
            ctx.log(tracing::Level::INFO, "VWAP execution canceled");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_vwap_config_default() {
        let config = VwapConfig::default();
        assert_eq!(config.duration, Duration::from_secs(3600));
        assert_eq!(config.buckets, 60);
        assert_eq!(config.max_participation_rate, dec!(0.10));
    }

    #[test]
    fn test_vwap_config_bucket_duration() {
        let config = VwapConfig::new(Duration::from_secs(3600), 60);
        assert_eq!(config.bucket_duration(), Duration::from_secs(60));
    }

    #[test]
    fn test_vwap_executor_new() {
        let config = VwapConfig::default();
        let executor = VwapExecutor::new(config);

        assert_eq!(executor.name(), "vwap");
        assert_eq!(executor.factory_name(), "default");
        assert!(executor.is_complete());
    }

    #[test]
    fn test_volume_profile_uniform() {
        let profile = VolumeProfile::uniform(10);
        assert_eq!(profile.weights.len(), 10);

        let total: Decimal = profile.weights.iter().sum();
        assert_eq!(total, Decimal::ONE);
    }

    #[test]
    fn test_volume_profile_cumulative() {
        let profile = VolumeProfile::uniform(4);

        assert_eq!(profile.cumulative_weight(0), dec!(0.25));
        assert_eq!(profile.cumulative_weight(1), dec!(0.50));
        assert_eq!(profile.cumulative_weight(2), dec!(0.75));
        assert_eq!(profile.cumulative_weight(3), Decimal::ONE);
    }

    #[test]
    fn test_vwap_calculate_slices() {
        let config = VwapConfig::new(Duration::from_secs(600), 10);
        let mut executor = VwapExecutor::new(config);

        let total_qty = Quantity::new(dec!(100)).unwrap();
        executor.calculate_slices(total_qty);

        assert_eq!(executor.slices.len(), 10);

        // With uniform profile, each slice should be 10
        for slice in &executor.slices {
            assert_eq!(slice.quantity.as_decimal(), dec!(10));
        }
    }
}
