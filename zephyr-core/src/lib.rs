//! # Zephyr Core
//!
//! Core types, traits, and interfaces for Zephyr cryptocurrency trading system.
//!
//! This crate provides:
//! - `NewType` wrappers for financial primitives (Price, Quantity, Amount, etc.)
//! - Core data structures (`TickData`, `KlineData`, `OrderBook`)
//! - Error types and handling framework
//! - Trait definitions for parsers, traders, and strategies
//! - Cryptocurrency-specific features (funding rates, margin calculations, contract management)
//! - Configuration management with YAML/TOML support and environment variable overrides

#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_const_for_fn)]

/// Core type definitions and 'NewType' wrappers
pub mod types;

/// Market data structures
pub mod data;

/// Error types and handling
pub mod error;

/// Core trait definitions
pub mod traits;

/// Cryptocurrency-specific features
pub mod crypto;

/// Configuration management
pub mod config;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::config::*;
    pub use crate::crypto::*;
    pub use crate::data::*;
    pub use crate::traits::*;
    pub use crate::types::*;
}
