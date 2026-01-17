//! MinImpact (Minimum Market Impact) execution unit.
//!
//! Executes orders by minimizing market impact through adaptive
//! order sizing based on order book depth and market conditions.

#![allow(clippy::needless_pass_by_ref_mut)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::if_not_else)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unnecessary_literal_bound)]
#![allow(clippy::or_fun_call)]

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

/// MinImpact execution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinImpactConfig {
    /// Maximum percentage of order book depth to consume per order.
    #[serde(default = "default_max_depth_pct")]
    pub max_depth_pct: Decimal,

    /// Maximum spread tolerance in basis points.
    #[serde(default = "default_max_spread_bps")]
    pub max_spread_bps: Decimal,

    /// Minimum time between orders.
    #[serde(default = "default_min_interval")]
    #[serde(with = "humantime_serde")]
    pub min_interval: Duration,

    /// Maximum time to complete execution.
    #[serde(default = "default_max_duration")]
    #[serde(with = "humantime_serde")]
    pub max_duration: Duration,

    /// Whether to use iceberg orders (hidden quantity).
    #[serde(default)]
    pub use_iceberg: bool,

    /// Visible quantity ratio for iceberg orders.
    #[serde(default = "default_iceberg_ratio")]
    pub iceberg_visible_ratio: Decimal,

    /// Price improvement threshold in ticks.
    #[serde(default)]
    pub price_improvement_ticks: i32,

    /// Whether to pause on high volatility.
    #[serde(default = "default_true")]
    pub pause_on_volatility: bool,

    /// Volatility threshold (standard deviations).
    #[serde(default = "default_volatility_threshold")]
    pub volatility_threshold: Decimal,
}

fn default_max_depth_pct() -> Decimal {
    Decimal::new(5, 2) // 5%
}

fn default_max_spread_bps() -> Decimal {
    Decimal::from(20) // 20 bps
}

fn default_min_interval() -> Duration {
    Duration::from_secs(5)
}

fn default_max_duration() -> Duration {
    Duration::from_secs(7200) // 2 hours
}

fn default_iceberg_ratio() -> Decimal {
    Decimal::new(20, 2) // 20% visible
}

fn default_true() -> bool {
    true
}

fn default_volatility_threshold() -> Decimal {
    Decimal::from(2) // 2 standard deviations
}

impl Default for MinImpactConfig {
    fn default() -> Self {
        Self {
            max_depth_pct: default_max_depth_pct(),
            max_spread_bps: default_max_spread_bps(),
            min_interval: default_min_interval(),
            max_duration: default_max_duration(),
            use_iceberg: false,
            iceberg_visible_ratio: default_iceberg_ratio(),
            price_improvement_ticks: 0,
            pause_on_volatility: true,
            volatility_threshold: default_volatility_threshold(),
        }
    }
}

impl MinImpactConfig {
    /// Creates a new MinImpact configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets maximum depth percentage.
    #[must_use]
    pub fn with_max_depth_pct(mut self, pct: Decimal) -> Self {
        self.max_depth_pct = pct;
        self
    }

    /// Sets maximum spread tolerance.
    #[must_use]
    pub fn with_max_spread_bps(mut self, bps: Decimal) -> Self {
        self.max_spread_bps = bps;
        self
    }

    /// Sets minimum interval between orders.
    #[must_use]
    pub fn with_min_interval(mut self, interval: Duration) -> Self {
        self.min_interval = interval;
        self
    }

    /// Enables iceberg orders.
    #[must_use]
    pub fn with_iceberg(mut self, visible_ratio: Decimal) -> Self {
        self.use_iceberg = true;
        self.iceberg_visible_ratio = visible_ratio;
        self
    }

    /// Sets price improvement threshold.
    #[must_use]
    pub fn with_price_improvement(mut self, ticks: i32) -> Self {
        self.price_improvement_ticks = ticks;
        self
    }
}

/// Market condition assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketCondition {
    /// Normal conditions - proceed with execution.
    Normal,
    /// Wide spread - wait for better conditions.
    WideSpread,
    /// High volatility - pause execution.
    HighVolatility,
    /// Thin book - reduce order size.
    ThinBook,
}

