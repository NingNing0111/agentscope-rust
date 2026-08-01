//! Typed sandbox errors.

use std::fmt;
use std::time::Duration;

use crate::session::SandboxState;

pub type SandboxResult<T> = Result<T, SandboxError>;

#[derive(Debug, Clone)]
pub enum SandboxError {
    ValidationError {
        message: String,
    },
    LifecycleError {
        state: SandboxState,
        operation: String,
    },
    PermissionDenied {
        path: Option<String>,
        operation: String,
    },
    TimeoutError {
        execution_id: String,
        timeout: Duration,
    },
    UnsupportedFeature {
        feature: String,
        reason: String,
    },
    SandboxUnavailable {
        backend: String,
        reason: String,
    },
    IoError {
        operation: String,
        message: String,
    },
    InternalError {
        message: String,
    },
}

impl SandboxError {
    #[must_use]
    pub fn category(&self) -> &'static str {
        match self {
            SandboxError::ValidationError { .. } => "validation_error",
            SandboxError::LifecycleError { .. } => "lifecycle_error",
            SandboxError::PermissionDenied { .. } => "permission_denied",
            SandboxError::TimeoutError { .. } => "timeout",
            SandboxError::UnsupportedFeature { .. } => "unsupported_feature",
            SandboxError::SandboxUnavailable { .. } => "sandbox_unavailable",
            SandboxError::IoError { .. } => "io_error",
            SandboxError::InternalError { .. } => "internal_error",
        }
    }
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SandboxError::ValidationError { message } => write!(f, "validation error: {message}"),
            SandboxError::LifecycleError { state, operation } => {
                write!(
                    f,
                    "sandbox lifecycle error during {operation}: state is {state:?}"
                )
            }
            SandboxError::PermissionDenied { path, operation } => {
                write!(f, "permission denied during {operation}")?;
                if let Some(path) = path {
                    write!(f, " for {path}")?;
                }
                Ok(())
            }
            SandboxError::TimeoutError {
                execution_id,
                timeout,
            } => {
                write!(f, "execution {execution_id} timed out after {timeout:?}")
            }
            SandboxError::UnsupportedFeature { feature, reason } => {
                write!(f, "unsupported sandbox feature {feature}: {reason}")
            }
            SandboxError::SandboxUnavailable { backend, reason } => {
                write!(f, "sandbox backend {backend} unavailable: {reason}")
            }
            SandboxError::IoError { operation, message } => {
                write!(f, "I/O error during {operation}: {message}")
            }
            SandboxError::InternalError { message } => {
                write!(f, "internal sandbox error: {message}")
            }
        }
    }
}

impl std::error::Error for SandboxError {}

impl From<std::io::Error> for SandboxError {
    fn from(value: std::io::Error) -> Self {
        SandboxError::IoError {
            operation: "io".into(),
            message: value.to_string(),
        }
    }
}
