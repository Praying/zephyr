//! Data aggregation for generating higher timeframe bars and snapshots.
//!
//! Provides:
//! - K-line aggregation from tick data
//! - Multi-timeframe K-line generation
//! - Market snapshots for point-in-time queries
//! - Historical data retrieval

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use zephyr_core::data::{KlineData, KlinePeriod, TickData};
use zephyr_core::types::{Amount, Quantity, Symbol, Timestamp};

use super::types::{DataServiceError, MarketSnapshot};

/// K-line aggregator for building bars from tick data.
pub struct KlineAggregator {
    /// Current incomplete K-line being built
    current_kline: Arc<RwLock<Option<KlineData>>>,
    /// Completed K-lines storage
    completed_klines: Arc<RwLock<Vec<KlineData>>>,
}

impl KlineAggregator {
    /// Creates a new K-line aggregator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_kline: Arc::new(RwLock::new(None)),
            completed_klines: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Processes a tick and updates the current K-line.
    pub async fn process_tick(&self, tick: &TickData, period: KlinePeriod) -> Option<KlineData> {
        let mut current = self.current_kline.write().await;

        match current.as_mut() {
            Some(kline) => {
                // Update existing K-line
                if kline.high < tick.price {
                    kline.high = tick.price;
                }
                if kline.low > tick.price {
                    kline.low = tick.price;
                }
                kline.close = tick.price;
                kline.volume =
                    Quantity::new(kline.volume.as_decimal() + tick.volume.as_decimal()).ok()?;

                // Check if we need to close this K-line
                if self.should_close_kline(kline.timestamp, tick.timestamp, period) {
                    let completed = kline.clone();
                    *current = None;

                    // Store completed K-line
                    let mut completed_klines = self.completed_klines.write().await;
                    completed_klines.push(completed.clone());

                    return Some(completed);
                }
                None
            }
            None => {
                // Start new K-line
                let new_kline = KlineData::builder()
                    .symbol(tick.symbol.clone())
                    .timestamp(self.get_kline_start_time(tick.timestamp, period))
                    .period(period)
                    .open(tick.price)
                    .high(tick.price)
                    .low(tick.price)
                    .close(tick.price)
                    .volume(tick.volume)
                    .turnover(Amount::new(tick.price.as_decimal() * tick.volume.as_decimal()).ok()?)
                    .build()
                    .ok()?;

                *current = Some(new_kline);
                None
            }
        }
    }

    /// Determines if a K-line should be closed.
    fn should_close_kline(
        &self,
        kline_start: Timestamp,
        tick_time: Timestamp,
        period: KlinePeriod,
    ) -> bool {
        let period_ms = self.period_to_ms(period);
        let elapsed = tick_time.as_millis() - kline_start.as_millis();
        elapsed >= period_ms
    }

    /// Gets the start time of a K-line for a given timestamp.
    fn get_kline_start_time(&self, timestamp: Timestamp, period: KlinePeriod) -> Timestamp {
        let period_ms = self.period_to_ms(period);
        let ts_ms = timestamp.as_millis();
        let start_ms = (ts_ms / period_ms) * period_ms;
        Timestamp::new(start_ms).unwrap_or(timestamp)
    }

    /// Converts a K-line period to milliseconds.
    fn period_to_ms(&self, period: KlinePeriod) -> i64 {
        match period {
            KlinePeriod::Minute1 => 60 * 1000,
            KlinePeriod::Minute5 => 5 * 60 * 1000,
            KlinePeriod::Minute15 => 15 * 60 * 1000,
            KlinePeriod::Minute30 => 30 * 60 * 1000,
            KlinePeriod::Hour1 => 60 * 60 * 1000,
            KlinePeriod::Hour4 => 4 * 60 * 60 * 1000,
            KlinePeriod::Day1 => 24 * 60 * 60 * 1000,
            KlinePeriod::Week1 => 7 * 24 * 60 * 60 * 1000,
        }
    }

    /// Gets all completed K-lines.
    pub async fn get_completed_klines(&self) -> Vec<KlineData> {
        self.completed_klines.read().await.clone()
    }

    /// Gets the current incomplete K-line.
    pub async fn get_current_kline(&self) -> Option<KlineData> {
        self.current_kline.read().await.clone()
    }

    /// Clears all K-lines.
    pub async fn clear(&self) {
        *self.current_kline.write().await = None;
        self.completed_klines.write().await.clear();
    }
}

impl Default for KlineAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Data aggregator for managing snapshots and historical data.
pub struct DataAggregator {
    /// Market snapshots by symbol
    snapshots: Arc<RwLock<HashMap<Symbol, MarketSnapshot>>>,
    /// K-line aggregators by symbol
    aggregators: Arc<RwLock<HashMap<Symbol, KlineAggregator>>>,
    /// Historical K-lines storage
    historical_klines: Arc<RwLock<HashMap<Symbol, Vec<KlineData>>>>,
}