/// MinImpact execution unit.
///
/// Executes orders by minimizing market impact through:
/// - Adaptive order sizing based on order book depth
/// - Spread monitoring to avoid crossing wide spreads
/// - Volatility detection to pause during turbulent periods
/// - Optional iceberg orders to hide true size
///
/// # Algorithm
///
/// 1. Assess market conditions (spread, depth, volatility)
/// 2. Calculate optimal order size based on book depth
/// 3. Wait for favorable conditions if necessary
/// 4. Submit order at best price with optional improvement
/// 5. Monitor fills and adjust strategy
pub struct MinImpactExecutor {
    /// Configuration.
    config: MinImpactConfig,
    /// Execution context.
    ctx: Option<Arc<dyn ExecuteContext>>,
    /// Symbol being executed.
    symbol: Option<Symbol>,
    /// Execution configuration.
    exec_config: Option<ExecuteConfig>,
    /// Target quantity.
    target_qty: Quantity,
    /// Order side.
    side: Option<OrderSide>,
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
    /// Last order submission time.
    last_order_time: Option<Timestamp>,
    /// Execution start time.
    start_time: Option<Timestamp>,
    /// Recent price observations for volatility.
    recent_prices: Vec<Decimal>,
    /// Current market condition.
    market_condition: MarketCondition,
    /// Slices for tracking.
    slices: Vec<SliceOrder>,
    /// Current slice index.
    current_slice: u32,
}

impl MinImpactExecutor {
    /// Creates a new MinImpact executor.
    #[must_use]
    pub fn new(config: MinImpactConfig) -> Self {
        Self {
            config,
            ctx: None,
            symbol: None,
            exec_config: None,
            target_qty: Quantity::ZERO,
            side: None,
            progress: ExecutionProgress::default(),
            metrics: ExecutionMetrics::default(),
            reference_price: None,
            active_orders: Vec::new(),
            complete: true,
            channel_ready: true,
            last_tick: None,
            last_order_time: None,
            start_time: None,
            recent_prices: Vec::with_capacity(100),
            market_condition: MarketCondition::Normal,
            slices: Vec::new(),
            current_slice: 0,
        }
    }

    /// Assesses current market conditions.
    fn assess_market(&mut self) -> MarketCondition {
        let tick = match &self.last_tick {
            Some(t) => t,
            None => return MarketCondition::Normal,
        };

        // Check spread
        if let Some(spread) = tick.spread() {
            if let Some(mid) = tick.mid_price() {
                let spread_bps = (spread.as_decimal() / mid.as_decimal()) * Decimal::from(10000);
                if spread_bps > self.config.max_spread_bps {
                    return MarketCondition::WideSpread;
                }
            }
        }

        // Check volatility
        if self.config.pause_on_volatility && self.recent_prices.len() >= 20 {
            let volatility = self.calculate_volatility();
            if volatility > self.config.volatility_threshold {
                return MarketCondition::HighVolatility;
            }
        }

        // Check book depth
        let depth = self.calculate_book_depth(tick);
        if depth < self.progress.remaining_qty.as_decimal() * Decimal::new(5, 1) {
            return MarketCondition::ThinBook;
        }

        MarketCondition::Normal
    }

    /// Calculates recent price volatility.
    fn calculate_volatility(&self) -> Decimal {
        if self.recent_prices.len() < 2 {
            return Decimal::ZERO;
        }

        // Calculate returns
        let returns: Vec<Decimal> = self
            .recent_prices
            .windows(2)
            .filter_map(|w| {
                if w[0] != Decimal::ZERO {
                    Some((w[1] - w[0]) / w[0])
                } else {
                    None
                }
            })
            .collect();

        if returns.is_empty() {
            return Decimal::ZERO;
        }

        // Calculate mean
        let mean: Decimal = returns.iter().sum::<Decimal>() / Decimal::from(returns.len());

        // Calculate variance
        let variance: Decimal = returns
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / Decimal::from(returns.len());

        // Return standard deviation (approximation)
        // Note: Decimal doesn't have sqrt, so we use a simple approximation
        variance * Decimal::from(100) // Scale for comparison
    }

    /// Calculates available book depth.
    fn calculate_book_depth(&self, tick: &TickData) -> Decimal {
        let side = match self.side {
            Some(s) => s,
            None => return Decimal::ZERO,
        };

        match side {
            OrderSide::Buy => tick
                .ask_quantities
                .as_ref()
                .map_or(Decimal::ZERO, |v| v.iter().map(|q| q.as_decimal()).sum()),
            OrderSide::Sell => tick
                .bid_quantities
                .as_ref()
                .map_or(Decimal::ZERO, |v| v.iter().map(|q| q.as_decimal()).sum()),
        }
    }

