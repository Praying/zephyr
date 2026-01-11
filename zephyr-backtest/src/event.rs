//! Backtest event types.

use serde::{Deserialize, Serialize};
use zephyr_core::data::{KlineData, Order, TickData};
use zephyr_core::types::{Symbol, Timestamp};

/// Type of backtest event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    /// Tick data event
    Tick,
    /// K-line bar close event
    Bar,
    /// Order fill event
    Fill,
    /// Order update event
    OrderUpdate,
}

/// Backtest event wrapper.
///
/// Represents an event that occurs during backtesting, with a timestamp
/// for chronological ordering.
#[derive(Debug, Clone)]
pub enum BacktestEvent {
    /// Tick data received
    Tick(TickData),
    /// K-line bar closed
    Bar(KlineData),
    /// Order filled or updated
    Order(Order),
}

impl BacktestEvent {
    /// Returns the timestamp of this event.
    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Self::Tick(tick) => tick.timestamp,
            Self::Bar(bar) => bar.timestamp,
            Self::Order(order) => order.update_time,
        }
    }

    /// Returns the symbol associated with this event.
    #[must_use]
    pub fn symbol(&self) -> &Symbol {
        match self {
            Self::Tick(tick) => &tick.symbol,
            Self::Bar(bar) => &bar.symbol,
            Self::Order(order) => &order.symbol,
        }
    }

    /// Returns the event type.
    #[must_use]
    pub fn event_type(&self) -> EventType {
        match self {
            Self::Tick(_) => EventType::Tick,
            Self::Bar(_) => EventType::Bar,
            Self::Order(_) => EventType::OrderUpdate,
        }
    }
}

/// Event source for multi-symbol replay.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EventSource {
    /// Symbol for this source
    pub symbol: Symbol,
    /// Current index in the data
    pub index: usize,
    /// Next timestamp to emit
    pub next_timestamp: Option<Timestamp>,
}

#[allow(dead_code)]
impl EventSource {
    /// Creates a new event source.
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            index: 0,
            next_timestamp: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use zephyr_core::data::KlinePeriod;
    use zephyr_core::types::{Amount, Price, Quantity};

    fn create_test_tick() -> TickData {
        TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .price(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(0.5)).unwrap())
            .build()
            .unwrap()
    }

    fn create_test_bar() -> KlineData {
        KlineData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
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
    fn test_event_timestamp() {
        let tick = create_test_tick();
        let event = BacktestEvent::Tick(tick);
        assert_eq!(event.timestamp().as_millis(), 1_704_067_200_000);
    }

    #[test]
    fn test_event_symbol() {
        let bar = create_test_bar();
        let event = BacktestEvent::Bar(bar);
        assert_eq!(event.symbol().as_str(), "BTC-USDT");
    }

    #[test]
    fn test_event_type() {
        let tick = create_test_tick();
        let bar = create_test_bar();

        assert_eq!(BacktestEvent::Tick(tick).event_type(), EventType::Tick);
        assert_eq!(BacktestEvent::Bar(bar).event_type(), EventType::Bar);
    }
}
