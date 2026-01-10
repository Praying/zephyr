//! Plugin system for Zephyr.
//!
//! Provides dynamic loading of strategy and exchange adapter plugins.
//!
//! # Architecture
//!
//! The plugin system supports:
//! - Strategy plugins (CTA, HFT, UFT)
//! - Exchange adapter plugins (Binance, OKX, etc.)
//! - Plugin versioning and compatibility checking
//! - Hot-reloading (optional)

mod loader;
mod registry;

pub use loader::PluginLoader;
pub use registry::PluginRegistry;
