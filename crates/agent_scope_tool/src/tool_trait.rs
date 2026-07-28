//! Core Tool trait and associated types for the AgentScope Tool System.
//!
//! Defines the [`Tool`] trait (the central extension point), [`ToolExecOutput`]
//! (complete or streaming execution result), [`ToolError`] (typed error
//! taxonomy), and the [`ToolChunk`] type alias.

use std::pin::Pin;

use agent_scope_message::ToolResultBlock;
use futures::Stream;
use serde_json::Value as JsonValue;

// ---------------------------------------------------------------------------
// ToolExecOutput (T004)
// ---------------------------------------------------------------------------

/// Result of a tool execution — either a one-shot complete result or a
/// streaming sequence of chunks.
///
/// The design mirrors [`agent_scope_model::ModelCallResult`] for API
/// consistency.
pub enum ToolExecOutput {
    /// One-shot execution result.  The wrapped [`ToolResultBlock`] has
    /// `is_last: true` and `state: Success`.
    Complete(ToolResultBlock),
    /// Streaming execution — the caller is responsible for consuming the
    /// stream.  Each [`ToolResultBlock`] has `is_last` set by the tool
    /// implementation; the framework does not auto-accumulate.
    Stream(Pin<Box<dyn Stream<Item = Result<ToolResultBlock, ToolError>> + Send>>),
}

impl std::fmt::Debug for ToolExecOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Complete(block) => f.debug_tuple("Complete").field(block).finish(),
            Self::Stream(_) => f.debug_tuple("Stream").field(&"<stream>").finish(),
        }
    }
}

// ---------------------------------------------------------------------------
// ToolError (T005)
// ---------------------------------------------------------------------------

/// Typed errors for all tool operations.
///
/// Aligns with the Constitution's Error Model (Art. 13).
#[derive(Debug, Clone, thiserror::Error)]
pub enum ToolError {
    /// Tool not found in the [`ToolKit`](crate::ToolKit).
    #[error("tool '{tool_name}' not found")]
    NotFound {
        /// Name of the tool that was requested but not registered.
        tool_name: String,
    },

    /// Input JSON could not be deserialized to the tool's expected parameter
    /// type.
    #[error("invalid input for tool '{tool_name}': {reason}")]
    InvalidInput {
        /// Name of the tool that received the invalid input.
        tool_name: String,
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// Tool execution failed — either handler panicked or a runtime error
    /// occurred.
    #[error("tool '{tool_name}' execution failed: {reason}")]
    Execution {
        /// Name of the tool whose handler failed.
        tool_name: String,
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// Tool execution was interrupted (e.g. by a cancellation token).
    #[error("tool '{tool_name}' was interrupted")]
    Interrupted {
        /// Name of the tool that was interrupted.
        tool_name: String,
    },
}

// ---------------------------------------------------------------------------
// ToolChunk (T006)
// ---------------------------------------------------------------------------

/// A streaming chunk from a tool execution.
///
/// This is simply an alias for [`agent_scope_message::ToolResultBlock`].
/// In streaming scenarios multiple chunks can be accumulated into a complete
/// result.  The `is_last` field (added in Feature 006) marks the final chunk.
pub type ToolChunk = ToolResultBlock;

// ---------------------------------------------------------------------------
// Tool trait (T007)
// ---------------------------------------------------------------------------

/// Core abstraction for executable tools.
///
/// A Tool has metadata (name, description, input-schema) and an execution
/// method ([`Tool::call`]).  It aligns with AgentScope Python's `ToolBase`.
///
/// # Contract guarantees
///
/// | Guarantee | Description |
/// |-----------|-------------|
/// | Thread safety | `Send + Sync` — shareable via `Arc<dyn Tool>` |
/// | No unsafe | Zero `unsafe` code in any implementation |
/// | Idempotent metadata | `name()`, `description()`, `input_schema()` return stable values |
/// | Panic boundary | `call()` MUST NOT propagate panics to the caller |
/// | Error typed | All failures are through [`ToolError`] |
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Returns the unique name of this tool.
    ///
    /// Used as the key in [`ToolKit`](crate::ToolKit) and as `function.name`
    /// in the OpenAI schema.
    fn name(&self) -> &str;

    /// Returns a human-readable description.
    ///
    /// Included in the tool schema sent to the model to help it decide when to
    /// call.
    fn description(&self) -> &str;

    /// Returns the JSON Schema for this tool's input parameters.
    ///
    /// Format: `{"type": "object", "properties": {...}, "required": [...]}`.
    fn input_schema(&self) -> JsonValue;

    /// Whether this tool can be safely called from multiple async tasks
    /// concurrently.
    ///
    /// Default: `true`.
    fn is_concurrency_safe(&self) -> bool {
        true
    }

    /// Whether this tool has no observable side effects.
    ///
    /// Default: `false`.
    fn is_read_only(&self) -> bool {
        false
    }

    /// Execute the tool with the given JSON input.
    ///
    /// # Arguments
    /// * `input` — A [`serde_json::Value`] representing the tool's parameters.
    ///   Must be a JSON object matching [`Tool::input_schema`].
    ///
    /// # Returns
    /// * `Ok(ToolExecOutput::Complete(chunk))` — one-shot result
    /// * `Ok(ToolExecOutput::Stream(stream))` — streaming result
    /// * `Err(ToolError)` — various failure modes
    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError>;
}
