//! ModelError — unified error type for all model-layer operations.

use std::fmt;

use crate::formatter::FormatError;

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
#[derive(Debug)]
pub enum ModelError {
    ApiError {
        status: u16,
        message: String,
        provider: String,
    },
    RetryExhausted {
        attempts: u32,
        last_error: Box<ModelError>,
        provider: String,
    },
    Cancelled,
    ValidationError {
        field: String,
        message: String,
    },
    SerializationError {
        context: String,
        source: serde_json::Error,
    },
    FormatError {
        context: String,
        source: FormatError,
    },
    StructuredOutputError {
        reason: String,
    },
    UnsupportedFeature {
        feature: String,
        provider: String,
    },
    ConfigError {
        message: String,
    },
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiError {
                status,
                message,
                provider,
            } => {
                write!(f, "[{provider}] API error {status}: {message}")
            }
            Self::RetryExhausted {
                attempts,
                last_error,
                provider,
            } => {
                write!(
                    f,
                    "[{provider}] Retry exhausted after {attempts} attempts: {last_error}"
                )
            }
            Self::Cancelled => write!(f, "Operation cancelled"),
            Self::ValidationError { field, message } => {
                write!(f, "Validation error on '{field}': {message}")
            }
            Self::SerializationError { context, source } => {
                write!(f, "Serialization error in {context}: {source}")
            }
            Self::FormatError { context, source } => {
                write!(f, "Format error in {context}: {source}")
            }
            Self::StructuredOutputError { reason } => {
                write!(f, "Structured output error: {reason}")
            }
            Self::UnsupportedFeature { feature, provider } => {
                write!(f, "[{provider}] Unsupported feature: {feature}")
            }
            Self::ConfigError { message } => {
                write!(f, "Config error: {message}")
            }
        }
    }
}

impl std::error::Error for ModelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SerializationError { source, .. } => Some(source),
            Self::FormatError { source, .. } => Some(source),
            _ => None,
        }
    }
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
