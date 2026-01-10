//! WebSocket server module.
//!
//! This module provides WebSocket functionality for real-time data streaming:
//! - Connection management with JWT authentication
//! - Channel-based subscription system
//! - Heartbeat mechanism for connection health
//! - Message compression for bandwidth optimization
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    WebSocket Server                          │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
//! │  │ Connection  │  │ Connection  │  │ Connection  │  ...    │
//! │  │     #1      │  │     #2      │  │     #3      │         │
//! │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
//! │         │                │                │                 │
//! │         └────────────────┼────────────────┘                 │
//! │                          ▼                                  │
//! │  ┌───────────────────────────────────────────────────────┐ │
//! │  │              Connection Registry                       │ │
//! │  │  - Track all active connections                        │ │
//! │  │  - Manage subscriptions per connection                 │ │
//! │  │  - Broadcast messages to subscribers                   │ │
//! │  └───────────────────────────────────────────────────────┘ │
//! │                          │                                  │
//! │                          ▼                                  │
//! │  ┌───────────────────────────────────────────────────────┐ │
//! │  │                   Broadcaster                          │ │
//! │  │  - Position updates                                    │ │
//! │  │  - Order updates                                       │ │
//! │  │  - PnL updates                                         │ │
//! │  │  - Market data                                         │ │
//! │  │  - Strategy status                                     │ │
//! │  │  - Risk alerts                                         │ │
//! │  └───────────────────────────────────────────────────────┘ │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Channels
//!
//! - `positions` - Real-time position updates
//! - `orders` - Order status changes (new, filled, canceled)
//! - `pnl` - Periodic PnL updates
//! - `market_data` - Tick data for subscribed symbols
//! - `strategies` - Strategy status changes
//! - `risk_alerts` - Risk limit breaches and alerts
//! - `logs` - Real-time log streaming
//!
//! # Authentication
//!
//! Connections must authenticate using JWT tokens either:
//! - Via query parameter: `ws://host/ws?token=<jwt>`
//! - Via auth message after connection: `{"type":"auth","token":"<jwt>"}`
//!
//! # Example Client Usage
//!
//! ```javascript
//! const ws = new WebSocket('ws://localhost:8080/ws?token=<jwt>');
//!
//! ws.onopen = () => {
//!     // Subscribe to channels
//!     ws.send(JSON.stringify({
//!         type: 'subscribe',
//!         channels: ['positions', 'orders', 'pnl'],
//!         symbols: ['BTC-USDT', 'ETH-USDT']
//!     }));
//! };
//!
//! ws.onmessage = (event) => {
//!     const msg = JSON.parse(event.data);
//!     switch (msg.type) {
//!         case 'position_update':
//!             console.log('Position:', msg.data);
//!             break;
//!         case 'order_update':
//!             console.log('Order:', msg.data);
//!             break;
//!         case 'pnl_update':
//!             console.log('PnL:', msg.data);
//!             break;
//!     }
//! };
//!
//! // Send ping for heartbeat
//! setInterval(() => {
//!     ws.send(JSON.stringify({ type: 'ping', timestamp: Date.now() }));
//! }, 30000);
//! ```

pub mod broadcaster;
pub mod config;
pub mod connection;
pub mod handler;
pub mod message;
pub mod state;

pub use broadcaster::WsBroadcaster;
pub use config::WsConfig;
pub use connection::{ConnectionId, ConnectionRegistry, ConnectionState};
pub use handler::ws_handler;
pub use message::{ClientMessage, ServerMessage, WsChannel};
pub use state::WsState;
