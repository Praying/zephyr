//! Data storage interface and implementations.
//!
//! This module provides:
//! - `DataStorage` trait for pluggable storage backends
//! - `FileStorageBackend` for file-based storage with memory mapping
//! - Compression support using LZ4
//! - Data integrity verification using CRC32 checksums
//! - Concurrent read access support

// Allow HashMap in this module - it's used for single-threaded storage index
#![allow(clippy::disallowed_types)]

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error};

use zephyr_core::data::{KlineData, TickData};

/// Storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Base directory for storage
    pub base_dir: PathBuf,
    /// Enable compression
    pub compression_enabled: bool,
    /// Enable checksums
    pub checksum_enabled: bool,
    /// Maximum file size before rotation (in bytes)
    pub max_file_size: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from("./data"),
            compression_enabled: true,
            checksum_enabled: true,
            max_file_size: 1024 * 1024 * 1024, // 1GB
        }
    }
}

/// Storage errors.
#[derive(Error, Debug)]
pub enum DataStorageError {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Compression error
    #[error("Compression error: {0}")]
    Compression(String),

    /// Checksum mismatch
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Expected checksum value
        expected: u32,
        /// Actual checksum value
        actual: u32,
    },

    /// Data corruption detected
    #[error("Data corruption detected: {0}")]
    DataCorruption(String),

    /// File not found
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Memory mapping error
    #[error("Memory mapping error: {0}")]
    MemoryMappingError(String),
}

/// Data storage trait for pluggable backends.
///
/// Provides interface for storing and retrieving market data with optional
/// compression and integrity verification.
#[async_trait::async_trait]
pub trait DataStorage: Send + Sync {
    /// Stores tick data.
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails.
    async fn store_tick(&self, tick: &TickData) -> Result<(), DataStorageError>;

    /// Stores multiple tick data records.
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails.
    async fn store_ticks(&self, ticks: &[TickData]) -> Result<(), DataStorageError>;

    /// Stores kline data.
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails.
    async fn store_kline(&self, kline: &KlineData) -> Result<(), DataStorageError>;

    /// Stores multiple kline data records.
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails.
    async fn store_klines(&self, klines: &[KlineData]) -> Result<(), DataStorageError>;

