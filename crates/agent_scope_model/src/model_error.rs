//! ModelError — unified error type for all model-layer operations.

use crate::formatter::FormatError;

use thiserror::Error;

/// Error categories for retryable error matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelErrorKind {
    ApiConnection,
    ApiTimeout,
    RateLimit,
    InternalServer,
    BadRequest,
    Authentication,
}

/// Unified error type for the Model layer.
#[derive(Debug, Error)]
pub enum ModelError {
    #[error("[{provider}] API error {status}: {message}")]
    ApiError {
        status: u16,
        message: String,
        provider: String,
    },
    #[error("[{provider}] Retry exhausted after {attempts} attempts: {last_error}")]
    RetryExhausted {
        attempts: u32,
        last_error: Box<ModelError>,
        provider: String,
    },
    #[error("Operation cancelled")]
    Cancelled,
    #[error("Validation error on '{field}': {message}")]
    ValidationError { field: String, message: String },
    #[error("Serialization error in {context}: {source}")]
    SerializationError {
        context: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("Format error in {context}: {source}")]
    FormatError {
        context: String,
        #[source]
        source: FormatError,
    },
    #[error("Structured output error: {reason}")]
    StructuredOutputError { reason: String },
    #[error("[{provider}] Unsupported feature: {feature}")]
    UnsupportedFeature { feature: String, provider: String },
    #[error("Config error: {message}")]
    ConfigError { message: String },
}

impl ModelError {
    /// Classify this error into a kind for retryable matching.
    pub fn kind(&self) -> Option<ModelErrorKind> {
        match self {
            Self::ApiError { status, .. } => match status {
                401 | 403 => Some(ModelErrorKind::Authentication),
                429 => Some(ModelErrorKind::RateLimit),
                400 | 422 => Some(ModelErrorKind::BadRequest),
                500..=599 => Some(ModelErrorKind::InternalServer),
                _ => Some(ModelErrorKind::ApiConnection),
            },
            Self::Cancelled
            | Self::ConfigError { .. }
            | Self::ValidationError { .. }
            | Self::SerializationError { .. }
            | Self::FormatError { .. }
            | Self::StructuredOutputError { .. }
            | Self::UnsupportedFeature { .. }
            | Self::RetryExhausted { .. } => None,
        }
    }
}

impl From<serde_json::Error> for ModelError {
    fn from(source: serde_json::Error) -> Self {
        Self::SerializationError {
            context: "json".to_string(),
            source,
        }
    }
}

impl From<FormatError> for ModelError {
    fn from(source: FormatError) -> Self {
        Self::FormatError {
            context: "formatting".to_string(),
            source,
        }
    }
}
