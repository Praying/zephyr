//! Bitget exchange adapter.
//!
//! This module provides adapters for Bitget exchange:
//! - BitgetParser - Market data parser for WebSocket streams
//! - BitgetTrader - Trading gateway for REST/WebSocket API
//!
//! # Supported Markets
//!
//! - Spot trading
//! - USDT-margined perpetual futures
//! - Coin-margined perpetual futures
//!
//! # Bitget-Specific Features
//!
//! - Requires passphrase in addition to API key and secret
//! - Uses product types: SPOT, USDT-FUTURES, COIN-FUTURES
//! - Supports margin modes: crossed, isolated
//!
//! # Example
//!
//! ```ignore
//! use zephyr_gateway::bitget::{BitgetParser, BitgetTrader};
//! use zephyr_core::traits::{ParserConfig, Credentials};
//!
//! // Create market data parser
//! let config = ParserConfig::builder()
//!     .endpoint("wss://ws.bitget.com/v2/ws/public")
//!     .exchange("bitget")
//!     .build();
//!
//! let mut parser = BitgetParser::new();
//! parser.init(&config).await?;
//! parser.connect().await?;
//!
//! // Create trading gateway
//! let creds = Credentials::new("api_key", "api_secret")
//!     .with_passphrase("passphrase");
//! let mut trader = BitgetTrader::new_futures();
//! trader.login(&creds).await?;
//! ```

mod parser;
mod trader;
mod types;

pub use parser::BitgetParser;
pub use trader::BitgetTrader;
pub use types::*;
