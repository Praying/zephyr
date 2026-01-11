//! Binance exchange adapter.
//!
//! This module provides adapters for Binance exchange:
//! - BinanceParser - Market data parser for WebSocket streams
//! - BinanceTrader - Trading gateway for REST/WebSocket API
//!
//! # Supported Markets
//!
//! - Spot trading
//! - USDT-margined perpetual futures
//!
//! # Example
//!
//! ```ignore
//! use zephyr_gateway::binance::{BinanceParser, BinanceTrader};
//! use zephyr_core::traits::{ParserConfig, Credentials};
//!
//! // Create market data parser
//! let config = ParserConfig::builder()
//!     .endpoint("wss://stream.binance.com:9443/ws")
//!     .exchange("binance")
//!     .build();
//!
//! let mut parser = BinanceParser::new();
//! parser.init(&config).await?;
//! parser.connect().await?;
//!
//! // Create trading gateway
//! let creds = Credentials::new("api_key", "api_secret");
//! let mut trader = BinanceTrader::new_futures();
//! trader.login(&creds).await?;
//! ```

mod parser;
mod trader;
mod types;

pub use parser::BinanceParser;
pub use trader::BinanceTrader;
pub use types::*;
