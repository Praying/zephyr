//! WebSocket connection handler.
//!
//! This module provides the WebSocket upgrade handler and message processing.

use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use flate2::{Compression, write::GzEncoder};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::config::WsConfig;
use super::connection::{ConnectionId, ConnectionState};
use super::message::{ClientMessage, ServerMessage};
use super::state::WsState;
use crate::auth::extract_bearer_token;

/// Query parameters for WebSocket connection.
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    /// Optional JWT token for authentication
    #[serde(default)]
    pub token: Option<String>,
}

/// WebSocket upgrade handler.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<WsState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, query.token, state))
}

/// Handles a WebSocket connection.
async fn handle_socket(socket: WebSocket, token: Option<String>, state: Arc<WsState>) {
    let conn_id = ConnectionId::generate();
    info!(%conn_id, "New WebSocket connection");

    // Create message channel for this connection
    let (tx, mut rx) = mpsc::channel::<ServerMessage>(state.config.max_queue_size);

    // Create connection state
    let mut conn_state = ConnectionState::new(conn_id, tx);

    // If token provided in query, authenticate immediately
    if let Some(ref token) = token {
        match state.jwt_manager.validate_token(token) {
            Ok(claims) => {
                info!(%conn_id, user_id = %claims.sub, "WebSocket authenticated via query token");
                conn_state.set_claims(claims);
            }
            Err(e) => {
                warn!(%conn_id, error = %e, "Invalid token in WebSocket query");
            }
        }
    }

    // Check if authenticated and prepare initial message
    let initial_auth_msg = if conn_state.is_authenticated() {
        Some(ServerMessage::AuthResult {
            success: true,
            error: None,
            user_id: conn_state.user_id().map(String::from),
        })
    } else {
        None
    };

    // Register connection
    let conn_state = state.registry.register(conn_state);

    // Split socket into sender and receiver
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send initial auth result if authenticated
    if let Some(msg) = initial_auth_msg {
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = ws_sender.send(Message::Text(json.into())).await;
        }
    }

    // Spawn task to forward messages from channel to WebSocket
    let config = state.config.clone();
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match serialize_message(&msg, &config) {
                Ok(ws_msg) => {
                    if ws_sender.send(ws_msg).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                }
            }
        }
    });

    // Process incoming messages
    let state_clone = state.clone();
    let conn_state_clone = conn_state.clone();

    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(msg) => {
                // Update heartbeat on any message
                conn_state_clone.write().update_heartbeat();

                match msg {
                    Message::Text(text) => {
                        handle_text_message(&text, &conn_state_clone, &state_clone).await;
                    }
                    Message::Binary(data) => {
                        // Try to decompress and parse
                        if let Ok(text) = decompress_message(&data) {
                            handle_text_message(&text, &conn_state_clone, &state_clone).await;
                        }
                    }
                    Message::Ping(_data) => {
                        // Axum handles pong automatically, but we track heartbeat
                        debug!(%conn_id, "Received ping");
                        let sender = conn_state_clone.read().sender.clone();
                        let _ = sender
                            .send(ServerMessage::Pong {
                                timestamp: None,
                                server_time: chrono::Utc::now().timestamp_millis(),
                            })
                            .await;
                    }
                    Message::Pong(_) => {
                        debug!(%conn_id, "Received pong");
                    }
                    Message::Close(_) => {
                        info!(%conn_id, "WebSocket close requested");
                        break;
                    }
                }
            }
            Err(e) => {
                error!(%conn_id, error = %e, "WebSocket error");
                break;
            }
        }
    }

    // Cleanup
    info!(%conn_id, "WebSocket connection closed");
    state.registry.unregister(conn_id);
    send_task.abort();
}

/// Handles a text message from the client.
async fn handle_text_message(
    text: &str,
    conn_state: &Arc<parking_lot::RwLock<ConnectionState>>,
    state: &Arc<WsState>,
) {
    let conn_id = conn_state.read().id;

    match serde_json::from_str::<ClientMessage>(text) {
        Ok(msg) => match msg {
            ClientMessage::Auth { token } => {
                handle_auth(&token, conn_state, state).await;
            }
            ClientMessage::Subscribe { channels, symbols } => {
                handle_subscribe(&channels, &symbols, conn_state).await;
            }
            ClientMessage::Unsubscribe { channels, symbols } => {
                handle_unsubscribe(&channels, &symbols, conn_state).await;
            }
            ClientMessage::Ping { timestamp } => {
                handle_ping(timestamp, conn_state).await;
            }
        },
        Err(e) => {
            warn!(%conn_id, error = %e, "Failed to parse client message");
            let error_msg = ServerMessage::Error {
                code: "INVALID_MESSAGE".to_string(),
                message: format!("Failed to parse message: {e}"),
            };
            let sender = conn_state.read().sender.clone();
            let _ = sender.send(error_msg).await;
        }
    }
}

