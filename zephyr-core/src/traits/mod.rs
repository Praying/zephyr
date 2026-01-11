//! Core trait definitions.
//!
//! This module provides trait definitions for parsers, traders,
//! and strategies in the Zephyr trading system.
//!
//! # Modules
//!
//! - parser - Market data parser traits and configuration
//! - trader - Trading gateway traits and order types
//! - strategy - Strategy traits for CTA and HFT strategies
//!
//! # Quick Start
//!
//! ```ignore
//! use zephyr_core::traits::*;
//!
//! // Implement a market data parser
//! struct MyParser { /* ... */ }
//! impl MarketDataParser for MyParser { /* ... */ }
//!
//! // Implement a trading gateway
//! struct MyTrader { /* ... */ }
//! impl TraderGateway for MyTrader { /* ... */ }
//!
//! // Implement a CTA strategy
//! struct MyStrategy { /* ... */ }
//! impl CtaStrategy for MyStrategy { /* ... */ }
//! ```

mod parser;
mod strategy;
mod trader;

// Re-export parser types
pub use parser::{MarketDataParser, ParserCallback, ParserConfig, ParserConfigBuilder};

// Re-export trader types (excluding types that are now in data module)
pub use trader::{Credentials, Trade, TraderCallback, TraderGateway};

// Re-export strategy types
pub use strategy::{
    CtaStrategy, CtaStrategyContext, HftStrategy, HftStrategyContext, LogLevel, OrderFlag,
    ScheduleFrequency, SelScheduleConfig, SelStrategy, SelStrategyContext, UftEngineConfig,
    UftEngineConfigBuilder, UftStrategy, UftStrategyContext,
};
