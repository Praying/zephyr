//! WebSocket message types and codec.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use zephyr_core::error::NetworkError;

/// WebSocket message types.
#[derive(Debug, Clone)]
pub enum WebSocketMessage {
    /// Text message.
    Text(String),
    /// Binary message.
    Binary(Vec<u8>),
    /// Ping frame.
    Ping(Vec<u8>),
    /// Pong frame.
    Pong(Vec<u8>),
    /// Close frame.
    Close(Option<CloseReason>),
}

/// Close frame reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseReason {
    /// Close code.
    pub code: u16,
    /// Close reason text.
    pub reason: String,
}

impl WebSocketMessage {
    /// Creates a text message.
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text(content.into())
    }

    /// Creates a binary message.
    #[must_use]
    pub fn binary(data: impl Into<Vec<u8>>) -> Self {
        Self::Binary(data.into())
    }

    /// Creates a ping message.
    #[must_use]
    pub fn ping(data: impl Into<Vec<u8>>) -> Self {
        Self::Ping(data.into())
    }

    /// Creates a pong message.
    #[must_use]
    pub fn pong(data: impl Into<Vec<u8>>) -> Self {
        Self::Pong(data.into())
    }

    /// Creates a close message.
    #[must_use]
    pub fn close(code: u16, reason: impl Into<String>) -> Self {
        Self::Close(Some(CloseReason {
            code,
            reason: reason.into(),
        }))
    }

    /// Returns true if this is a text message.
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }

    /// Returns true if this is a binary message.
    #[must_use]
    pub fn is_binary(&self) -> bool {
        matches!(self, Self::Binary(_))
    }

    /// Returns true if this is a ping message.
    #[must_use]
    pub fn is_ping(&self) -> bool {
        matches!(self, Self::Ping(_))
    }

    /// Returns true if this is a pong message.
    #[must_use]
    pub fn is_pong(&self) -> bool {
        matches!(self, Self::Pong(_))
    }

    /// Returns true if this is a close message.
    #[must_use]
    pub fn is_close(&self) -> bool {
        matches!(self, Self::Close(_))
    }

    /// Returns the text content if this is a text message.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the binary content if this is a binary message.
    #[must_use]
    pub fn as_binary(&self) -> Option<&[u8]> {
        match self {
            Self::Binary(b) => Some(b),
            _ => None,
        }
    }

    /// Converts the message to bytes for sending.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            Self::Text(s) => s.into_bytes(),
            Self::Binary(b) | Self::Ping(b) | Self::Pong(b) => b,
            Self::Close(_) => Vec::new(),
        }
    }
}

/// Message codec for serialization/deserialization.
///
/// Provides methods to encode and decode messages to/from JSON.
#[derive(Debug, Clone, Default)]
pub struct MessageCodec {
    /// Whether to use pretty printing for JSON.
    pub pretty: bool,
}

impl MessageCodec {
    /// Creates a new message codec.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a codec with pretty printing enabled.
    #[must_use]
    pub fn pretty() -> Self {
        Self { pretty: true }
    }

    /// Encodes a value to a JSON text message.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if serialization fails.
    pub fn encode<T: Serialize>(&self, value: &T) -> Result<WebSocketMessage, NetworkError> {
        let json = if self.pretty {
            serde_json::to_string_pretty(value)
        } else {
            serde_json::to_string(value)
        }
        .map_err(|e| NetworkError::WebSocket {
            reason: format!("Failed to serialize message: {e}"),
        })?;

        Ok(WebSocketMessage::Text(json))
    }

    /// Decodes a text message to a value.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if deserialization fails.
    pub fn decode<T: DeserializeOwned>(
        &self,
        message: &WebSocketMessage,
    ) -> Result<T, NetworkError> {
        match message {
            WebSocketMessage::Text(text) => {
                serde_json::from_str(text).map_err(|e| NetworkError::WebSocket {
                    reason: format!("Failed to deserialize message: {e}"),
                })
            }
            WebSocketMessage::Binary(data) => {
                serde_json::from_slice(data).map_err(|e| NetworkError::WebSocket {
                    reason: format!("Failed to deserialize binary message: {e}"),
                })
            }
            _ => Err(NetworkError::WebSocket {
                reason: "Cannot decode non-data message".to_string(),
            }),
        }
    }

    /// Decodes a JSON string directly.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if deserialization fails.
    pub fn decode_str<T: DeserializeOwned>(&self, json: &str) -> Result<T, NetworkError> {
        serde_json::from_str(json).map_err(|e| NetworkError::WebSocket {
            reason: format!("Failed to deserialize JSON: {e}"),
        })
    }

    /// Encodes a value to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if serialization fails.
    pub fn encode_str<T: Serialize>(&self, value: &T) -> Result<String, NetworkError> {
        let json = if self.pretty {
            serde_json::to_string_pretty(value)
        } else {
            serde_json::to_string(value)
        }
        .map_err(|e| NetworkError::WebSocket {
            reason: format!("Failed to serialize to JSON: {e}"),
        })?;

        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestMessage {
        id: u64,
        data: String,
    }

    #[test]
    fn test_message_types() {
        let text = WebSocketMessage::text("hello");
        assert!(text.is_text());
        assert_eq!(text.as_text(), Some("hello"));

        let binary = WebSocketMessage::binary(vec![1, 2, 3]);
        assert!(binary.is_binary());
        assert_eq!(binary.as_binary(), Some(&[1, 2, 3][..]));

        let ping = WebSocketMessage::ping(vec![]);
        assert!(ping.is_ping());

        let pong_msg = WebSocketMessage::pong(vec![]);
        assert!(pong_msg.is_pong());

        let close = WebSocketMessage::close(1000, "normal");
        assert!(close.is_close());
    }

    #[test]
    fn test_codec_roundtrip() {
        let codec = MessageCodec::new();
        let original = TestMessage {
            id: 42,
            data: "test".to_string(),
        };

        let encoded = codec.encode(&original).unwrap();
        let decoded: TestMessage = codec.decode(&encoded).unwrap();

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_codec_str_roundtrip() {
        let codec = MessageCodec::new();
        let original = TestMessage {
            id: 123,
            data: "hello world".to_string(),
        };

        let json = codec.encode_str(&original).unwrap();
        let decoded: TestMessage = codec.decode_str(&json).unwrap();

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_codec_decode_error() {
        let codec = MessageCodec::new();
        let invalid = WebSocketMessage::text("not valid json");
        let result: Result<TestMessage, _> = codec.decode(&invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_into_bytes() {
        let text = WebSocketMessage::text("hello");
        assert_eq!(text.into_bytes(), b"hello".to_vec());

        let binary = WebSocketMessage::binary(vec![1, 2, 3]);
        assert_eq!(binary.into_bytes(), vec![1, 2, 3]);
    }
}
