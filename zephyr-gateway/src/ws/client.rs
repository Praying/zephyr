//! WebSocket client implementation with automatic reconnection and heartbeat.

#![allow(clippy::unused_async)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::future_not_send)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::too_many_lines)]

use async_trait::async_trait;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{interval, timeout};
use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tracing::{debug, error, info, warn};
use zephyr_core::error::NetworkError;

use super::config::WebSocketConfig;
use super::message::WebSocketMessage;
use super::state::{ConnectionState, InternalState};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, TungsteniteMessage>;
type WsSource = SplitStream<WsStream>;

/// Callback trait for WebSocket events.
#[async_trait]
pub trait WebSocketCallback: Send + Sync {
    /// Called when a message is received.
    async fn on_message(&self, message: WebSocketMessage);

    /// Called when the connection is established.
    async fn on_connected(&self);

    /// Called when the connection is lost.
    async fn on_disconnected(&self, reason: Option<String>);

    /// Called when an error occurs.
    async fn on_error(&self, error: NetworkError);

    /// Called when a reconnection attempt is made.
    async fn on_reconnecting(&self, attempt: u32, max_attempts: u32) {
        let _ = (attempt, max_attempts);
    }
}

/// WebSocket client with automatic reconnection and heartbeat.
///
/// # Features
///
/// - Automatic reconnection with exponential backoff
/// - Heartbeat/ping-pong mechanism
/// - Message serialization/deserialization
/// - Thread-safe state management
///
/// # Example
///
/// ```ignore
/// use zephyr_gateway::ws::{WebSocketClient, WebSocketConfig, WebSocketCallback};
///
/// let config = WebSocketConfig::builder()
///     .url("wss://example.com/ws")
///     .reconnect_enabled(true)
///     .build();
///
/// let mut client = WebSocketClient::new(config);
/// client.connect().await?;
/// ```
pub struct WebSocketClient {
    config: WebSocketConfig,
    state: Arc<RwLock<InternalState>>,
    callback: Option<Arc<dyn WebSocketCallback>>,
    send_tx: Option<mpsc::Sender<WebSocketMessage>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl WebSocketClient {
    /// Creates a new WebSocket client with the given configuration.
    #[must_use]
    pub fn new(config: WebSocketConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(InternalState::new())),
            callback: None,
            send_tx: None,
            shutdown_tx: None,
        }
    }

    /// Sets the callback for receiving events.
    pub fn set_callback(&mut self, callback: impl WebSocketCallback + 'static) {
        self.callback = Some(Arc::new(callback));
    }

    /// Returns the current connection state.
    #[must_use]
    pub fn state(&self) -> ConnectionState {
        self.state.read().state
    }

    /// Returns whether the client is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.state.read().state.is_connected()
    }

    /// Returns the number of reconnection attempts.
    #[must_use]
    pub fn reconnect_attempts(&self) -> u32 {
        self.state.read().reconnect_attempts
    }

    /// Connects to the WebSocket server.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if connection fails.
    pub async fn connect(&mut self) -> Result<(), NetworkError> {
        if self.is_connected() {
            return Ok(());
        }

        self.state.write().state = ConnectionState::Connecting;

        let (ws_stream, _) = timeout(
            self.config.connect_timeout(),
            connect_async(&self.config.url),
        )
        .await
        .map_err(|_| NetworkError::Timeout {
            timeout_ms: self.config.connect_timeout_ms,
        })?
        .map_err(|e| NetworkError::ConnectionFailed {
            reason: e.to_string(),
        })?;

        self.setup_connection(ws_stream).await;
        self.state.write().mark_connected();

        if let Some(callback) = &self.callback {
            callback.on_connected().await;
        }

        info!(
            exchange = %self.config.exchange,
            url = %self.config.url,
            "WebSocket connected"
        );

        Ok(())
    }

    /// Disconnects from the WebSocket server.
    pub async fn disconnect(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }

        self.send_tx = None;
        self.state.write().mark_closed();

        if let Some(callback) = &self.callback {
            callback
                .on_disconnected(Some("Client disconnected".to_string()))
                .await;
        }

        info!(
            exchange = %self.config.exchange,
            "WebSocket disconnected"
        );
    }

    /// Sends a message to the server.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if sending fails.
    pub async fn send(&self, message: WebSocketMessage) -> Result<(), NetworkError> {
        let send_tx = self
            .send_tx
            .as_ref()
            .ok_or(NetworkError::ConnectionClosed {
                reason: "Not connected".to_string(),
            })?;

        send_tx
            .send(message)
            .await
            .map_err(|_| NetworkError::ConnectionClosed {
                reason: "Send channel closed".to_string(),
            })
    }

    /// Sends a text message to the server.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if sending fails.
    pub async fn send_text(&self, text: impl Into<String>) -> Result<(), NetworkError> {
        self.send(WebSocketMessage::Text(text.into())).await
    }

    /// Sends a JSON message to the server.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if serialization or sending fails.
    pub async fn send_json<T: serde::Serialize>(&self, value: &T) -> Result<(), NetworkError> {
        let json = serde_json::to_string(value).map_err(|e| NetworkError::WebSocket {
            reason: format!("Failed to serialize: {e}"),
        })?;
        self.send_text(json).await
    }

    async fn setup_connection(&mut self, ws_stream: WsStream) {
        let (sink, stream) = ws_stream.split();

        let (send_tx, send_rx) = mpsc::channel::<WebSocketMessage>(100);
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

        self.send_tx = Some(send_tx.clone());
        self.shutdown_tx = Some(shutdown_tx);

        let state = Arc::clone(&self.state);
        let callback = self.callback.clone();
        let config = self.config.clone();

        // Spawn the message handling task
        tokio::spawn(Self::run_connection(
            sink,
            stream,
            send_rx,
            shutdown_rx,
            state,
            callback,
            config,
        ));
    }

    async fn run_connection(
        mut sink: WsSink,
        mut stream: WsSource,
        mut send_rx: mpsc::Receiver<WebSocketMessage>,
        mut shutdown_rx: mpsc::Receiver<()>,
        state: Arc<RwLock<InternalState>>,
        callback: Option<Arc<dyn WebSocketCallback>>,
        config: WebSocketConfig,
    ) {
        let mut heartbeat_interval = interval(config.heartbeat_interval());
        heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = shutdown_rx.recv() => {
                    debug!("Shutdown signal received");
                    let _ = sink.close().await;
                    break;
                }

                // Handle outgoing messages
                Some(msg) = send_rx.recv() => {
                    let tung_msg = Self::to_tungstenite_message(msg);
                    if let Err(e) = sink.send(tung_msg).await {
                        error!(error = %e, "Failed to send message");
                        if let Some(cb) = &callback {
                            cb.on_error(NetworkError::WebSocket {
                                reason: e.to_string(),
                            }).await;
                        }
                    }
                }

                // Handle incoming messages
                Some(result) = stream.next() => {
                    match result {
                        Ok(msg) => {
                            state.write().record_message();
                            if let Some(ws_msg) = Self::from_tungstenite_message(msg) {
                                // Handle pong internally
                                if ws_msg.is_pong() {
                                    state.write().record_pong();
                                    debug!("Pong received");
                                    continue;
                                }

                                // Handle ping - respond with pong
                                if let WebSocketMessage::Ping(data) = &ws_msg {
                                    let pong = TungsteniteMessage::Pong(data.clone());
                                    if let Err(e) = sink.send(pong).await {
                                        warn!(error = %e, "Failed to send pong");
                                    }
                                    continue;
                                }

                                // Handle close
                                if ws_msg.is_close() {
                                    info!("Server sent close frame");
                                    state.write().mark_disconnected();
                                    if let Some(cb) = &callback {
                                        cb.on_disconnected(Some("Server closed connection".to_string())).await;
                                    }
                                    break;
                                }

                                // Forward to callback
                                if let Some(cb) = &callback {
                                    cb.on_message(ws_msg).await;
                                }
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "WebSocket error");
                            state.write().mark_disconnected();
                            if let Some(cb) = &callback {
                                cb.on_error(NetworkError::WebSocket {
                                    reason: e.to_string(),
                                }).await;
                                cb.on_disconnected(Some(e.to_string())).await;
                            }
                            break;
                        }
                    }
                }

                // Handle heartbeat
                _ = heartbeat_interval.tick() => {
                    if config.auto_ping {
                        let ping_msg = if let Some(custom) = &config.custom_ping_message {
                            TungsteniteMessage::Text(custom.clone())
                        } else {
                            TungsteniteMessage::Ping(vec![])
                        };

                        state.write().record_ping();

                        if let Err(e) = sink.send(ping_msg).await {
                            warn!(error = %e, "Failed to send ping");
                        } else {
                            debug!("Ping sent");
                        }
                    }
                }
            }
        }
    }

    fn to_tungstenite_message(msg: WebSocketMessage) -> TungsteniteMessage {
        match msg {
            WebSocketMessage::Text(s) => TungsteniteMessage::Text(s),
            WebSocketMessage::Binary(b) => TungsteniteMessage::Binary(b),
            WebSocketMessage::Ping(b) => TungsteniteMessage::Ping(b),
            WebSocketMessage::Pong(b) => TungsteniteMessage::Pong(b),
            WebSocketMessage::Close(reason) => {
                if let Some(r) = reason {
                    TungsteniteMessage::Close(Some(
                        tokio_tungstenite::tungstenite::protocol::CloseFrame {
                            code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::from(r.code),
                            reason: r.reason.into(),
                        },
                    ))
                } else {
                    TungsteniteMessage::Close(None)
                }
            }
        }
    }

    fn from_tungstenite_message(msg: TungsteniteMessage) -> Option<WebSocketMessage> {
        match msg {
            TungsteniteMessage::Text(s) => Some(WebSocketMessage::Text(s)),
            TungsteniteMessage::Binary(b) => Some(WebSocketMessage::Binary(b)),
            TungsteniteMessage::Ping(b) => Some(WebSocketMessage::Ping(b)),
            TungsteniteMessage::Pong(b) => Some(WebSocketMessage::Pong(b)),
            TungsteniteMessage::Close(frame) => Some(WebSocketMessage::Close(frame.map(|f| {
                super::message::CloseReason {
                    code: f.code.into(),
                    reason: f.reason.to_string(),
                }
            }))),
            TungsteniteMessage::Frame(_) => None,
        }
    }
}