impl DataAggregator {
    /// Creates a new data aggregator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            aggregators: Arc::new(RwLock::new(HashMap::new())),
            historical_klines: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Processes a tick and updates snapshots and aggregators.
    pub async fn process_tick(&self, tick: &TickData) {
        // Update snapshot
        let mut snapshots = self.snapshots.write().await;
        let snapshot = snapshots
            .entry(tick.symbol.clone())
            .or_insert_with(|| MarketSnapshot::new(tick.symbol.clone()));
        snapshot.update_tick(tick.clone());
    }

    /// Stores a K-line in historical storage.
    pub async fn store_kline(&self, kline: &KlineData) {
        let mut historical = self.historical_klines.write().await;
        let entry = historical
            .entry(kline.symbol.clone())
            .or_insert_with(Vec::new);
        entry.push(kline.clone());
    }

    /// Gets a market snapshot for a symbol.
    pub fn get_snapshot(&self, symbol: &Symbol) -> Option<MarketSnapshot> {
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            handle.block_on(async { self.snapshots.read().await.get(symbol).cloned() })
        } else {
            None
        }
    }

    /// Gets historical K-line data.
    pub async fn get_historical(
        &self,
        symbol: &Symbol,
        _period: KlinePeriod,
        start: Timestamp,
        end: Timestamp,
    ) -> Result<Vec<KlineData>, DataServiceError> {
        if start >= end {
            return Err(DataServiceError::InvalidTimeRange {
                start: start.as_millis(),
                end: end.as_millis(),
            });
        }

        let historical = self.historical_klines.read().await;
        let klines = historical
            .get(symbol)
            .ok_or_else(|| DataServiceError::SymbolNotFound(symbol.to_string()))?;

        let filtered: Vec<KlineData> = klines
            .iter()
            .filter(|k| k.timestamp >= start && k.timestamp < end)
            .cloned()
            .collect();

        Ok(filtered)
    }

    /// Gets all snapshots.
    pub async fn get_all_snapshots(&self) -> Vec<MarketSnapshot> {
        self.snapshots.read().await.values().cloned().collect()
    }

    /// Clears all data.
    pub async fn clear(&self) {
        self.snapshots.write().await.clear();
        self.aggregators.write().await.clear();
        self.historical_klines.write().await.clear();
    }
}

impl Default for DataAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use zephyr_core::types::Price;

    fn create_test_tick(price: Decimal, volume: Decimal) -> TickData {
        TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1704067200000).unwrap())
            .price(Price::new(price).unwrap())
            .volume(Quantity::new(volume).unwrap())
            .bid_price(Price::new(price - dec!(1)).unwrap())
            .bid_quantity(Quantity::new(dec!(10)).unwrap())
            .ask_price(Price::new(price + dec!(1)).unwrap())
            .ask_quantity(Quantity::new(dec!(8)).unwrap())
            .build()
            .unwrap()
    }

    fn create_test_kline() -> KlineData {
        KlineData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1704067200000).unwrap())
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

    #[tokio::test]
    async fn test_kline_aggregator_creation() {
        let agg = KlineAggregator::new();
        assert!(agg.get_current_kline().await.is_none());
        assert!(agg.get_completed_klines().await.is_empty());
    }

    #[tokio::test]
    async fn test_process_tick() {
        let agg = KlineAggregator::new();
        let tick = create_test_tick(dec!(42000), dec!(0.5));

        let result = agg.process_tick(&tick, KlinePeriod::Hour1).await;
        assert!(result.is_none()); // First tick doesn't complete a kline

        let current = agg.get_current_kline().await;
        assert!(current.is_some());
    }

    #[tokio::test]
    async fn test_data_aggregator_creation() {
        let agg = DataAggregator::new();
        let snapshots = agg.get_all_snapshots().await;
        assert!(snapshots.is_empty());
    }

    #[tokio::test]
    async fn test_process_tick_aggregator() {
        let agg = DataAggregator::new();
        let tick = create_test_tick(dec!(42000), dec!(0.5));

        agg.process_tick(&tick).await;

        let snapshots = agg.get_all_snapshots().await;
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].symbol, tick.symbol);
    }

    #[tokio::test]
    async fn test_store_and_retrieve_kline() {
        let agg = DataAggregator::new();
        let kline = create_test_kline();

        agg.store_kline(&kline).await;

        let result = agg
            .get_historical(
                &kline.symbol,
                kline.period,
                Timestamp::new(1704067200000).unwrap(),
                Timestamp::new(1704067300000).unwrap(),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_invalid_time_range() {
        let agg = DataAggregator::new();
        let symbol = Symbol::new("BTC-USDT").unwrap();

        let result = agg
            .get_historical(
                &symbol,
                KlinePeriod::Hour1,
                Timestamp::new(2000).unwrap(),
                Timestamp::new(1000).unwrap(),
            )
            .await;

        assert!(result.is_err());
    }
}