    /// Calculates optimal order size based on market conditions.
    fn calculate_order_size(&self) -> Quantity {
        let tick = match &self.last_tick {
            Some(t) => t,
            None => return Quantity::ZERO,
        };

        let remaining = self.progress.remaining_qty;
        if remaining.is_zero() {
            return Quantity::ZERO;
        }

        // Calculate based on book depth
        let depth = self.calculate_book_depth(tick);
        let max_from_depth = depth * self.config.max_depth_pct;

        // Use smaller of remaining and depth-based limit
        let order_size = remaining.as_decimal().min(max_from_depth);

        // Apply execution config limits
        let order_size = if let Some(exec_config) = &self.exec_config {
            let rounded = exec_config.round_quantity(order_size);
            exec_config.clamp_quantity(rounded)
        } else {
            order_size
        };

        Quantity::new(order_size).unwrap_or(Quantity::ZERO)
    }

    /// Gets the price for the order.
    fn get_order_price(&self) -> Option<Price> {
        let tick = self.last_tick.as_ref()?;
        let side = self.side?;

        // Start with best price on our side
        let base_price = match side {
            OrderSide::Buy => tick.best_bid()?,
            OrderSide::Sell => tick.best_ask()?,
        };

        // Apply price improvement
        if self.config.price_improvement_ticks != 0 {
            if let Some(exec_config) = &self.exec_config {
                if let Some(tick_size) = exec_config.tick_size {
                    let improvement =
                        tick_size * Decimal::from(self.config.price_improvement_ticks);
                    let improved = match side {
                        OrderSide::Buy => base_price.as_decimal() + improvement,
                        OrderSide::Sell => base_price.as_decimal() - improvement,
                    };
                    return Price::new(improved).ok();
                }
            }
        }

        Some(base_price)
    }

    /// Checks if enough time has passed since last order.
    fn can_submit_order(&self) -> bool {
        let last_time = match self.last_order_time {
            Some(t) => t,
            None => return true,
        };

        let current = self
            .ctx
            .as_ref()
            .map(|c| c.current_time())
            .unwrap_or_else(Timestamp::now);

        let elapsed_ms = current.as_millis().saturating_sub(last_time.as_millis());
        elapsed_ms >= self.config.min_interval.as_millis() as i64
    }

    /// Checks if execution has exceeded max duration.
    fn is_timed_out(&self) -> bool {
        let start = match self.start_time {
            Some(t) => t,
            None => return false,
        };

        let current = self
            .ctx
            .as_ref()
            .map(|c| c.current_time())
            .unwrap_or_else(Timestamp::now);

        let elapsed_ms = current.as_millis().saturating_sub(start.as_millis());
        elapsed_ms >= self.config.max_duration.as_millis() as i64
    }