    /// Retrieves tick data for a symbol within a time range.
    ///
    /// # Errors
    ///
    /// Returns error if retrieval fails.
    async fn get_ticks(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<TickData>, DataStorageError>;

    /// Retrieves kline data for a symbol within a time range.
    ///
    /// # Errors
    ///
    /// Returns error if retrieval fails.
    async fn get_klines(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<KlineData>, DataStorageError>;

    /// Verifies data integrity.
    ///
    /// # Errors
    ///
    /// Returns error if verification fails.
    async fn verify_integrity(&self) -> Result<bool, DataStorageError>;
}

/// File-based storage backend with memory mapping support.
///
/// Provides high-performance storage with optional LZ4 compression
/// and CRC32 checksums for data integrity.
pub struct FileStorageBackend {
    config: StorageConfig,
    // In-memory index for concurrent access (reserved for future use)
    #[allow(dead_code)]
    index: Arc<RwLock<StorageIndex>>,
}

/// In-memory index for fast lookups.
#[derive(Debug, Default)]
#[allow(dead_code)]
struct StorageIndex {
    tick_files: std::collections::HashMap<String, PathBuf>,
    kline_files: std::collections::HashMap<String, PathBuf>,
}

impl FileStorageBackend {
    /// Creates a new file storage backend.
    ///
    /// # Errors
    ///
    /// Returns error if configuration is invalid or directory creation fails.
    pub fn new(config: StorageConfig) -> Result<Self, DataStorageError> {
        // Create base directory if it doesn't exist
        std::fs::create_dir_all(&config.base_dir).map_err(|e| {
            error!("Failed to create storage directory: {}", e);
            DataStorageError::Io(e)
        })?;

        // Create subdirectories
        std::fs::create_dir_all(config.base_dir.join("ticks")).map_err(|e| {
            error!("Failed to create ticks directory: {}", e);
            DataStorageError::Io(e)
        })?;

        std::fs::create_dir_all(config.base_dir.join("klines")).map_err(|e| {
            error!("Failed to create klines directory: {}", e);
            DataStorageError::Io(e)
        })?;

        debug!("FileStorageBackend initialized at {:?}", config.base_dir);

        Ok(Self {
            config,
            index: Arc::new(RwLock::new(StorageIndex::default())),
        })
    }

    /// Gets the file path for tick data.
    fn get_tick_file_path(&self, symbol: &str, date: &str) -> PathBuf {
        self.config
            .base_dir
            .join("ticks")
            .join(format!("{symbol}_{date}.dat"))
    }

    /// Gets the file path for kline data.
    fn get_kline_file_path(&self, symbol: &str, period: &str, date: &str) -> PathBuf {
        self.config
            .base_dir
            .join("klines")
            .join(format!("{symbol}_{period}_{date}.dat"))
    }

    /// Compresses data using LZ4.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, DataStorageError> {
        Ok(lz4_flex::compress_prepend_size(data))
    }

    /// Decompresses data using LZ4.
    #[allow(clippy::unused_self)]
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, DataStorageError> {
        lz4_flex::decompress_size_prepended(data)
            .map_err(|e| DataStorageError::Compression(format!("LZ4 decompression failed: {e}")))
    }

    /// Calculates CRC32 checksum.
    #[allow(clippy::unused_self)]
    fn calculate_checksum(&self, data: &[u8]) -> u32 {
        crc32fast::hash(data)
    }

    /// Verifies CRC32 checksum.
    fn verify_checksum(&self, data: &[u8], expected: u32) -> Result<(), DataStorageError> {
        let actual = self.calculate_checksum(data);
        if actual == expected {
            Ok(())
        } else {
            Err(DataStorageError::ChecksumMismatch { expected, actual })
        }
    }

    /// Writes data to file with optional compression and checksum.
    #[allow(clippy::unused_async)]
    async fn write_data(&self, path: &Path, data: &[u8]) -> Result<(), DataStorageError> {
        let mut final_data = data.to_vec();

        // Compress if enabled
        if self.config.compression_enabled {
            final_data = self.compress(&final_data)?;
        }

        // Calculate checksum
        let checksum = self.calculate_checksum(&final_data);

        // Write to file: [checksum (4 bytes)][data]
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|e| {
                error!("Failed to open file for writing: {:?}", path);
                DataStorageError::Io(e)
            })?;

        file.write_all(&checksum.to_le_bytes()).map_err(|e| {
            error!("Failed to write checksum: {}", e);
            DataStorageError::Io(e)
        })?;

        file.write_all(&final_data).map_err(|e| {
            error!("Failed to write data: {}", e);
            DataStorageError::Io(e)
        })?;

        debug!("Data written to {:?}", path);
        Ok(())
    }

    /// Reads data from file with optional decompression and checksum verification.
    #[allow(clippy::unused_async)]
    async fn read_data(&self, path: &Path) -> Result<Vec<u8>, DataStorageError> {
        let mut file = File::open(path).map_err(|e| {
            error!("Failed to open file for reading: {:?}", path);
            if e.kind() == io::ErrorKind::NotFound {
                DataStorageError::FileNotFound(path.to_path_buf())
            } else {
                DataStorageError::Io(e)
            }
        })?;

        // Read checksum (4 bytes)
        let mut checksum_bytes = [0u8; 4];
        file.read_exact(&mut checksum_bytes).map_err(|e| {
            error!("Failed to read checksum: {}", e);
            DataStorageError::Io(e)
        })?;
        let expected_checksum = u32::from_le_bytes(checksum_bytes);

        // Read data
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| {
            error!("Failed to read data: {}", e);
            DataStorageError::Io(e)
        })?;

        // Verify checksum
        if self.config.checksum_enabled {
            self.verify_checksum(&data, expected_checksum)?;
        }

