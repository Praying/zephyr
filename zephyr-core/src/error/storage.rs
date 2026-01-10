//! Storage-related error types.
//!
//! This module provides error types for storage operations including
//! I/O failures, serialization errors, and capacity limits.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Storage error type covering I/O failures, serialization errors,
/// and capacity limits.
///
/// # Examples
///
/// ```
/// use zephyr_core::error::StorageError;
///
/// let error = StorageError::IoError {
///     operation: "write".to_string(),
///     path: "/data/klines.bin".to_string(),
///     reason: "Disk full".to_string(),
/// };
/// assert!(error.to_string().contains("write"));
/// ```
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageError {
    /// I/O operation failed.
    #[error("[Storage] I/O error during {operation} on '{path}': {reason}")]
    IoError {
        /// Operation that failed (read, write, delete, etc.).
        operation: String,
        /// Path to the file or resource.
        path: String,
        /// Reason for the I/O error.
        reason: String,
    },

    /// Serialization failed.
    #[error("[Storage] Serialization error: {reason}")]
    SerializationError {
        /// Reason for the serialization error.
        reason: String,
    },

    /// Deserialization failed.
    #[error("[Storage] Deserialization error: {reason}")]
    DeserializationError {
        /// Reason for the deserialization error.
        reason: String,
    },

    /// Storage capacity limit exceeded.
    #[error("[Storage] Capacity exceeded: {current} / {limit} {unit}")]
    CapacityExceeded {
        /// Current usage.
        current: u64,
        /// Maximum limit.
        limit: u64,
        /// Unit of measurement (bytes, records, etc.).
        unit: String,
    },

    /// File or resource not found.
    #[error("[Storage] Not found: {path}")]
    NotFound {
        /// Path to the missing file or resource.
        path: String,
    },

    /// File or resource already exists.
    #[error("[Storage] Already exists: {path}")]
    AlreadyExists {
        /// Path to the existing file or resource.
        path: String,
    },

    /// Permission denied.
    #[error("[Storage] Permission denied: {path}")]
    PermissionDenied {
        /// Path to the file or resource.
        path: String,
    },

    /// Storage is locked.
    #[error("[Storage] Resource locked: {path}")]
    Locked {
        /// Path to the locked resource.
        path: String,
    },

    /// Compression error.
    #[error("[Storage] Compression error: {reason}")]
    CompressionError {
        /// Reason for the compression error.
        reason: String,
    },

    /// Decompression error.
    #[error("[Storage] Decompression error: {reason}")]
    DecompressionError {
        /// Reason for the decompression error.
        reason: String,
    },

    /// Memory mapping error.
    #[error("[Storage] Memory mapping error for '{path}': {reason}")]
    MmapError {
        /// Path to the file being mapped.
        path: String,
        /// Reason for the mapping error.
        reason: String,
    },

    /// Database error.
    #[error("[Storage] Database error: {reason}")]
    DatabaseError {
        /// Reason for the database error.
        reason: String,
    },

    /// Transaction failed.
    #[error("[Storage] Transaction failed: {reason}")]
    TransactionFailed {
        /// Reason for the transaction failure.
        reason: String,
    },

    /// Index corruption detected.
    #[error("[Storage] Index corruption in '{path}': {reason}")]
    IndexCorruption {
        /// Path to the corrupted index.
        path: String,
        /// Reason or description of the corruption.
        reason: String,
    },
}

impl StorageError {
    /// Returns true if this error is recoverable.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::Locked { .. } | Self::CapacityExceeded { .. })
    }

    /// Returns the severity level of this error.
    #[must_use]
    pub fn severity(&self) -> super::ErrorSeverity {
        use super::ErrorSeverity;
        match self {
            Self::IndexCorruption { .. }
            | Self::TransactionFailed { .. }
            | Self::DatabaseError { .. } => ErrorSeverity::Fatal,
            Self::Locked { .. } | Self::CapacityExceeded { .. } => ErrorSeverity::Recoverable,
            Self::IoError { .. }
            | Self::SerializationError { .. }
            | Self::DeserializationError { .. }
            | Self::CompressionError { .. }
            | Self::DecompressionError { .. }
            | Self::MmapError { .. }
            | Self::PermissionDenied { .. } => ErrorSeverity::Warning,
            Self::NotFound { .. } | Self::AlreadyExists { .. } => ErrorSeverity::Info,
        }
    }

    /// Returns a suggested retry delay in milliseconds, if applicable.
    #[must_use]
    pub fn suggested_retry_delay_ms(&self) -> Option<u64> {
        match self {
            Self::Locked { .. } => Some(100),
            Self::CapacityExceeded { .. } => Some(5000),
            _ => None,
        }
    }

    /// Creates an I/O read error.
    #[must_use]
    pub fn read_error(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::IoError {
            operation: "read".to_string(),
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Creates an I/O write error.
    #[must_use]
    pub fn write_error(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::IoError {
            operation: "write".to_string(),
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Creates a not found error.
    #[must_use]
    pub fn not_found(path: impl Into<String>) -> Self {
        Self::NotFound { path: path.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_error() {
        let error = StorageError::IoError {
            operation: "write".to_string(),
            path: "/data/klines.bin".to_string(),
            reason: "Disk full".to_string(),
        };
        assert!(error.to_string().contains("write"));
        assert!(error.to_string().contains("klines.bin"));
    }

    #[test]
    fn test_capacity_exceeded() {
        let error = StorageError::CapacityExceeded {
            current: 1024,
            limit: 512,
            unit: "MB".to_string(),
        };
        assert!(error.to_string().contains("1024"));
        assert!(error.to_string().contains("512"));
        assert!(error.to_string().contains("MB"));
    }

    #[test]
    fn test_is_recoverable() {
        let locked = StorageError::Locked {
            path: "/data/lock".to_string(),
        };
        assert!(locked.is_recoverable());

        let not_found = StorageError::NotFound {
            path: "/data/missing".to_string(),
        };
        assert!(!not_found.is_recoverable());
    }

    #[test]
    fn test_helper_methods() {
        let error = StorageError::read_error("/data/file.bin", "File corrupted");
        assert!(matches!(
            error,
            StorageError::IoError {
                operation,
                ..
            } if operation == "read"
        ));

        let error = StorageError::write_error("/data/file.bin", "No space");
        assert!(matches!(
            error,
            StorageError::IoError {
                operation,
                ..
            } if operation == "write"
        ));

        let error = StorageError::not_found("/data/missing.bin");
        assert!(matches!(error, StorageError::NotFound { .. }));
    }

    #[test]
    fn test_serde_roundtrip() {
        let error = StorageError::DatabaseError {
            reason: "Connection lost".to_string(),
        };
        let json = serde_json::to_string(&error).unwrap();
        let parsed: StorageError = serde_json::from_str(&json).unwrap();
        assert_eq!(error, parsed);
    }
}