    /// Executes an order.
    async fn execute_order(&mut self) -> Result<(), String> {
        let ctx = self.ctx.as_ref().ok_or("Context not initialized")?;
        let symbol = self.symbol.as_ref().ok_or("Symbol not set")?;
        let side = self.side.ok_or("Side not determined")?;

        let qty = self.calculate_order_size();
        if qty.is_zero() {
            return Ok(());
        }

        let price = self.get_order_price().ok_or("Cannot determine price")?;

        // Submit order
        let result = match side {
            OrderSide::Buy => ctx.buy(symbol, price, qty).await,
            OrderSide::Sell => ctx.sell(symbol, price, qty).await,
        };

        match result {
            Ok(order_ids) => {
                self.active_orders.extend(order_ids.clone());
                self.metrics.record_order_submitted();
                self.last_order_time = Some(ctx.current_time());

                // Track as slice
                let slice = SliceOrder::new(self.current_slice, qty, ctx.current_time());
                self.slices.push(slice);
                self.current_slice += 1;

                ctx.log(
                    tracing::Level::INFO,
                    &format!(
                        "MinImpact order: {} @ {}, remaining: {}",
                        qty, price, self.progress.remaining_qty
                    ),
                );
            }
            Err(e) => {
                self.metrics.record_order_rejected();
                ctx.log(
                    tracing::Level::WARN,
                    &format!("MinImpact order failed: {}", e),
                );
            }
        }

        Ok(())
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
impl ExecuteUnit for MinImpactExecutor {
    fn name(&self) -> &str {
        "min_impact"
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
        let ctx = match &self.ctx {
            Some(c) => c,
            None => return,
        };

        let symbol = match &self.symbol {
            Some(s) => s,
            None => return,
        };

        let current_position = ctx.get_position(symbol);
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
        self.start_time = Some(ctx.current_time());
        self.reference_price = self.last_tick.as_ref().map(|t| t.price);

        // Initialize progress (unknown number of slices)
        self.progress = ExecutionProgress::new(qty_to_execute, 0);
        self.progress.start_time = self.start_time;

        ctx.log(
            tracing::Level::INFO,
            &format!(
                "MinImpact execution started: {} {}, max duration: {:?}",
                qty_to_execute,
                self.side.map(|s| s.to_string()).unwrap_or_default(),
                self.config.max_duration
            ),
        );
    }

    async fn on_tick(&mut self, tick: &TickData) {
        if self.symbol.as_ref() == Some(&tick.symbol) {
            // Track prices for volatility
            self.recent_prices.push(tick.price.as_decimal());
            if self.recent_prices.len() > 100 {
                self.recent_prices.remove(0);
            }

            self.last_tick = Some(tick.clone());
        }

        if self.complete || !self.channel_ready {
            return;
        }

        // Check timeout
        if self.is_timed_out() {
            if let Some(ctx) = &self.ctx {
                ctx.log(
                    tracing::Level::WARN,
                    "MinImpact execution timed out, completing with remaining quantity",
                );
            }
            self.complete = true;
            return;
        }

        // Assess market conditions
        self.market_condition = self.assess_market();

        // Only proceed if conditions are favorable and enough time has passed
        if self.market_condition == MarketCondition::Normal && self.can_submit_order() {
            let _ = self.execute_order().await;
        } else if self.market_condition != MarketCondition::Normal {
            if let Some(ctx) = &self.ctx {
                ctx.log(
                    tracing::Level::DEBUG,
                    &format!("MinImpact waiting: {:?}", self.market_condition),
                );
            }
        }

        // Check completion
        if self.progress.remaining_qty.is_zero() && self.active_orders.is_empty() {
            self.complete = true;

            if let Some(ref_price) = self.reference_price {
                self.metrics.calculate_slippage(ref_price);
            }

            if let Some(ctx) = &self.ctx {
                ctx.log(
                    tracing::Level::INFO,
                    &format!(
                        "MinImpact execution complete: executed {}, slippage {} bps",
                        self.metrics.total_executed, self.metrics.slippage_bps
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
                    "MinImpact fill: {} @ {}, remaining: {}",
                    trade.quantity, trade.price, self.progress.remaining_qty
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
            ctx.log(tracing::Level::INFO, "MinImpact: Trading channel ready");
        }
    }

    async fn on_channel_lost(&mut self) {
        self.channel_ready = false;
        if let Some(ctx) = &self.ctx {
            ctx.log(tracing::Level::WARN, "MinImpact: Trading channel lost");
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
            ctx.log(tracing::Level::INFO, "MinImpact execution canceled");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_min_impact_config_default() {
        let config = MinImpactConfig::default();
        assert_eq!(config.max_depth_pct, dec!(0.05));
        assert_eq!(config.max_spread_bps, dec!(20));
        assert!(!config.use_iceberg);
    }

    #[test]
    fn test_min_impact_config_builder() {
        let config = MinImpactConfig::new()
            .with_max_depth_pct(dec!(0.10))
            .with_max_spread_bps(dec!(30))
            .with_iceberg(dec!(0.25))
            .with_price_improvement(2);

        assert_eq!(config.max_depth_pct, dec!(0.10));
        assert_eq!(config.max_spread_bps, dec!(30));
        assert!(config.use_iceberg);
        assert_eq!(config.iceberg_visible_ratio, dec!(0.25));
        assert_eq!(config.price_improvement_ticks, 2);
    }

    #[test]
    fn test_min_impact_executor_new() {
        let config = MinImpactConfig::default();
        let executor = MinImpactExecutor::new(config);

        assert_eq!(executor.name(), "min_impact");
        assert_eq!(executor.factory_name(), "default");
        assert!(executor.is_complete());
    }

    #[test]
    fn test_market_condition_enum() {
        assert_eq!(MarketCondition::Normal, MarketCondition::Normal);
        assert_ne!(MarketCondition::Normal, MarketCondition::WideSpread);
    }
}