/// Reconnecting WebSocket client wrapper.
///
/// Wraps a `WebSocketClient` and provides automatic reconnection logic.
#[allow(dead_code)]
pub struct ReconnectingWebSocketClient {
    client: WebSocketClient,
}

#[allow(dead_code)]
impl ReconnectingWebSocketClient {
    /// Creates a new reconnecting WebSocket client.
    #[must_use]
    pub fn new(config: WebSocketConfig) -> Self {
        Self {
            client: WebSocketClient::new(config),
        }
    }

    /// Sets the callback for receiving events.
    pub fn set_callback(&mut self, callback: impl WebSocketCallback + 'static) {
        self.client.set_callback(callback);
    }

    /// Returns the inner client.
    #[must_use]
    pub fn inner(&self) -> &WebSocketClient {
        &self.client
    }

    /// Returns the inner client mutably.
    pub fn inner_mut(&mut self) -> &mut WebSocketClient {
        &mut self.client
    }

    /// Connects with automatic reconnection on failure.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if all reconnection attempts fail.
    pub async fn connect(&mut self) -> Result<(), NetworkError> {
        let mut attempt = 0u32;

        loop {
            match self.client.connect().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if !self.client.config.should_reconnect(attempt) {
                        return Err(e);
                    }

                    let delay = self.client.config.calculate_reconnect_delay(attempt);
                    warn!(
                        attempt = attempt + 1,
                        max_attempts = self.client.config.max_reconnect_attempts,
                        delay_ms = delay.as_millis(),
                        error = %e,
                        "Connection failed, reconnecting"
                    );

                    if let Some(callback) = &self.client.callback {
                        callback
                            .on_reconnecting(attempt + 1, self.client.config.max_reconnect_attempts)
                            .await;
                    }

                    self.client.state.write().mark_reconnecting();
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }

    /// Sends a message.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if sending fails.
    pub async fn send(&self, message: WebSocketMessage) -> Result<(), NetworkError> {
        self.client.send(message).await
    }

    /// Sends a text message.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if sending fails.
    pub async fn send_text(&self, text: impl Into<String>) -> Result<(), NetworkError> {
        self.client.send_text(text).await
    }

    /// Disconnects from the server.
    pub async fn disconnect(&mut self) {
        self.client.disconnect().await;
    }

    /// Returns whether the client is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.client.is_connected()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = WebSocketConfig::builder()
            .url("wss://example.com/ws")
            .exchange("test")
            .build();

        let client = WebSocketClient::new(config);
        assert!(!client.is_connected());
        assert_eq!(client.state(), ConnectionState::Disconnected);
    }

    #[test]
    fn test_message_conversion() {
        // Test text message
        let text = WebSocketMessage::text("hello");
        let tung = WebSocketClient::to_tungstenite_message(text);
        assert!(matches!(tung, TungsteniteMessage::Text(_)));

        // Test binary message
        let binary = WebSocketMessage::binary(vec![1, 2, 3]);
        let tung = WebSocketClient::to_tungstenite_message(binary);
        assert!(matches!(tung, TungsteniteMessage::Binary(_)));

        // Test ping message
        let ping = WebSocketMessage::ping(vec![]);
        let tung = WebSocketClient::to_tungstenite_message(ping);
        assert!(matches!(tung, TungsteniteMessage::Ping(_)));
    }

    #[test]
    fn test_from_tungstenite_message() {
        let text = TungsteniteMessage::Text("hello".to_string());
        let ws_msg = WebSocketClient::from_tungstenite_message(text);
        assert!(ws_msg.is_some());
        assert!(ws_msg.unwrap().is_text());

        let binary = TungsteniteMessage::Binary(vec![1, 2, 3]);
        let ws_msg = WebSocketClient::from_tungstenite_message(binary);
        assert!(ws_msg.is_some());
        assert!(ws_msg.unwrap().is_binary());
    }
}
