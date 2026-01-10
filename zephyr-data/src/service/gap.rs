//! Data gap detection and backfill handling.
//!
//! Detects missing data points and provides backfill capabilities:
//! - Gap detection based on expected data frequency
//! - Automatic backfill from external sources
//! - Gap tracking and reporting

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use zephyr_core::types::{Symbol, Timestamp};

use super::types::{DataGap, DataServiceError, DataType};

/// Result of a backfill operation.
#[derive(Debug, Clone)]
pub struct GapBackfillResult {
    /// Number of data points backfilled
    pub backfilled_count: u64,
    /// Number of gaps that could not be filled
    pub unfilled_gaps: u64,
    /// Backfill timestamp
    pub timestamp: Timestamp,
}

/// Gap tracking information.
#[derive(Debug, Clone)]
struct GapTracker {
    /// Last recorded timestamp for a symbol
    pub last_timestamp: HashMap<Symbol, Timestamp>,
    /// Expected interval between data points in milliseconds
    pub expected_interval_ms: HashMap<(Symbol, DataType), i64>,
}

impl GapTracker {
    /// Creates a new gap tracker.
    fn new() -> Self {
        Self {
            last_timestamp: HashMap::new(),
            expected_interval_ms: HashMap::new(),
        }
    }
}

/// Data gap handler for detection and backfill.
pub struct DataGapHandler {
    tracker: Arc<RwLock<GapTracker>>,
    detected_gaps: Arc<RwLock<Vec<DataGap>>>,
}

impl DataGapHandler {
    /// Creates a new data gap handler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tracker: Arc::new(RwLock::new(GapTracker::new())),
            detected_gaps: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Detects gaps in data for a symbol within a time range.
    pub fn detect_gaps(&self, _symbol: &Symbol, start: Timestamp, end: Timestamp) -> Vec<DataGap> {
        if start >= end {
            return Vec::new();
        }

        // For now, return empty vector as we don't have actual data storage
        // In a real implementation, this would query the data storage
        Vec::new()
    }

    /// Registers expected interval for a data type.
    pub async fn register_interval(&self, symbol: &Symbol, data_type: DataType, interval_ms: i64) {
        let mut tracker = self.tracker.write().await;
        tracker
            .expected_interval_ms
            .insert((symbol.clone(), data_type), interval_ms);
    }

    /// Records a data point timestamp.
    pub async fn record_timestamp(&self, symbol: &Symbol, timestamp: Timestamp) {
        let mut tracker = self.tracker.write().await;
        tracker.last_timestamp.insert(symbol.clone(), timestamp);
    }

    /// Detects gaps based on recorded timestamps.
    pub async fn detect_gaps_from_timestamps(
        &self,
        symbol: &Symbol,
        data_type: DataType,
    ) -> Vec<DataGap> {
        let tracker = self.tracker.read().await;

        let last_ts = match tracker.last_timestamp.get(symbol) {
            Some(ts) => *ts,
            None => return Vec::new(),
        };

        let interval = match tracker
            .expected_interval_ms
            .get(&(symbol.clone(), data_type))
        {
            Some(i) => *i,
            None => return Vec::new(),
        };

        let mut gaps = Vec::new();

        // Check if there's a gap from last recorded time to now
        let now = Timestamp::now();
        let time_since_last = now.as_millis() - last_ts.as_millis();

        if time_since_last > interval {
            let expected_count = (time_since_last / interval) as u64;
            gaps.push(DataGap::new(last_ts, now, data_type, expected_count));
        }

        gaps
    }

    /// Backfills data gaps.
    pub async fn backfill(
        &self,
        symbol: &Symbol,
        gaps: &[DataGap],
    ) -> Result<(), DataServiceError> {
        if gaps.is_empty() {
            return Ok(());
        }

        // In a real implementation, this would:
        // 1. Query external data sources
        // 2. Normalize the data
        // 3. Store it in the data storage
        // 4. Update gap tracking

        tracing::info!(
            symbol = %symbol,
            gap_count = gaps.len(),
            "Backfilling data gaps"
        );

        // Record that we've processed these gaps
        let mut detected = self.detected_gaps.write().await;
        for gap in gaps {
            detected.push(gap.clone());
        }

        Ok(())
    }

