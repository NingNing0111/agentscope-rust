//! AgentError — typed error enum for all agent operations.

use std::time::Duration;

use thiserror::Error;

/// Typed error for agent operations.
///
/// 10 variants covering validation, model, tool, timeout, cancellation,
/// permission, compression, empty context, max iterations, and config errors.
#[derive(Debug, Error)]
pub enum AgentError {
    /// Invalid input or configuration.
    #[error("Validation error: {message}")]
    ValidationError { message: String },

    /// Model call failure (wraps ModelError from agent_scope_model).
    #[error("Model error: {source}")]
    ModelError {
        #[source]
        source: agent_scope_model::ModelError,
    },

    /// Tool execution failure (wraps ToolError from agent_scope_tool).
    #[error("Tool error: {source}")]
    ToolError {
        #[source]
        source: agent_scope_tool::ToolError,
    },

    /// Operation timed out.
    #[error("Timeout: {operation} after {duration:?}")]
    TimeoutError {
        operation: String,
        duration: Duration,
    },

    /// Reply was cancelled/interrupted.
    #[error("Reply cancelled: {reply_id}")]
    CancellationError { reply_id: String },

    /// Tool execution rejected by permission engine.
    #[error("Permission denied for tool '{tool_name}': {reason}")]
    PermissionDenied { tool_name: String, reason: String },

    /// Context compression model call failed.
    #[error("Context compression failed: {reason}")]
    ContextCompressionFailed { reason: String },

    /// `reply(None)` called with empty state context.
    #[error("No content to reply to — state context is empty")]
    NoContentToReply,

    /// ReAct loop exceeded iteration limit.
    #[error("Max iterations ({max_iters}) exceeded")]
    MaxItersExceeded { max_iters: u32 },

    /// Config validation failed at build time.
    #[error("Invalid config field '{field}': {message}")]
    InvalidConfig { field: String, message: String },

    /// A streaming reply is already in progress.
    /// Callers must consume or drop the existing stream before starting a new reply.
    #[error("A streaming reply is already in progress")]
    AlreadyStreaming,

    /// Session persistence failure (wraps SessionError from agent_scope_state).
    ///
    /// Raised when resuming a persisted session at build time, or when an
    /// automatic save fails after a reply. Does not break the reply result
    /// already produced.
    #[error("Session persistence error: {source}")]
    SessionError {
        #[source]
        source: agent_scope_state::SessionError,
    },
}

impl From<agent_scope_model::ModelError> for AgentError {
    fn from(source: agent_scope_model::ModelError) -> Self {
        Self::ModelError { source }
    }
}

impl From<agent_scope_tool::ToolError> for AgentError {
    fn from(source: agent_scope_tool::ToolError) -> Self {
        Self::ToolError { source }
    }
}

impl From<agent_scope_state::SessionError> for AgentError {
    fn from(source: agent_scope_state::SessionError) -> Self {
        Self::SessionError { source }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    /// T016: AgentError Display should be human-readable.
    #[test]
    fn test_agent_error_display() {
        let err = AgentError::NoContentToReply;
        assert!(!err.to_string().is_empty());
        assert!(err.to_string().contains("No content to reply"));

        let err = AgentError::MaxItersExceeded { max_iters: 5 };
        assert!(err.to_string().contains("5"));

        let err = AgentError::InvalidConfig {
            field: "name".into(),
            message: "must not be empty".into(),
        };
        assert!(err.to_string().contains("name"));
        assert!(err.to_string().contains("must not be empty"));
    }

    /// T016: Source chain should work for wrapped errors.
    #[test]
    fn test_agent_error_source_chain() {
        let model_err = agent_scope_model::ModelError::ValidationError {
            field: "msg".into(),
            message: "bad".into(),
        };
        let agent_err = AgentError::from(model_err);
        assert!(agent_err.source().is_some());

        let tool_err = agent_scope_tool::ToolError::NotFound {
            tool_name: "unknown".into(),
        };
        let agent_err = AgentError::from(tool_err);
        assert!(agent_err.source().is_some());
    }

    /// T016: Non-wrapped errors have no source.
    #[test]
    fn test_agent_error_no_source() {
        let err = AgentError::ValidationError {
            message: "test".into(),
        };
        assert!(err.source().is_none());
    }

    /// T016: CancellationError format.
    #[test]
    fn test_cancellation_error_format() {
        let err = AgentError::CancellationError {
            reply_id: "reply-1".into(),
        };
        assert!(err.to_string().contains("reply-1"));
    }

    /// T016: PermissionDenied format.
    #[test]
    fn test_permission_denied_format() {
        let err = AgentError::PermissionDenied {
            tool_name: "dangerous_tool".into(),
            reason: "blocked by policy".into(),
        };
        assert!(err.to_string().contains("dangerous_tool"));
        assert!(err.to_string().contains("blocked by policy"));
    }

    /// T009: AlreadyStreaming error format.
    #[test]
    fn test_already_streaming_display() {
        let err = AgentError::AlreadyStreaming;
        let msg = err.to_string();
        assert!(msg.contains("streaming reply"));
        assert!(!msg.is_empty());
    }
}
