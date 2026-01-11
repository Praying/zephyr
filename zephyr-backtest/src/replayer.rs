//! Data replayer for historical data playback.
//!
//! Provides chronological replay of historical tick and K-line data
//! across multiple symbols.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use serde::{Deserialize, Serialize};
use zephyr_core::data::{KlineData, TickData};
use zephyr_core::types::{Symbol, Timestamp};

use crate::error::BacktestError;
use crate::event::BacktestEvent;

/// Configuration for data replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayConfig {
    /// Start timestamp for replay (inclusive)
    pub start_time: Option<Timestamp>,
    /// End timestamp for replay (inclusive)
    pub end_time: Option<Timestamp>,
    /// Whether to validate data is sorted
    #[serde(default = "default_validate_order")]
    pub validate_order: bool,
}

fn default_validate_order() -> bool {
    true
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            start_time: None,
            end_time: None,
            validate_order: true,
        }
    }
}

/// Entry in the priority queue for event ordering.
#[derive(Debug)]
struct QueueEntry {
    timestamp: Timestamp,
    symbol_idx: usize,
    is_tick: bool,
    data_idx: usize,
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
    }
}

impl Eq for QueueEntry {}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior (earliest first)
        other.timestamp.cmp(&self.timestamp)
    }
}

/// Data replayer for historical data playback.
///
/// Replays tick and K-line data in chronological order across multiple symbols.
///
/// # Examples
///
/// ```
/// use zephyr_backtest::{DataReplayer, ReplayConfig};
/// use zephyr_core::types::Symbol;
///
/// let mut replayer = DataReplayer::new(ReplayConfig::default());
/// // Add data and replay...
/// ```
pub struct DataReplayer {
    config: ReplayConfig,
    /// Tick data by symbol
    tick_data: HashMap<Symbol, Vec<TickData>>,
    /// K-line data by symbol
    kline_data: HashMap<Symbol, Vec<KlineData>>,
    /// Symbol index mapping
    symbols: Vec<Symbol>,
    /// Priority queue for event ordering
    queue: BinaryHeap<QueueEntry>,
    /// Current indices for tick data per symbol
    tick_indices: Vec<usize>,
    /// Current indices for kline data per symbol
    kline_indices: Vec<usize>,
    /// Whether the replayer has been initialized
    initialized: bool,
    /// Timestamps processed (for verification)
    processed_timestamps: Vec<Timestamp>,
}

impl DataReplayer {
    /// Creates a new data replayer with the given configuration.
    #[must_use]
    pub fn new(config: ReplayConfig) -> Self {
        Self {
            config,
            tick_data: HashMap::new(),
            kline_data: HashMap::new(),
            symbols: Vec::new(),
            queue: BinaryHeap::new(),
            tick_indices: Vec::new(),
            kline_indices: Vec::new(),
            initialized: false,
            processed_timestamps: Vec::new(),
        }
    }

    /// Adds tick data for a symbol.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is not sorted chronologically.
    pub fn add_tick_data(
        &mut self,
        symbol: Symbol,
        data: Vec<TickData>,
    ) -> Result<(), BacktestError> {
        if self.config.validate_order {
            Self::validate_tick_order(&data)?;
        }

        // Filter by time range
        let filtered = self.filter_tick_data(data);

        if !self.tick_data.contains_key(&symbol) && !self.kline_data.contains_key(&symbol) {
            self.symbols.push(symbol.clone());
        }

        self.tick_data.insert(symbol, filtered);
        self.initialized = false;
        Ok(())
    }

    /// Adds K-line data for a symbol.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is not sorted chronologically.
    pub fn add_kline_data(
        &mut self,
        symbol: Symbol,
        data: Vec<KlineData>,
    ) -> Result<(), BacktestError> {
        if self.config.validate_order {
            Self::validate_kline_order(&data)?;
        }

        // Filter by time range
        let filtered = self.filter_kline_data(data);

        if !self.tick_data.contains_key(&symbol) && !self.kline_data.contains_key(&symbol) {
            self.symbols.push(symbol.clone());
        }

        self.kline_data.insert(symbol, filtered);
        self.initialized = false;
        Ok(())
    }

