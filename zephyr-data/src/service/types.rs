//! Data service types and error definitions.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

use zephyr_core::data::{KlineData, KlinePeriod, OrderBook, TickData};
use zephyr_core::types::{FundingRate, Symbol, Timestamp};

/// Unique identifier for a data subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    /// Creates a new subscription ID.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the inner ID value.
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for SubscriptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sub_{}", self.0)
    }
}

/// Data type enumeration for subscriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// Real-time tick data
    Tick,
    /// K-line/candlestick data with specified period
    Kline(KlinePeriod),
    /// Order book data
    OrderBook,
    /// Trade data
    Trade,
    /// Funding rate data (for perpetuals)
    FundingRate,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tick => write!(f, "tick"),
            Self::Kline(period) => write!(f, "kline_{period}"),
            Self::OrderBook => write!(f, "orderbook"),
            Self::Trade => write!(f, "trade"),
            Self::FundingRate => write!(f, "funding_rate"),
        }
    }
}

/// Market snapshot containing current state for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    /// Trading pair symbol
    pub symbol: Symbol,
    /// Snapshot timestamp
    pub timestamp: Timestamp,
    /// Last tick data
    pub last_tick: Option<TickData>,
    /// Current order book
    pub orderbook: Option<OrderBook>,
    /// Last K-line data (1-minute)
    pub last_kline: Option<KlineData>,
    /// Current funding rate (for perpetuals)
    pub funding_rate: Option<FundingRate>,
}

impl MarketSnapshot {
    /// Creates a new empty market snapshot.
    #[must_use]
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            timestamp: Timestamp::now(),
            last_tick: None,
            orderbook: None,
            last_kline: None,
            funding_rate: None,
        }
    }

    /// Updates the snapshot with new tick data.
    pub fn update_tick(&mut self, tick: TickData) {
        self.timestamp = tick.timestamp;
        self.last_tick = Some(tick);
    }

    /// Updates the snapshot with new order book data.
    pub fn update_orderbook(&mut self, orderbook: OrderBook) {
        self.timestamp = orderbook.timestamp;
        self.orderbook = Some(orderbook);
    }

    /// Updates the snapshot with new K-line data.
    pub fn update_kline(&mut self, kline: KlineData) {
        self.timestamp = kline.timestamp;
        self.last_kline = Some(kline);
    }

    /// Updates the funding rate.
    pub fn update_funding_rate(&mut self, rate: FundingRate) {
        self.funding_rate = Some(rate);
    }
}

/// Data gap representing missing data in a time range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataGap {
    /// Start of the gap (inclusive)
    pub start: Timestamp,
    /// End of the gap (exclusive)
    pub end: Timestamp,
    /// Type of data that is missing
    pub data_type: DataType,
    /// Expected number of data points in this gap
    pub expected_count: u64,
}

impl DataGap {
    /// Creates a new data gap.
    #[must_use]
    pub const fn new(
        start: Timestamp,
        end: Timestamp,
        data_type: DataType,
        expected_count: u64,
    ) -> Self {
        Self {
            start,
            end,
            data_type,
            expected_count,
        }
    }

    /// Returns the duration of the gap in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> i64 {
        self.end.as_millis() - self.start.as_millis()
    }
}

/// Data service error type.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataServiceError {
    /// Subscription not found
    #[error("Subscription not found: {0}")]
    SubscriptionNotFound(SubscriptionId),

    /// Symbol not found
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    /// Data not available
    #[error("Data not available: {0}")]
    DataNotAvailable(String),

    /// Invalid time range
    #[error("Invalid time range: start {start} >= end {end}")]
    InvalidTimeRange {
        /// Start timestamp
        start: i64,
        /// End timestamp
        end: i64,
    },

    /// Backfill failed
    #[error("Backfill failed: {reason}")]
    BackfillFailed {
        /// Reason for failure
        reason: String,
    },

    /// Quality check failed
    #[error("Quality check failed: {reason}")]
    QualityCheckFailed {
        /// Reason for failure
        reason: String,
    },

    /// Storage error
    #[error("Storage error: {reason}")]
    StorageError {
        /// Reason for failure
        reason: String,
    },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {limit} requests per {window_secs}s")]
    RateLimitExceeded {
        /// Rate limit
        limit: u32,
        /// Window in seconds
        window_secs: u32,
    },

    /// Internal error
    #[error("Internal error: {reason}")]
    InternalError {
        /// Reason for failure
        reason: String,
    },
}

impl DataServiceError {
    /// Creates a storage error.
    #[must_use]
    pub fn storage(reason: impl Into<String>) -> Self {
        Self::StorageError {
            reason: reason.into(),
        }
    }

    /// Creates an internal error.
    #[must_use]
    pub fn internal(reason: impl Into<String>) -> Self {
        Self::InternalError {
            reason: reason.into(),
        }
    }

    /// Creates a backfill failed error.
    #[must_use]
    pub fn backfill_failed(reason: impl Into<String>) -> Self {
        Self::BackfillFailed {
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_id() {
        let id = SubscriptionId::new(42);
        assert_eq!(id.as_u64(), 42);
        assert_eq!(format!("{id}"), "sub_42");
    }

    #[test]
    fn test_data_type_display() {
        assert_eq!(format!("{}", DataType::Tick), "tick");
        assert_eq!(
            format!("{}", DataType::Kline(KlinePeriod::Hour1)),
            "kline_1h"
        );
        assert_eq!(format!("{}", DataType::OrderBook), "orderbook");
    }

    #[test]
    fn test_market_snapshot() {
        let symbol = Symbol::new("BTC-USDT").unwrap();
        let snapshot = MarketSnapshot::new(symbol.clone());

        assert_eq!(snapshot.symbol, symbol);
        assert!(snapshot.last_tick.is_none());
        assert!(snapshot.orderbook.is_none());
    }

    #[test]
    fn test_data_gap() {
        let gap = DataGap::new(
            Timestamp::new(1000).unwrap(),
            Timestamp::new(2000).unwrap(),
            DataType::Tick,
            100,
        );

        assert_eq!(gap.duration_ms(), 1000);
        assert_eq!(gap.expected_count, 100);
    }

    #[test]
    fn test_data_service_error() {
        let error = DataServiceError::storage("disk full");
        assert!(error.to_string().contains("disk full"));

        let error = DataServiceError::InvalidTimeRange {
            start: 100,
            end: 50,
        };
        assert!(error.to_string().contains("100"));
    }
}