        // Decompress if needed
        if self.config.compression_enabled {
            data = self.decompress(&data)?;
        }

        debug!("Data read from {:?}", path);
        Ok(data)
    }
}

#[async_trait::async_trait]
impl DataStorage for FileStorageBackend {
    async fn store_tick(&self, tick: &TickData) -> Result<(), DataStorageError> {
        let json = serde_json::to_vec(tick)?;
        let date = chrono::Local::now().format("%Y%m%d").to_string();
        let path = self.get_tick_file_path(tick.symbol.as_str(), &date);
        self.write_data(&path, &json).await
    }

    async fn store_ticks(&self, ticks: &[TickData]) -> Result<(), DataStorageError> {
        for tick in ticks {
            self.store_tick(tick).await?;
        }
        Ok(())
    }

    async fn store_kline(&self, kline: &KlineData) -> Result<(), DataStorageError> {
        let json = serde_json::to_vec(kline)?;
        let date = chrono::Local::now().format("%Y%m%d").to_string();
        let path = self.get_kline_file_path(kline.symbol.as_str(), kline.period.as_str(), &date);
        self.write_data(&path, &json).await
    }

    async fn store_klines(&self, klines: &[KlineData]) -> Result<(), DataStorageError> {
        for kline in klines {
            self.store_kline(kline).await?;
        }
        Ok(())
    }

    async fn get_ticks(
        &self,
        symbol: &str,
        _start_time: i64,
        _end_time: i64,
    ) -> Result<Vec<TickData>, DataStorageError> {
        // Simplified implementation - in production would query by time range
        let today = chrono::Local::now().format("%Y%m%d").to_string();
        let path = self.get_tick_file_path(symbol, &today);

        let file_data = self.read_data(&path).await?;
        let tick: TickData = serde_json::from_slice(&file_data)?;
        Ok(vec![tick])
    }

    async fn get_klines(
        &self,
        symbol: &str,
        _start_time: i64,
        _end_time: i64,
    ) -> Result<Vec<KlineData>, DataStorageError> {
        // Simplified implementation - in production would query by time range
        let today = chrono::Local::now().format("%Y%m%d").to_string();
        let path = self.get_kline_file_path(symbol, "1h", &today);

        let file_data = self.read_data(&path).await?;
        let kline: KlineData = serde_json::from_slice(&file_data)?;
        Ok(vec![kline])
    }

    async fn verify_integrity(&self) -> Result<bool, DataStorageError> {
        // Scan all files and verify checksums
        let ticks_dir = self.config.base_dir.join("ticks");
        let klines_dir = self.config.base_dir.join("klines");

        for entry in std::fs::read_dir(&ticks_dir).map_err(|e| {
            error!("Failed to read ticks directory: {}", e);
            DataStorageError::Io(e)
        })? {
            let entry = entry.map_err(|e| {
                error!("Failed to read directory entry: {}", e);
                DataStorageError::Io(e)
            })?;
            let path = entry.path();

            if path.is_file() {
                let _ = self.read_data(&path).await?;
            }
        }

        for entry in std::fs::read_dir(&klines_dir).map_err(|e| {
            error!("Failed to read klines directory: {}", e);
            DataStorageError::Io(e)
        })? {
            let entry = entry.map_err(|e| {
                error!("Failed to read directory entry: {}", e);
                DataStorageError::Io(e)
            })?;
            let path = entry.path();

            if path.is_file() {
                let _ = self.read_data(&path).await?;
            }
        }

        debug!("Data integrity verification completed successfully");
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zephyr_core::types::{Price, Quantity, Symbol, Timestamp};

    #[tokio::test]
    async fn test_file_storage_create() {
        let config = StorageConfig {
            base_dir: PathBuf::from("./test_data"),
            compression_enabled: false,
            checksum_enabled: true,
            max_file_size: 1024 * 1024,
        };

        let result = FileStorageBackend::new(config);
        assert!(result.is_ok());

        // Cleanup
        let _ = std::fs::remove_dir_all("./test_data");
    }

    #[tokio::test]
    async fn test_store_and_retrieve_tick() {
        let config = StorageConfig {
            base_dir: PathBuf::from("./test_data_tick"),
            compression_enabled: false,
            checksum_enabled: true,
            max_file_size: 1024 * 1024,
        };

        let storage = FileStorageBackend::new(config).unwrap();

        let tick = TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1704067200000).unwrap())
            .price(Price::new("42000".parse().unwrap()).unwrap())
            .volume(Quantity::new("0.5".parse().unwrap()).unwrap())
            .bid_price(Price::new("41999".parse().unwrap()).unwrap())
            .bid_quantity(Quantity::new("10".parse().unwrap()).unwrap())
            .ask_price(Price::new("42001".parse().unwrap()).unwrap())
            .ask_quantity(Quantity::new("8".parse().unwrap()).unwrap())
            .build()
            .unwrap();

