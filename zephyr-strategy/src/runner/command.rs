//! Runner command types.
//!
//! Defines the commands that can be sent to a strategy runner.

use crate::types::{Bar, Tick};
use std::sync::Arc;
use zephyr_core::data::OrderStatus;

/// Commands that can be sent to a strategy runner.
///
/// These commands control the strategy's lifecycle and feed it market data.
#[derive(Debug, Clone)]
pub enum RunnerCommand {
    /// Process a tick (real-time trade) update.
    OnTick(Arc<Tick>),

    /// Process a bar (candlestick) update.
    OnBar(Arc<Bar>),

    /// Handle an order status change.
    OnOrderStatus(OrderStatus),

    /// Hot reload the strategy (Python only).
    ///
    /// Attempts to reload the strategy code from disk while preserving state.
    Reload {
        /// Path to the Python script
        path: String,
        /// Class name to instantiate
        class_name: String,
    },

    /// Gracefully stop the strategy.
    ///
    /// Calls `on_stop()` before terminating the runner.
    Stop,
}

impl RunnerCommand {
    /// Creates a new `OnTick` command.
    #[must_use]
    pub fn tick(tick: Tick) -> Self {
        Self::OnTick(Arc::new(tick))
    }

    /// Creates a new `OnBar` command.
    #[must_use]
    pub fn bar(bar: Bar) -> Self {
        Self::OnBar(Arc::new(bar))
    }

    /// Creates a new `OnOrderStatus` command.
    #[must_use]
    pub fn order_status(status: OrderStatus) -> Self {
        Self::OnOrderStatus(status)
    }

    /// Creates a new `Reload` command.
    #[must_use]
    pub fn reload(path: impl Into<String>, class_name: impl Into<String>) -> Self {
        Self::Reload {
            path: path.into(),
            class_name: class_name.into(),
        }
    }

    /// Creates a new `Stop` command.
    #[must_use]
    pub const fn stop() -> Self {
        Self::Stop
    }

    /// Returns true if this is a stop command.
    #[must_use]
    pub const fn is_stop(&self) -> bool {
        matches!(self, Self::Stop)
    }

    /// Returns true if this is a reload command.
    #[must_use]
    pub const fn is_reload(&self) -> bool {
        matches!(self, Self::Reload { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use zephyr_core::data::{KlineData, KlinePeriod, TickData};
    use zephyr_core::types::{Amount, Price, Quantity, Symbol, Timestamp};

    fn create_test_tick() -> Tick {
        TickData::builder()
            .symbol(Symbol::new_unchecked("BTC-USDT"))
            .timestamp(Timestamp::new_unchecked(1_704_067_200_000))
            .price(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(0.5)).unwrap())
            .build()
            .unwrap()
    }

    fn create_test_bar() -> Bar {
        KlineData::builder()
            .symbol(Symbol::new_unchecked("BTC-USDT"))
            .timestamp(Timestamp::new_unchecked(1_704_067_200_000))
            .period(KlinePeriod::Hour1)
            .open(Price::new(dec!(42000)).unwrap())
            .high(Price::new(dec!(42500)).unwrap())
            .low(Price::new(dec!(41800)).unwrap())
            .close(Price::new(dec!(42300)).unwrap())
            .volume(Quantity::new(dec!(100)).unwrap())
            .turnover(Amount::new(dec!(4200000)).unwrap())
            .build()
            .unwrap()
    }

    #[test]
    fn test_command_tick() {
        let cmd = RunnerCommand::tick(create_test_tick());
        assert!(matches!(cmd, RunnerCommand::OnTick(_)));
    }

    #[test]
    fn test_command_bar() {
        let cmd = RunnerCommand::bar(create_test_bar());
        assert!(matches!(cmd, RunnerCommand::OnBar(_)));
    }

    #[test]
    fn test_command_reload() {
        let cmd = RunnerCommand::reload("./strategy.py", "MyStrategy");
        assert!(cmd.is_reload());
        if let RunnerCommand::Reload { path, class_name } = cmd {
            assert_eq!(path, "./strategy.py");
            assert_eq!(class_name, "MyStrategy");
        }
    }

    #[test]
    fn test_command_stop() {
        let cmd = RunnerCommand::stop();
        assert!(cmd.is_stop());
    }
}
