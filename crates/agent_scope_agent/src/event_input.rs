//! Event input types for event-driven HITL (Feature 032).
//!
//! Aligned with the Python `_reply_impl(inputs=...)` semantics:
//! `reply_stream_event` accepts one of the HITL events the host injects
//! to resume a paused reply. The plain-message path (`reply_stream`) is
//! unchanged.

use agent_scope_event::{ExternalExecutionResultEvent, UserConfirmResultEvent, UserInterruptEvent};

/// Host-injected event input for resuming a paused reply.
///
/// Mirrors Python's `inputs: UserConfirmResultEvent | UserInterruptEvent |
/// ExternalExecutionResultEvent | None` dispatch in `_agent.py:_reply_impl`.
#[derive(Debug, Clone)]
pub enum EventInput {
    /// User confirmation results for previously-asked tool calls.
    Confirm(UserConfirmResultEvent),
    /// User interrupt; ends an in-progress / awaiting reply.
    Interrupt(UserInterruptEvent),
    /// External execution results for previously-submitted tool calls.
    ExternalResult(ExternalExecutionResultEvent),
}
