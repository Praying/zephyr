//! DualThrust Strategy Execution Example
//!
//! This example demonstrates how to run the DualThrust CTA strategy
//! using the Zephyr engine.
//!
//! # Running
//!
//! ```bash
//! cargo run --example run_dual_thrust
//! ```
//!
//! # Architecture
//!
//! The execution flow follows Zephyr's M+1+N architecture:
//! 1. Strategy generates position signals via `set_position()`
//! 2. SignalAggregator collects and merges signals from multiple strategies
//! 3. Execution layer (not shown) converts aggregated positions to orders
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
//! │  DualThrust     │────▶│ SignalAggregator │────▶│ Execution Layer │
//! │  Strategy       │     │ (Position Merge) │     │ (Order Routing) │
//! └─────────────────┘     └──────────────────┘     └─────────────────┘
//!         │                        │
//!         ▼                        ▼
//! ┌─────────────────┐     ┌──────────────────┐
//! │ CtaStrategy     │     │ Aggregated       │
//! │ Context         │     │ Position         │
//! └─────────────────┘     └──────────────────┘
//! ```

use std::sync::Arc;

use rust_decimal_macros::dec;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use zephyr_core::data::{KlineData, KlinePeriod, TickData};
use zephyr_core::types::{Amount, Price, Quantity, Symbol, Timestamp};
use zephyr_engine::{
    CtaStrategyContextImpl, CtaStrategyRunner, DualThrustConfig, DualThrustStrategy,
    SignalAggregator,
};

/// Creates mock historical daily bars for testing.
/// In production, this data would come from a data service.
fn create_historical_bars(symbol: &Symbol, days: usize) -> Vec<KlineData> {
    let base_ts = 1704067200000i64; // 2024-01-01 00:00:00 UTC
    let day_ms = 24 * 60 * 60 * 1000i64;

    (0..days)
        .map(|i| {
            let ts = base_ts + (i as i64) * day_ms;
            // Simulate price movement with some volatility
            let base_price = dec!(40000) + rust_decimal::Decimal::from(i as i64) * dec!(200);
            let volatility = dec!(500);

            KlineData {
                symbol: symbol.clone(),
                timestamp: Timestamp::new_unchecked(ts),
                period: KlinePeriod::Day1,
                open: Price::new(base_price).unwrap(),
                high: Price::new(base_price + volatility).unwrap(),
                low: Price::new(base_price - volatility * dec!(0.6)).unwrap(),
                close: Price::new(base_price + volatility * dec!(0.3)).unwrap(),
                volume: Quantity::new(dec!(1000)).unwrap(),
                turnover: Amount::new(dec!(40000000)).unwrap(),
            }
        })
        .collect()
}

