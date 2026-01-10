//! # Zephyr Data
//!
//! Data storage and management for the Zephyr trading system.
//!
//! This crate provides:
//! - High-performance data storage with memory-mapped files
//! - Data compression using LZ4
//! - Checksum verification for data integrity
//! - Concurrent read access support

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod service;
pub mod storage;

pub use service::{
    AnomalyType, DataAggregator, DataGapHandler, DataQualityChecker, DataQualityReport,
    DataServiceError, DataSubscription, DataType, DefaultDataService, KlineAggregator,
    MarketSnapshot, SubscriptionId, SubscriptionManager,
};
pub use storage::{DataStorage, DataStorageError, FileStorageBackend, StorageConfig};