    /// Validates that tick data is sorted chronologically.
    fn validate_tick_order(data: &[TickData]) -> Result<(), BacktestError> {
        for i in 1..data.len() {
            if data[i].timestamp < data[i - 1].timestamp {
                return Err(BacktestError::UnsortedData {
                    index: i,
                    current: data[i - 1].timestamp.as_millis(),
                    next: data[i].timestamp.as_millis(),
                });
            }
        }
        Ok(())
    }

    /// Validates that K-line data is sorted chronologically.
    fn validate_kline_order(data: &[KlineData]) -> Result<(), BacktestError> {
        for i in 1..data.len() {
            if data[i].timestamp < data[i - 1].timestamp {
                return Err(BacktestError::UnsortedData {
                    index: i,
                    current: data[i - 1].timestamp.as_millis(),
                    next: data[i].timestamp.as_millis(),
                });
            }
        }
        Ok(())
    }

    /// Filters tick data by time range.
    fn filter_tick_data(&self, data: Vec<TickData>) -> Vec<TickData> {
        data.into_iter()
            .filter(|t| {
                let ts = t.timestamp;
                let after_start = self.config.start_time.is_none_or(|s| ts >= s);
                let before_end = self.config.end_time.is_none_or(|e| ts <= e);
                after_start && before_end
            })
            .collect()
    }

    /// Filters K-line data by time range.
    fn filter_kline_data(&self, data: Vec<KlineData>) -> Vec<KlineData> {
        data.into_iter()
            .filter(|k| {
                let ts = k.timestamp;
                let after_start = self.config.start_time.is_none_or(|s| ts >= s);
                let before_end = self.config.end_time.is_none_or(|e| ts <= e);
                after_start && before_end
            })
            .collect()
    }

    /// Initializes the replayer for iteration.
    fn initialize(&mut self) {
        self.queue.clear();
        self.tick_indices = vec![0; self.symbols.len()];
        self.kline_indices = vec![0; self.symbols.len()];
        self.processed_timestamps.clear();

        // Add initial entries to the queue
        for (idx, symbol) in self.symbols.iter().enumerate() {
            // Add first tick if available
            if let Some(ticks) = self.tick_data.get(symbol)
                && !ticks.is_empty()
            {
                self.queue.push(QueueEntry {
                    timestamp: ticks[0].timestamp,
                    symbol_idx: idx,
                    is_tick: true,
                    data_idx: 0,
                });
            }

            // Add first kline if available
            if let Some(klines) = self.kline_data.get(symbol)
                && !klines.is_empty()
            {
                self.queue.push(QueueEntry {
                    timestamp: klines[0].timestamp,
                    symbol_idx: idx,
                    is_tick: false,
                    data_idx: 0,
                });
            }
        }

        self.initialized = true;
    }

    /// Resets the replayer to the beginning.
    pub fn reset(&mut self) {
        self.initialized = false;
        self.processed_timestamps.clear();
    }

    /// Returns the next event in chronological order.
    #[must_use]
    pub fn next_event(&mut self) -> Option<BacktestEvent> {
        if !self.initialized {
            self.initialize();
        }

        let entry = self.queue.pop()?;
        let symbol = &self.symbols[entry.symbol_idx];

        let event = if entry.is_tick {
            let ticks = self.tick_data.get(symbol)?;
            let tick = ticks.get(entry.data_idx)?.clone();

            // Add next tick to queue
            let next_idx = entry.data_idx + 1;
            if next_idx < ticks.len() {
                self.queue.push(QueueEntry {
                    timestamp: ticks[next_idx].timestamp,
                    symbol_idx: entry.symbol_idx,
                    is_tick: true,
                    data_idx: next_idx,
                });
            }

            self.processed_timestamps.push(tick.timestamp);
            BacktestEvent::Tick(tick)
        } else {
            let klines = self.kline_data.get(symbol)?;
            let kline = klines.get(entry.data_idx)?.clone();

            // Add next kline to queue
            let next_idx = entry.data_idx + 1;
            if next_idx < klines.len() {
                self.queue.push(QueueEntry {
                    timestamp: klines[next_idx].timestamp,
                    symbol_idx: entry.symbol_idx,
                    is_tick: false,
                    data_idx: next_idx,
                });
            }

            self.processed_timestamps.push(kline.timestamp);
            BacktestEvent::Bar(kline)
        };

        Some(event)
    }

