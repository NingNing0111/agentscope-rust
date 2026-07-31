//! Error types for the embedding model layer.

use std::fmt;

/// Errors that can occur when using an embedding model.
#[derive(Debug, Clone)]
pub enum EmbeddingError {
    /// API key not configured.
    ApiKeyMissing(String),
    /// HTTP request failed.
    HttpError(String),
    /// API returned an error.
    ApiError {
        /// Error code from the API.
        code: String,
        /// Human-readable error message.
        message: String,
    },
    /// Multi-modal input rejected (model doesn't support it).
    MultimodalNotSupported,
    /// Response deserialization failed.
    DeserializationError(String),
    /// Dimension mismatch: model returned unexpected vector length.
    DimensionMismatch {
        /// Expected vector length (from model card).
        expected: u32,
        /// Actual vector length received.
        got: usize,
    },
}

impl fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKeyMissing(msg) => write!(f, "API key missing: {msg}"),
            Self::HttpError(msg) => write!(f, "HTTP error: {msg}"),
            Self::ApiError { code, message } => {
                write!(f, "API error [{code}]: {message}")
            }
            Self::MultimodalNotSupported => {
                write!(f, "multi-modal input not supported by this model")
            }
            Self::DeserializationError(msg) => {
                write!(f, "deserialization error: {msg}")
            }
            Self::DimensionMismatch { expected, got } => {
                write!(f, "dimension mismatch: expected {expected}, got {got}")
            }
        }
    }
}

impl std::error::Error for EmbeddingError {}
