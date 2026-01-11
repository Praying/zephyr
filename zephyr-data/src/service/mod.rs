//! Data service module for enhanced data management.
//!
//! This module provides:
//! - `DataService` trait for data ingestion and subscription
//! - `DataQualityChecker` for anomaly detection and quality reports
//! - `DataGapHandler` for gap detection and backfill
//! - `DataAggregator` for K-line aggregation and snapshot generation
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      DataService                             │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
//! │  │ Subscription │  │   Quality   │  │    Gap      │         │
//! │  │   Manager    │  │   Checker   │  │   Handler   │         │
//! │  └─────────────┘  └─────────────┘  └─────────────┘         │
//! │  ┌─────────────┐  ┌─────────────┐                          │
//! │  │ Aggregator  │  │   Storage   │                          │
//! │  └─────────────┘  └─────────────┘                          │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod aggregator;
pub mod gap;
pub mod quality;
pub mod subscription;
pub mod types;

pub use aggregator::{DataAggregator, KlineAggregator};
pub use gap::{DataGapHandler, GapBackfillResult};
pub use quality::{AnomalyType, DataQualityChecker, DataQualityReport};
pub use subscription::{DataSubscription, SubscriptionManager};
pub use types::{DataGap, DataServiceError, DataType, MarketSnapshot, SubscriptionId};

use std::sync::Arc;

use async_trait::async_trait;
use zephyr_core::data::{KlineData, KlinePeriod, TickData};
use zephyr_core::types::{Symbol, Timestamp};

/// Data service trait for data ingestion, subscription, and management.
///
/// Provides a unified interface for:
/// - Real-time data subscription with configurable delivery modes
/// - Historical data retrieval
/// - Data quality monitoring
/// - Gap detection and backfill
///
/// # Requirements
///
/// Implements Requirements 47.1 and 47.6:
/// - Data ingestion pipeline for multiple exchange feeds
/// - Data subscription with configurable delivery modes (push/pull)
#[async_trait]
pub trait DataService: Send + Sync {
    /// Subscribes to real-time data for the specified symbols and data types.
    ///
    /// # Arguments
    ///
    /// * `symbols` - Symbols to subscribe to
    /// * `data_types` - Types of data to receive (tick, kline, orderbook, etc.)
    ///
    /// # Returns
    ///
    /// A subscription ID that can be used to manage the subscription.
    ///
    /// # Errors
    ///
    /// Returns error if subscription fails.
    async fn subscribe(
        &self,
        symbols: &[Symbol],
        data_types: &[DataType],
    ) -> Result<SubscriptionId, DataServiceError>;

    /// Unsubscribes from a previous subscription.
    ///
    /// # Arguments
    ///
    /// * `id` - The subscription ID to cancel
    ///
    /// # Errors
    ///
    /// Returns error if unsubscription fails.
    async fn unsubscribe(&self, id: SubscriptionId) -> Result<(), DataServiceError>;

    /// Retrieves historical K-line data.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading pair symbol
    /// * `period` - The K-line period
    /// * `start` - Start timestamp (inclusive)
    /// * `end` - End timestamp (exclusive)
    ///
    /// # Returns
    ///
    /// Vector of K-line data within the specified range.
    ///
    /// # Errors
    ///
    /// Returns error if retrieval fails.
    async fn get_historical(
        &self,
        symbol: &Symbol,
        period: KlinePeriod,
        start: Timestamp,
        end: Timestamp,
    ) -> Result<Vec<KlineData>, DataServiceError>;

    /// Gets the current market snapshot for a symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading pair symbol
    ///
    /// # Returns
    ///
    /// The current market snapshot, or None if not available.
    fn get_snapshot(&self, symbol: &Symbol) -> Option<MarketSnapshot>;

    /// Checks data quality for a symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading pair symbol
    ///
    /// # Returns
    ///
    /// A data quality report with completeness, latency, and anomaly metrics.
    fn check_quality(&self, symbol: &Symbol) -> DataQualityReport;

    /// Detects data gaps within a time range.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading pair symbol
    /// * `start` - Start timestamp
    /// * `end` - End timestamp
    ///
    /// # Returns
    ///
    /// Vector of detected data gaps.
    fn detect_gaps(&self, symbol: &Symbol, start: Timestamp, end: Timestamp) -> Vec<DataGap>;

    /// Backfills data gaps.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading pair symbol
    /// * `gaps` - Gaps to backfill
    ///
    /// # Errors
    ///
    /// Returns error if backfill fails.
    async fn backfill(&self, symbol: &Symbol, gaps: &[DataGap]) -> Result<(), DataServiceError>;

    /// Ingests tick data into the service.
    ///
    /// # Arguments
    ///
    /// * `tick` - The tick data to ingest
    ///
    /// # Errors
    ///
    /// Returns error if ingestion fails.
    async fn ingest_tick(&self, tick: TickData) -> Result<(), DataServiceError>;

    /// Ingests K-line data into the service.
    ///
    /// # Arguments
    ///
    /// * `kline` - The K-line data to ingest
    ///
    /// # Errors
    ///
    /// Returns error if ingestion fails.
    async fn ingest_kline(&self, kline: KlineData) -> Result<(), DataServiceError>;