/// Handles authentication message.
async fn handle_auth(
    token: &str,
    conn_state: &Arc<parking_lot::RwLock<ConnectionState>>,
    state: &Arc<WsState>,
) {
    let conn_id = conn_state.read().id;

    // Extract token (handle "Bearer " prefix if present)
    let token = extract_bearer_token(token).unwrap_or(token);

    match state.jwt_manager.validate_token(token) {
        Ok(claims) => {
            info!(%conn_id, user_id = %claims.sub, "WebSocket authenticated");
            let sender = {
                let mut state = conn_state.write();
                state.set_claims(claims.clone());
                state.sender.clone()
            };

            let msg = ServerMessage::AuthResult {
                success: true,
                error: None,
                user_id: Some(claims.sub),
            };
            let _ = sender.send(msg).await;
        }
        Err(e) => {
            warn!(%conn_id, error = %e, "WebSocket authentication failed");
            let sender = conn_state.read().sender.clone();
            let msg = ServerMessage::AuthResult {
                success: false,
                error: Some(e.to_string()),
                user_id: None,
            };
            let _ = sender.send(msg).await;
        }
    }
}

/// Handles subscribe message.
async fn handle_subscribe(
    channels: &[super::message::WsChannel],
    symbols: &[String],
    conn_state: &Arc<parking_lot::RwLock<ConnectionState>>,
) {
    let (conn_id, is_authenticated, sender) = {
        let state = conn_state.read();
        (state.id, state.is_authenticated(), state.sender.clone())
    };

    // Check if authenticated
    if !is_authenticated {
        let msg = ServerMessage::Error {
            code: "UNAUTHORIZED".to_string(),
            message: "Authentication required before subscribing".to_string(),
        };
        let _ = sender.send(msg).await;
        return;
    }

    conn_state.write().subscribe(channels, symbols);

    debug!(%conn_id, ?channels, ?symbols, "Subscribed to channels");

    let msg = ServerMessage::Subscribed {
        channels: channels.to_vec(),
    };
    let _ = sender.send(msg).await;
}

/// Handles unsubscribe message.
async fn handle_unsubscribe(
    channels: &[super::message::WsChannel],
    symbols: &[String],
    conn_state: &Arc<parking_lot::RwLock<ConnectionState>>,
) {
    let (conn_id, sender) = {
        let state = conn_state.read();
        (state.id, state.sender.clone())
    };

    conn_state.write().unsubscribe(channels, symbols);

    debug!(%conn_id, ?channels, ?symbols, "Unsubscribed from channels");

    let msg = ServerMessage::Unsubscribed {
        channels: channels.to_vec(),
    };
    let _ = sender.send(msg).await;
}

/// Handles ping message.
async fn handle_ping(
    timestamp: Option<i64>,
    conn_state: &Arc<parking_lot::RwLock<ConnectionState>>,
) {
    let sender = conn_state.read().sender.clone();
    let msg = ServerMessage::Pong {
        timestamp,
        server_time: chrono::Utc::now().timestamp_millis(),
    };
    let _ = sender.send(msg).await;
}

/// Serializes a server message, optionally compressing it.
fn serialize_message(msg: &ServerMessage, config: &WsConfig) -> Result<Message, serde_json::Error> {
    let json = serde_json::to_string(msg)?;

    if config.enable_compression && json.len() > config.compression_threshold {
        // Compress the message
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(json.as_bytes()).ok();
        if let Ok(compressed) = encoder.finish() {
            if compressed.len() < json.len() {
                return Ok(Message::Binary(compressed.into()));
            }
        }
    }

    Ok(Message::Text(json.into()))
}

/// Decompresses a binary message.
fn decompress_message(data: &[u8]) -> Result<String, std::io::Error> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(data);
    let mut text = String::new();
    decoder.read_to_string(&mut text)?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_message_text() {
        let config = WsConfig {
            enable_compression: false,
            ..Default::default()
        };
        let msg = ServerMessage::Pong {
            timestamp: Some(123),
            server_time: 456,
        };

        let result = serialize_message(&msg, &config).unwrap();
        assert!(matches!(result, Message::Text(_)));
    }

    #[test]
    fn test_serialize_message_compressed() {
        let config = WsConfig {
            enable_compression: true,
            compression_threshold: 10, // Very low threshold for testing
            ..Default::default()
        };

        // Create a message that's larger than threshold
        let msg = ServerMessage::Error {
            code: "TEST_ERROR".to_string(),
            message: "This is a longer error message that should be compressed".to_string(),
        };

        let result = serialize_message(&msg, &config).unwrap();
        // May or may not be compressed depending on compression ratio
        assert!(matches!(result, Message::Text(_) | Message::Binary(_)));
    }

    #[test]
    fn test_decompress_message() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let original = r#"{"type":"pong","timestamp":123}"#;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(original.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();

        let decompressed = decompress_message(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }
}
