//! Hyperliquid DEX adapter.
//!
//! This module provides adapters for Hyperliquid decentralized exchange:
//! - [`HyperliquidParser`] - Market data parser for WebSocket streams
//! - [`HyperliquidTrader`] - Trading gateway for REST/WebSocket API
//!
//! # DEX-Specific Features
//!
//! Hyperliquid is a decentralized perpetual exchange built on its own L1 blockchain.
//! Key differences from centralized exchanges:
//!
//! - **Wallet Authentication**: Uses private key signing instead of API keys
//! - **Nonce Management**: Transactions require sequential nonces
//! - **On-chain Confirmation**: Orders are confirmed on-chain
//! - **Vault Trading**: Supports institutional vault-based trading
//!
//! # Supported Markets
//!
//! - Perpetual futures (USD-denominated)
//! - Spot trading
//!
//! # Example
//!
//! ```ignore
//! use zephyr_gateway::hyperliquid::{HyperliquidParser, HyperliquidTrader};
//! use zephyr_core::traits::{ParserConfig, Credentials};
//!
//! // Create market data parser
//! let config = ParserConfig::builder()
//!     .endpoint("wss://api.hyperliquid.xyz/ws")
//!     .exchange("hyperliquid")
//!     .build();
//!
//! let mut parser = HyperliquidParser::new();
//! parser.init(&config).await?;
//! parser.connect().await?;
//!
//! // Create trading gateway
//! // For Hyperliquid: api_key = wallet address, api_secret = private key
//! let creds = Credentials::new("0x...", "private_key_hex");
//! let mut trader = HyperliquidTrader::new();
//! trader.login(&creds).await?;
//! ```

mod parser;
mod trader;
mod types;

pub use parser::HyperliquidParser;
pub use trader::HyperliquidTrader;
pub use types::*;