    /// Returns all processed timestamps (for verification).
    #[must_use]
    pub fn processed_timestamps(&self) -> &[Timestamp] {
        &self.processed_timestamps
    }

    /// Returns true if all timestamps are in chronological order.
    #[must_use]
    pub fn verify_chronological_order(&self) -> bool {
        self.processed_timestamps.windows(2).all(|w| w[0] <= w[1])
    }

    /// Returns the total number of events to replay.
    #[must_use]
    pub fn total_events(&self) -> usize {
        let tick_count: usize = self.tick_data.values().map(Vec::len).sum();
        let kline_count: usize = self.kline_data.values().map(Vec::len).sum();
        tick_count + kline_count
    }

    /// Returns the symbols being replayed.
    #[must_use]
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }
}

impl Iterator for DataReplayer {
    type Item = BacktestEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_event()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use zephyr_core::data::KlinePeriod;
    use zephyr_core::types::{Amount, Price, Quantity};

    fn create_tick(symbol: &str, ts: i64, price: rust_decimal::Decimal) -> TickData {
        TickData::builder()
            .symbol(Symbol::new(symbol).unwrap())
            .timestamp(Timestamp::new(ts).unwrap())
            .price(Price::new(price).unwrap())
            .volume(Quantity::new(dec!(1)).unwrap())
            .build()
            .unwrap()
    }

    fn create_kline(symbol: &str, ts: i64) -> KlineData {
        KlineData::builder()
            .symbol(Symbol::new(symbol).unwrap())
            .timestamp(Timestamp::new(ts).unwrap())
            .period(KlinePeriod::Minute1)
            .open(Price::new(dec!(100)).unwrap())
            .high(Price::new(dec!(105)).unwrap())
            .low(Price::new(dec!(95)).unwrap())
            .close(Price::new(dec!(102)).unwrap())
            .volume(Quantity::new(dec!(1000)).unwrap())
            .turnover(Amount::new(dec!(100000)).unwrap())
            .build()
            .unwrap()
    }

    #[test]
    fn test_replayer_single_symbol_ticks() {
        let mut replayer = DataReplayer::new(ReplayConfig::default());

        let ticks = vec![
            create_tick("BTC-USDT", 1000, dec!(100)),
            create_tick("BTC-USDT", 2000, dec!(101)),
            create_tick("BTC-USDT", 3000, dec!(102)),
        ];

        replayer
            .add_tick_data(Symbol::new("BTC-USDT").unwrap(), ticks)
            .unwrap();

        let events: Vec<_> = replayer.collect();
        assert_eq!(events.len(), 3);

        // Verify chronological order
        let timestamps: Vec<_> = events.iter().map(|e| e.timestamp().as_millis()).collect();
        assert_eq!(timestamps, vec![1000, 2000, 3000]);
    }

    #[test]
    fn test_replayer_multi_symbol_interleaved() {
        let mut replayer = DataReplayer::new(ReplayConfig::default());

        let btc_ticks = vec![
            create_tick("BTC-USDT", 1000, dec!(100)),
            create_tick("BTC-USDT", 3000, dec!(102)),
            create_tick("BTC-USDT", 5000, dec!(104)),
        ];

        let eth_ticks = vec![
            create_tick("ETH-USDT", 2000, dec!(50)),
            create_tick("ETH-USDT", 4000, dec!(51)),
            create_tick("ETH-USDT", 6000, dec!(52)),
        ];

        replayer
            .add_tick_data(Symbol::new("BTC-USDT").unwrap(), btc_ticks)
            .unwrap();
        replayer
            .add_tick_data(Symbol::new("ETH-USDT").unwrap(), eth_ticks)
            .unwrap();

        let events: Vec<_> = replayer.collect();
        assert_eq!(events.len(), 6);

        // Verify chronological order
        let timestamps: Vec<_> = events.iter().map(|e| e.timestamp().as_millis()).collect();
        assert_eq!(timestamps, vec![1000, 2000, 3000, 4000, 5000, 6000]);
    }

