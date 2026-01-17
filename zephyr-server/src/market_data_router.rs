//! Market data routing to strategies.

use std::sync::Arc;
use tracing::info;

use zephyr_core::data::{KlineData, TickData};

/// Market data router for distributing data to strategies.
pub struct MarketDataRouter {
    // Placeholder for future routing logic
}

impl MarketDataRouter {
    /// Creates a new market data router.
    pub fn new() -> Self {
        Self {}
    }

    /// Routes tick data to subscribed strategies.
    pub async fn on_tick(&self, tick: TickData) {
        info!("Routing tick data for symbol: {}", tick.symbol);
        // TODO: Implement actual routing to strategy runners
    }

    /// Routes K-line data to subscribed strategies.
    pub async fn on_bar(&self, bar: KlineData) {
        info!("Routing bar data for symbol: {}", bar.symbol);
        // TODO: Implement actual routing to strategy runners
    }
}

impl Default for MarketDataRouter {
    fn default() -> Self {
        Self::new()
    }
}