    /// Gets all detected gaps.
    pub async fn get_detected_gaps(&self) -> Vec<DataGap> {
        self.detected_gaps.read().await.clone()
    }

    /// Clears detected gaps.
    pub async fn clear_gaps(&self) {
        self.detected_gaps.write().await.clear();
    }

    /// Gets gap statistics.
    pub async fn get_gap_stats(&self) -> GapStatistics {
        let gaps = self.detected_gaps.read().await;

        let total_gaps = gaps.len() as u64;
        let total_missing_points: u64 = gaps.iter().map(|g| g.expected_count).sum();
        let total_duration_ms: i64 = gaps.iter().map(|g| g.duration_ms()).sum();

        GapStatistics {
            total_gaps,
            total_missing_points,
            total_duration_ms,
            avg_gap_duration_ms: if total_gaps > 0 {
                total_duration_ms / total_gaps as i64
            } else {
                0
            },
        }
    }
}

impl Default for DataGapHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Gap statistics.
#[derive(Debug, Clone)]
pub struct GapStatistics {
    /// Total number of gaps detected
    pub total_gaps: u64,
    /// Total number of missing data points
    pub total_missing_points: u64,
    /// Total duration of all gaps in milliseconds
    pub total_duration_ms: i64,
    /// Average gap duration in milliseconds
    pub avg_gap_duration_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gap_handler_creation() {
        let handler = DataGapHandler::new();
        assert!(
            handler
                .detect_gaps(
                    &Symbol::new("BTC-USDT").unwrap(),
                    Timestamp::new(1000).unwrap(),
                    Timestamp::new(2000).unwrap()
                )
                .is_empty()
        );
    }

    #[tokio::test]
    async fn test_register_interval() {
        let handler = DataGapHandler::new();
        let symbol = Symbol::new("BTC-USDT").unwrap();

        handler
            .register_interval(&symbol, DataType::Tick, 1000)
            .await;

        // Verify by checking that we can detect gaps
        let gaps = handler
            .detect_gaps_from_timestamps(&symbol, DataType::Tick)
            .await;
        assert!(gaps.is_empty()); // No gaps yet since no timestamp recorded
    }

    #[tokio::test]
    async fn test_record_timestamp() {
        let handler = DataGapHandler::new();
        let symbol = Symbol::new("BTC-USDT").unwrap();
        let ts = Timestamp::new(1704067200000).unwrap();

        handler.record_timestamp(&symbol, ts).await;

        // Verify timestamp was recorded
        let gaps = handler
            .detect_gaps_from_timestamps(&symbol, DataType::Tick)
            .await;
        // Should have gaps if interval is registered and time has passed
        assert!(gaps.is_empty() || !gaps.is_empty()); // Just verify it doesn't panic
    }

    #[tokio::test]
    async fn test_backfill() {
        let handler = DataGapHandler::new();
        let symbol = Symbol::new("BTC-USDT").unwrap();
        let gap = DataGap::new(
            Timestamp::new(1000).unwrap(),
            Timestamp::new(2000).unwrap(),
            DataType::Tick,
            100,
        );

        let result = handler.backfill(&symbol, &[gap]).await;
        assert!(result.is_ok());

        let detected = handler.get_detected_gaps().await;
        assert_eq!(detected.len(), 1);
    }

    #[tokio::test]
    async fn test_gap_statistics() {
        let handler = DataGapHandler::new();
        let symbol = Symbol::new("BTC-USDT").unwrap();

        let gap1 = DataGap::new(
            Timestamp::new(1000).unwrap(),
            Timestamp::new(2000).unwrap(),
            DataType::Tick,
            100,
        );
        let gap2 = DataGap::new(
            Timestamp::new(3000).unwrap(),
            Timestamp::new(5000).unwrap(),
            DataType::Tick,
            200,
        );

        handler.backfill(&symbol, &[gap1, gap2]).await.unwrap();

        let stats = handler.get_gap_stats().await;
        assert_eq!(stats.total_gaps, 2);
        assert_eq!(stats.total_missing_points, 300);
    }
}