    /// Gets all active subscriptions.
    fn get_subscriptions(&self) -> Vec<DataSubscription>;
}

/// Default implementation of the data service.
pub struct DefaultDataService {
    subscription_manager: Arc<SubscriptionManager>,
    quality_checker: Arc<DataQualityChecker>,
    gap_handler: Arc<DataGapHandler>,
    aggregator: Arc<DataAggregator>,
}

impl DefaultDataService {
    /// Creates a new default data service.
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscription_manager: Arc::new(SubscriptionManager::new()),
            quality_checker: Arc::new(DataQualityChecker::new()),
            gap_handler: Arc::new(DataGapHandler::new()),
            aggregator: Arc::new(DataAggregator::new()),
        }
    }

    /// Gets a reference to the subscription manager.
    #[must_use]
    pub fn subscription_manager(&self) -> &SubscriptionManager {
        &self.subscription_manager
    }

    /// Gets a reference to the quality checker.
    #[must_use]
    pub fn quality_checker(&self) -> &DataQualityChecker {
        &self.quality_checker
    }

    /// Gets a reference to the gap handler.
    #[must_use]
    pub fn gap_handler(&self) -> &DataGapHandler {
        &self.gap_handler
    }

    /// Gets a reference to the aggregator.
    #[must_use]
    pub fn aggregator(&self) -> &DataAggregator {
        &self.aggregator
    }
}

impl Default for DefaultDataService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DataService for DefaultDataService {
    async fn subscribe(
        &self,
        symbols: &[Symbol],
        data_types: &[DataType],
    ) -> Result<SubscriptionId, DataServiceError> {
        self.subscription_manager
            .subscribe(symbols, data_types)
            .await
    }

    async fn unsubscribe(&self, id: SubscriptionId) -> Result<(), DataServiceError> {
        self.subscription_manager.unsubscribe(id).await
    }

    async fn get_historical(
        &self,
        symbol: &Symbol,
        period: KlinePeriod,
        start: Timestamp,
        end: Timestamp,
    ) -> Result<Vec<KlineData>, DataServiceError> {
        self.aggregator
            .get_historical(symbol, period, start, end)
            .await
    }

    fn get_snapshot(&self, symbol: &Symbol) -> Option<MarketSnapshot> {
        self.aggregator.get_snapshot(symbol)
    }

    fn check_quality(&self, symbol: &Symbol) -> DataQualityReport {
        self.quality_checker.check_quality(symbol)
    }

    fn detect_gaps(&self, symbol: &Symbol, start: Timestamp, end: Timestamp) -> Vec<DataGap> {
        self.gap_handler.detect_gaps(symbol, start, end)
    }

    async fn backfill(&self, symbol: &Symbol, gaps: &[DataGap]) -> Result<(), DataServiceError> {
        self.gap_handler.backfill(symbol, gaps).await
    }

    async fn ingest_tick(&self, tick: TickData) -> Result<(), DataServiceError> {
        // Update quality metrics
        self.quality_checker.record_tick(&tick);

        // Update aggregator
        self.aggregator.process_tick(&tick).await;

        // Notify subscribers
        self.subscription_manager.notify_tick(&tick);

        Ok(())
    }

    async fn ingest_kline(&self, kline: KlineData) -> Result<(), DataServiceError> {
        // Update quality metrics
        self.quality_checker.record_kline(&kline);

        // Store in aggregator
        self.aggregator.store_kline(&kline).await;

        // Notify subscribers
        self.subscription_manager.notify_kline(&kline);

        Ok(())
    }

    fn get_subscriptions(&self) -> Vec<DataSubscription> {
        self.subscription_manager.get_all_sync()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use zephyr_core::types::{Amount, Price, Quantity};

    fn create_test_tick() -> TickData {
        TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .price(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(0.5)).unwrap())
            .bid_price(Price::new(dec!(41999)).unwrap())
            .bid_quantity(Quantity::new(dec!(10)).unwrap())
            .ask_price(Price::new(dec!(42001)).unwrap())
            .ask_quantity(Quantity::new(dec!(8)).unwrap())
            .build()
            .unwrap()
    }

    fn create_test_kline() -> KlineData {
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
    fn test_default_data_service_creation() {
        let service = DefaultDataService::new();
        // Just verify it can be created
        let _ = service;
    }

    #[tokio::test]
    async fn test_ingest_tick() {
        let service = DefaultDataService::new();
        let tick = create_test_tick();

        let result = service.ingest_tick(tick).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ingest_kline() {
        let service = DefaultDataService::new();
        let kline = create_test_kline();

        let result = service.ingest_kline(kline).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_subscribe_unsubscribe() {
        let service = DefaultDataService::new();
        let symbols = vec![Symbol::new("BTC-USDT").unwrap()];
        let data_types = vec![DataType::Tick];

        let sub_id = service.subscribe(&symbols, &data_types).await.unwrap();
        assert_eq!(sub_id.as_u64(), 1);

        let result = service.unsubscribe(sub_id).await;
        assert!(result.is_ok());
    }
}