    #[test]
    fn test_replayer_mixed_tick_and_kline() {
        let mut replayer = DataReplayer::new(ReplayConfig::default());

        let ticks = vec![
            create_tick("BTC-USDT", 1000, dec!(100)),
            create_tick("BTC-USDT", 2500, dec!(101)),
        ];

        let klines = vec![
            create_kline("BTC-USDT", 2000),
            create_kline("BTC-USDT", 3000),
        ];

        replayer
            .add_tick_data(Symbol::new("BTC-USDT").unwrap(), ticks)
            .unwrap();
        replayer
            .add_kline_data(Symbol::new("BTC-USDT").unwrap(), klines)
            .unwrap();

        let events: Vec<_> = replayer.collect();
        assert_eq!(events.len(), 4);

        // Verify chronological order
        let timestamps: Vec<_> = events.iter().map(|e| e.timestamp().as_millis()).collect();
        assert_eq!(timestamps, vec![1000, 2000, 2500, 3000]);
    }

    #[test]
    fn test_replayer_validates_unsorted_data() {
        let mut replayer = DataReplayer::new(ReplayConfig::default());

        let unsorted_ticks = vec![
            create_tick("BTC-USDT", 2000, dec!(100)),
            create_tick("BTC-USDT", 1000, dec!(101)), // Out of order
        ];

        let result = replayer.add_tick_data(Symbol::new("BTC-USDT").unwrap(), unsorted_ticks);
        assert!(matches!(result, Err(BacktestError::UnsortedData { .. })));
    }

    #[test]
    fn test_replayer_time_range_filter() {
        let config = ReplayConfig {
            start_time: Some(Timestamp::new(2000).unwrap()),
            end_time: Some(Timestamp::new(4000).unwrap()),
            validate_order: true,
        };

        let mut replayer = DataReplayer::new(config);

        let ticks = vec![
            create_tick("BTC-USDT", 1000, dec!(100)),
            create_tick("BTC-USDT", 2000, dec!(101)),
            create_tick("BTC-USDT", 3000, dec!(102)),
            create_tick("BTC-USDT", 4000, dec!(103)),
            create_tick("BTC-USDT", 5000, dec!(104)),
        ];

        replayer
            .add_tick_data(Symbol::new("BTC-USDT").unwrap(), ticks)
            .unwrap();

        assert_eq!(replayer.count(), 3); // Only 2000, 3000, 4000
    }

    #[test]
    fn test_replayer_reset() {
        let mut replayer = DataReplayer::new(ReplayConfig::default());

        let ticks = vec![
            create_tick("BTC-USDT", 1000, dec!(100)),
            create_tick("BTC-USDT", 2000, dec!(101)),
        ];

        replayer
            .add_tick_data(Symbol::new("BTC-USDT").unwrap(), ticks)
            .unwrap();

        // First pass
        let events1: Vec<_> = replayer.by_ref().collect();
        assert_eq!(events1.len(), 2);

        // Reset and replay
        replayer.reset();
        assert_eq!(replayer.count(), 2);
    }

    #[test]
    fn test_replayer_verify_chronological_order() {
        let mut replayer = DataReplayer::new(ReplayConfig::default());

        let ticks = vec![
            create_tick("BTC-USDT", 1000, dec!(100)),
            create_tick("BTC-USDT", 2000, dec!(101)),
            create_tick("BTC-USDT", 3000, dec!(102)),
        ];

        replayer
            .add_tick_data(Symbol::new("BTC-USDT").unwrap(), ticks)
            .unwrap();

        // Consume all events
        let _: Vec<_> = replayer.by_ref().collect();

        assert!(replayer.verify_chronological_order());
    }

    #[test]
    fn test_replayer_total_events() {
        let mut replayer = DataReplayer::new(ReplayConfig::default());

        let ticks = vec![
            create_tick("BTC-USDT", 1000, dec!(100)),
            create_tick("BTC-USDT", 2000, dec!(101)),
        ];

        let klines = vec![create_kline("BTC-USDT", 3000)];

        replayer
            .add_tick_data(Symbol::new("BTC-USDT").unwrap(), ticks)
            .unwrap();
        replayer
            .add_kline_data(Symbol::new("BTC-USDT").unwrap(), klines)
            .unwrap();

        assert_eq!(replayer.total_events(), 3);
    }
}
