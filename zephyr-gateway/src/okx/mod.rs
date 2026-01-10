//! OKX exchange adapter.
//!
//! This module provides adapters for OKX exchange:
//! - [`OkxParser`] - Market data parser for WebSocket streams
//! - [`OkxTrader`] - Trading gateway for REST/WebSocket API
//!
//! # Supported Markets
//!
//! - Spot trading
//! - USDT-margined perpetual swaps
//! - Coin-margined perpetual swaps
//! - Futures (delivery)
//!
//! # OKX-Specific Features
//!
//! - Requires passphrase in addition to API key and secret
//! - Uses different trade modes: cash (spot), cross, isolated
//! - Supports position side for hedge mode
//! - Simulated trading mode available
//!
//! # Example
//!
//! ```ignore
//! use zephyr_gateway::okx::{OkxParser, OkxTrader};
//! use zephyr_core::traits::{ParserConfig, Credentials};
//!
//! // Create market data parser
//! let config = ParserConfig::builder()
//!     .endpoint("wss://ws.okx.com:8443/ws/v5/public")
//!     .exchange("okx")
//!     .build();
//!
//! let mut parser = OkxParser::new();
//! parser.init(&config).await?;
//! parser.connect().await?;
//!
//! // Create trading gateway
//! let creds = Credentials::new("api_key", "api_secret")
//!     .with_passphrase("passphrase");
//! let mut trader = OkxTrader::new_swap();
//! trader.login(&creds).await?;
//! ```

mod parser;
mod trader;
mod types;

pub use parser::OkxParser;
pub use trader::OkxTrader;
pub use types::*;