/// Creates a mock tick for testing.
fn create_tick(symbol: &Symbol, price: rust_decimal::Decimal, ts: i64) -> TickData {
    TickData::builder()
        .symbol(symbol.clone())
        .timestamp(Timestamp::new_unchecked(ts))
        .price(Price::new(price).unwrap())
        .volume(Quantity::new(dec!(10)).unwrap())
        .bid_price(Price::new(price - dec!(0.5)).unwrap())
        .bid_quantity(Quantity::new(dec!(100)).unwrap())
        .ask_price(Price::new(price + dec!(0.5)).unwrap())
        .ask_quantity(Quantity::new(dec!(100)).unwrap())
        .build()
        .unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("╔════════════════════════════════════════════════════════════╗");
    info!("║         DualThrust Strategy Execution Example              ║");
    info!("╚════════════════════════════════════════════════════════════╝");

    // ═══════════════════════════════════════════════════════════════════
    // Step 1: Setup - Create trading symbol and configuration
    // ═══════════════════════════════════════════════════════════════════
    let symbol = Symbol::new("BTC-USDT")?;
    info!("\n📊 Trading Symbol: {}", symbol);

    let config = DualThrustConfig {
        lookback_days: 4,       // Look back 4 days for range calculation
        k1: dec!(0.5),          // Upper boundary coefficient
        k2: dec!(0.5),          // Lower boundary coefficient
        position_size: dec!(1), // Trade 1 unit per signal
    };
    info!(
        "⚙️  Strategy Config: lookback={} days, K1={}, K2={}, size={}",
        config.lookback_days, config.k1, config.k2, config.position_size
    );

    // ═══════════════════════════════════════════════════════════════════
    // Step 2: Create the DualThrust strategy instance
    // ═══════════════════════════════════════════════════════════════════
    let strategy = DualThrustStrategy::new("dual_thrust_btc", symbol.clone(), config);

    // ═══════════════════════════════════════════════════════════════════
    // Step 3: Create SignalAggregator
    // This component collects position signals from all strategies
    // and merges them into a single aggregated position per symbol
    // ═══════════════════════════════════════════════════════════════════
    let signal_aggregator = Arc::new(SignalAggregator::new());
    info!("\n🔗 SignalAggregator created (collects signals from strategies)");

    // ═══════════════════════════════════════════════════════════════════
    // Step 4: Create Strategy Context using builder pattern
    // The context provides:
    // - Position management (get_position, set_position)
    // - Market data access (get_bars, get_ticks, get_price)
    // - Signal routing to the aggregator
    // ═══════════════════════════════════════════════════════════════════
    let context = Arc::new(
        CtaStrategyContextImpl::builder()
            .strategy_name("dual_thrust_btc")
            .add_symbol(symbol.clone())
            .signal_aggregator(signal_aggregator.clone())
            .max_kline_cache(1000)
            .max_tick_cache(10000)
            .build()?,
    );
    info!("📦 Strategy Context created");

    // ═══════════════════════════════════════════════════════════════════
    // Step 5: Pre-load historical data
    // In production, this would be loaded from a data service
    // ═══════════════════════════════════════════════════════════════════
    let historical_bars = create_historical_bars(&symbol, 10);
    for bar in &historical_bars {
        context.add_kline(bar.clone());
    }
    info!("📈 Loaded {} historical daily bars", historical_bars.len());

    // ═══════════════════════════════════════════════════════════════════
    // Step 6: Create and start the Strategy Runner
    // The runner manages the strategy lifecycle:
    // - Initialization (on_init)
    // - Event processing (on_tick, on_bar)
    // - Shutdown (on_stop)
    // ═══════════════════════════════════════════════════════════════════
    let mut runner = CtaStrategyRunner::new(Box::new(strategy), context.clone());
    runner.start().await?;
    info!("🚀 Strategy '{}' started", runner.name());

    // ═══════════════════════════════════════════════════════════════════
    // Step 7: Simulate a new trading day
    // DualThrust calculates boundaries at the start of each day:
    // - Range = MAX(HH-LC, HC-LL) from lookback period
    // - Upper = Today's Open + K1 × Range
    // - Lower = Today's Open - K2 × Range
    // ═══════════════════════════════════════════════════════════════════
    info!("\n═══════════════════════════════════════════════════════════════");
    info!("📅 Simulating Trading Day");
    info!("═══════════════════════════════════════════════════════════════");

    let today_ts = 1704067200000i64 + 10 * 24 * 60 * 60 * 1000; // Day 10
    let today_open = dec!(42000);

    // Calculate expected boundaries for reference
    // Based on our mock data: Range ≈ 800 (HH-LC or HC-LL)
    // Upper = 42000 + 0.5 × 800 = 42400
    // Lower = 42000 - 0.5 × 800 = 41600
    info!("📍 Today's Open: {}", today_open);
    info!("📍 Expected Upper Boundary: ~42400 (Open + K1 × Range)");
    info!("📍 Expected Lower Boundary: ~41600 (Open - K2 × Range)");

    let today_bar = KlineData {
        symbol: symbol.clone(),
        timestamp: Timestamp::new_unchecked(today_ts),
        period: KlinePeriod::Day1,
        open: Price::new(today_open).unwrap(),
        high: Price::new(dec!(42500)).unwrap(),
        low: Price::new(dec!(41500)).unwrap(),
        close: Price::new(dec!(42200)).unwrap(),
        volume: Quantity::new(dec!(1500)).unwrap(),
        turnover: Amount::new(dec!(63000000)).unwrap(),
    };

    context.set_current_time(Timestamp::new_unchecked(today_ts));
    runner.on_bar(&today_bar).await?;

    // ═══════════════════════════════════════════════════════════════════
    // Step 8: Simulate intraday tick data
    // The strategy will generate signals when price breaks boundaries
    // ═══════════════════════════════════════════════════════════════════
    info!("\n═══════════════════════════════════════════════════════════════");
    info!("⏱️  Processing Intraday Ticks");
    info!("═══════════════════════════════════════════════════════════════");

    let tick_scenarios = [
        (dec!(42100), "Within range - no signal"),
        (dec!(42300), "Within range - no signal"),
        (dec!(42500), "Above upper boundary - LONG signal expected"),
        (dec!(42700), "Still above - hold long"),
        (dec!(41500), "Below lower boundary - REVERSE to SHORT"),
        (dec!(41300), "Still below - hold short"),
    ];

    for (i, (price, description)) in tick_scenarios.iter().enumerate() {
        let tick_ts = today_ts + (i as i64 + 1) * 60 * 1000; // 1 minute intervals
        let tick = create_tick(&symbol, *price, tick_ts);

        context.set_current_time(Timestamp::new_unchecked(tick_ts));
        runner.on_tick(&tick).await?;

        let position = signal_aggregator.get_total_position(&symbol);
        let pos_str = match position.as_decimal() {
            p if p > rust_decimal::Decimal::ZERO => format!("+{} (LONG)", p),
            p if p < rust_decimal::Decimal::ZERO => format!("{} (SHORT)", p),
            _ => "0 (FLAT)".to_string(),
        };

        info!(
            "  Tick {}: price={:>8} | position={:>12} | {}",
            i + 1,
            price,
            pos_str,
            description
        );
    }

    // ═══════════════════════════════════════════════════════════════════
    // Step 9: Review signals and final state
    // ═══════════════════════════════════════════════════════════════════
    info!("\n═══════════════════════════════════════════════════════════════");
    info!("📋 Signal Summary");
    info!("═══════════════════════════════════════════════════════════════");

    let signals = signal_aggregator.get_signals(&symbol);
    info!("Total signals generated: {}", signals.len());
    for (i, signal) in signals.iter().enumerate() {
        info!(
            "  {}. Strategy: {} | Target: {:>3} | Tag: {}",
            i + 1,
            signal.strategy_name,
            signal.target_position,
            signal.tag
        );
    }

    let final_position = signal_aggregator.get_aggregated_position(&symbol);
    info!(
        "\n📊 Final Aggregated Position: {}",
        final_position.total_position
    );
    for (strategy, pos) in &final_position.strategy_positions {
        info!("  └─ {}: {}", strategy, pos);
    }

    // ═══════════════════════════════════════════════════════════════════
    // Step 10: Cleanup
    // ═══════════════════════════════════════════════════════════════════
    runner.stop().await;
    info!("\n✅ Strategy stopped successfully");

    info!("\n╔════════════════════════════════════════════════════════════╗");
    info!("║                    Example Complete                        ║");
    info!("╚════════════════════════════════════════════════════════════╝");

    Ok(())
}
