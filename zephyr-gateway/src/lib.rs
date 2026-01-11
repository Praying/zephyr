//! # Zephyr Gateway
//!
//! Exchange adapters and network communication for the Zephyr trading system.
//!
//! This crate provides:
//! - WebSocket client with automatic reconnection and heartbeat
//! - REST client with rate limiting and request signing
//! - Exchange-specific adapters (Binance, OKX, Bitget, Hyperliquid)
//!
//! # Architecture
//!
//! The gateway module is organized into:
//! - `ws` - WebSocket client infrastructure
//! - `rest` - REST client infrastructure
//! - Exchange-specific modules (binance, okx, etc.)
//!
//! # Example
//!
//! ```ignore
//! use zephyr_gateway::ws::{WebSocketClient, WebSocketConfig};
//!
//! let config = WebSocketConfig::builder()
//!     .url("wss://stream.binance.com:9443/ws")
//!     .reconnect_enabled(true)
//!     .build();
//!
//! let client = WebSocketClient::new(config);
//! client.connect().await?;
//! ```

#![warn(missing_docs)]
#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(clippy::cargo)]
#![allow(clippy::nursery)]

/// WebSocket client infrastructure
pub mod ws;

/// REST client infrastructure
pub mod rest;

/// Binance exchange adapter
#[cfg(feature = "binance")]
pub mod binance;

/// OKX exchange adapter
#[cfg(feature = "okx")]
pub mod okx;

/// Bitget exchange adapter
#[cfg(feature = "bitget")]
pub mod bitget;

/// Hyperliquid DEX adapter
#[cfg(feature = "hyperliquid")]
pub mod hyperliquid;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::rest::{RestClient, RestConfig, RestConfigBuilder};
    pub use crate::ws::{
        WebSocketCallback, WebSocketClient, WebSocketConfig, WebSocketConfigBuilder,
        WebSocketMessage,
    };

    #[cfg(feature = "binance")]
    pub use crate::binance::{BinanceMarket, BinanceParser, BinanceTrader};

    #[cfg(feature = "okx")]
    pub use crate::okx::{OkxMarket, OkxParser, OkxTrader};

    #[cfg(feature = "bitget")]
    pub use crate::bitget::{BitgetMarket, BitgetParser, BitgetTrader};

    #[cfg(feature = "hyperliquid")]
    pub use crate::hyperliquid::{HyperliquidMarket, HyperliquidParser, HyperliquidTrader};
}
