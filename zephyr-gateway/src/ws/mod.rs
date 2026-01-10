//! WebSocket client infrastructure.
//!
//! This module provides a robust WebSocket client with:
//! - Automatic reconnection with exponential backoff
//! - Heartbeat/ping-pong mechanism
//! - Message serialization/deserialization
//! - Connection state management
//!
//! # Example
//!
//! ```ignore
//! use zephyr_gateway::ws::{WebSocketClient, WebSocketConfig, WebSocketCallback};
//!
//! struct MyCallback;
//!
//! #[async_trait::async_trait]
//! impl WebSocketCallback for MyCallback {
//!     async fn on_message(&self, message: String) {
//!         println!("Received: {}", message);
//!     }
//!     // ... other methods
//! }
//!
//! let config = WebSocketConfig::builder()
//!     .url("wss://example.com/ws")
//!     .reconnect_enabled(true)
//!     .build();
//!
//! let mut client = WebSocketClient::new(config);
//! client.set_callback(Box::new(MyCallback));
//! client.connect().await?;
//! ```

mod client;
mod config;
mod message;
mod state;

pub use client::{WebSocketCallback, WebSocketClient};
pub use config::{WebSocketConfig, WebSocketConfigBuilder};
pub use message::{MessageCodec, WebSocketMessage};
pub use state::ConnectionState;