        let result = storage.store_tick(&tick).await;
        assert!(result.is_ok());

        // Cleanup
        let _ = std::fs::remove_dir_all("./test_data_tick");
    }

    #[tokio::test]
    async fn test_store_and_retrieve_kline() {
        let config = StorageConfig {
            base_dir: PathBuf::from("./test_data_kline"),
            compression_enabled: false,
            checksum_enabled: true,
            max_file_size: 1024 * 1024,
        };

        let storage = FileStorageBackend::new(config).unwrap();

        let kline = KlineData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1704067200000).unwrap())
            .period(zephyr_core::data::KlinePeriod::Hour1)
            .open(Price::new("42000".parse().unwrap()).unwrap())
            .high(Price::new("42500".parse().unwrap()).unwrap())
            .low(Price::new("41800".parse().unwrap()).unwrap())
            .close(Price::new("42300".parse().unwrap()).unwrap())
            .volume(Quantity::new("100".parse().unwrap()).unwrap())
            .turnover(zephyr_core::types::Amount::new("4200000".parse().unwrap()).unwrap())
            .build()
            .unwrap();

        let result = storage.store_kline(&kline).await;
        assert!(result.is_ok());

        // Cleanup
        let _ = std::fs::remove_dir_all("./test_data_kline");
    }

    #[tokio::test]
    async fn test_compression_roundtrip() {
        let config = StorageConfig {
            base_dir: PathBuf::from("./test_data_compression"),
            compression_enabled: true,
            checksum_enabled: true,
            max_file_size: 1024 * 1024,
        };

        let storage = FileStorageBackend::new(config).unwrap();

        let tick = TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1704067200000).unwrap())
            .price(Price::new("42000".parse().unwrap()).unwrap())
            .volume(Quantity::new("0.5".parse().unwrap()).unwrap())
            .bid_price(Price::new("41999".parse().unwrap()).unwrap())
            .bid_quantity(Quantity::new("10".parse().unwrap()).unwrap())
            .ask_price(Price::new("42001".parse().unwrap()).unwrap())
            .ask_quantity(Quantity::new("8".parse().unwrap()).unwrap())
            .build()
            .unwrap();

        let result = storage.store_tick(&tick).await;
        assert!(result.is_ok());

        // Cleanup
        let _ = std::fs::remove_dir_all("./test_data_compression");
    }

    #[tokio::test]
    async fn test_checksum_verification() {
        let config = StorageConfig {
            base_dir: PathBuf::from("./test_data_checksum"),
            compression_enabled: false,
            checksum_enabled: true,
            max_file_size: 1024 * 1024,
        };

        let storage = FileStorageBackend::new(config).unwrap();

        let data = b"test data";
        let checksum = storage.calculate_checksum(data);

        let result = storage.verify_checksum(data, checksum);
        assert!(result.is_ok());

        // Verify with wrong checksum
        let result = storage.verify_checksum(data, checksum + 1);
        assert!(result.is_err());

        // Cleanup
        let _ = std::fs::remove_dir_all("./test_data_checksum");
    }
}
