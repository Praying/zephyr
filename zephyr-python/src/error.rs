//! Python error types and exception handling.
//!
//! This module provides Python-native exceptions that map to Rust error types.

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use thiserror::Error;

use zephyr_core::error::{
    ConfigError, DataError, ExchangeError, NetworkError, StorageError, StrategyError, ZephyrError,
};
use zephyr_core::types::ValidationError;

// Create Python exception types
create_exception!(
    zephyr_py,
    ZephyrException,
    PyException,
    "Base exception for all Zephyr errors."
);
create_exception!(
    zephyr_py,
    ValidationException,
    ZephyrException,
    "Validation error for invalid input values."
);
create_exception!(
    zephyr_py,
    NetworkException,
    ZephyrException,
    "Network-related errors (connection, timeout, etc.)."
);
create_exception!(
    zephyr_py,
    ExchangeException,
    ZephyrException,
    "Exchange API errors (auth, rate limit, etc.)."
);
create_exception!(
    zephyr_py,
    DataException,
    ZephyrException,
    "Data parsing and validation errors."
);
create_exception!(
    zephyr_py,
    StrategyException,
    ZephyrException,
    "Strategy execution errors."
);
create_exception!(
    zephyr_py,
    ConfigException,
    ZephyrException,
    "Configuration errors."
);
create_exception!(
    zephyr_py,
    StorageException,
    ZephyrException,
    "Storage and I/O errors."
);

/// Python-compatible error type.
#[derive(Debug, Error)]
pub enum PyZephyrError {
    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),

    /// Network error
    #[error("Network error: {0}")]
    Network(String),

    /// Exchange error
    #[error("Exchange error: {0}")]
    Exchange(String),

    /// Data error
    #[error("Data error: {0}")]
    Data(String),

    /// Strategy error
    #[error("Strategy error: {0}")]
    Strategy(String),

    /// Config error
    #[error("Config error: {0}")]
    Config(String),

    /// Storage error
    #[error("Storage error: {0}")]
    Storage(String),
}

impl From<ValidationError> for PyZephyrError {
    fn from(err: ValidationError) -> Self {
        Self::Validation(err.to_string())
    }
}

impl From<NetworkError> for PyZephyrError {
    fn from(err: NetworkError) -> Self {
        Self::Network(err.to_string())
    }
}

impl From<ExchangeError> for PyZephyrError {
    fn from(err: ExchangeError) -> Self {
        Self::Exchange(err.to_string())
    }
}

impl From<DataError> for PyZephyrError {
    fn from(err: DataError) -> Self {
        Self::Data(err.to_string())
    }
}

impl From<StrategyError> for PyZephyrError {
    fn from(err: StrategyError) -> Self {
        Self::Strategy(err.to_string())
    }
}

impl From<ConfigError> for PyZephyrError {
    fn from(err: ConfigError) -> Self {
        Self::Config(err.to_string())
    }
}

impl From<StorageError> for PyZephyrError {
    fn from(err: StorageError) -> Self {
        Self::Storage(err.to_string())
    }
}

impl From<ZephyrError> for PyZephyrError {
    fn from(err: ZephyrError) -> Self {
        match err {
            ZephyrError::Network(e) => Self::Network(e.to_string()),
            ZephyrError::Exchange(e) => Self::Exchange(e.to_string()),
            ZephyrError::Data(e) => Self::Data(e.to_string()),
            ZephyrError::Strategy(e) => Self::Strategy(e.to_string()),
            ZephyrError::Config(e) => Self::Config(e.to_string()),
            ZephyrError::Storage(e) => Self::Storage(e.to_string()),
        }
    }
}

impl From<PyZephyrError> for PyErr {
    fn from(err: PyZephyrError) -> Self {
        match err {
            PyZephyrError::Validation(msg) => ValidationException::new_err(msg),
            PyZephyrError::Network(msg) => NetworkException::new_err(msg),
            PyZephyrError::Exchange(msg) => ExchangeException::new_err(msg),
            PyZephyrError::Data(msg) => DataException::new_err(msg),
            PyZephyrError::Strategy(msg) => StrategyException::new_err(msg),
            PyZephyrError::Config(msg) => ConfigException::new_err(msg),
            PyZephyrError::Storage(msg) => StorageException::new_err(msg),
        }
    }
}

/// Registers error types with the Python module.
pub fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("ZephyrException", m.py().get_type::<ZephyrException>())?;
    m.add(
        "ValidationException",
        m.py().get_type::<ValidationException>(),
    )?;
    m.add("NetworkException", m.py().get_type::<NetworkException>())?;
    m.add("ExchangeException", m.py().get_type::<ExchangeException>())?;
    m.add("DataException", m.py().get_type::<DataException>())?;
    m.add("StrategyException", m.py().get_type::<StrategyException>())?;
    m.add("ConfigException", m.py().get_type::<ConfigException>())?;
    m.add("StorageException", m.py().get_type::<StorageException>())?;
    Ok(())
}
