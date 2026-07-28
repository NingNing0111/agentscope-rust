//! Control and interaction events.

use serde::{Deserialize, Serialize};

use agent_scope_message::{PermissionRule, ToolCallBlock, ToolResultBlock};

use crate::base::EventBase;

// ---------------------------------------------------------------------------
// ConfirmResult
// ---------------------------------------------------------------------------

/// Result of a single tool confirmation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmResult {
    pub confirmed: bool,
    pub tool_call: ToolCallBlock,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<PermissionRule>>,
}

// ---------------------------------------------------------------------------
// Control events
// ---------------------------------------------------------------------------

/// The agent has exceeded its max iterations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExceedMaxItersEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub name: String,
}

/// User confirmation is required for tool call(s).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequireUserConfirmEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub tool_calls: Vec<ToolCallBlock>,
}

/// User has responded to tool confirmation requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfirmResultEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub confirm_results: Vec<ConfirmResult>,
}

/// The user has interrupted the current reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInterruptEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
}

/// External execution is required for tool call(s).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequireExternalExecutionEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub tool_calls: Vec<ToolCallBlock>,
}

/// External execution results have been received.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalExecutionResultEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub execution_results: Vec<ToolResultBlock>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exceed_max_iters_event_serialization() {
        let event = ExceedMaxItersEvent {
            base: EventBase::new(),
            reply_id: "reply-001".into(),
            name: "agent-1".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""name":"agent-1""#));
    }

    #[test]
    fn test_user_interrupt_event_serialization() {
        let event = UserInterruptEvent {
            base: EventBase::new(),
            reply_id: "reply-001".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""reply_id":"reply-001""#));
    }

    #[test]
    fn test_confirm_result_serialization() {
        let result = ConfirmResult {
            confirmed: true,
            tool_call: ToolCallBlock::new("tc-1".into(), "search".into(), "{}".into()),
            rules: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains(r#""confirmed":true"#));
        assert!(json.contains(r#""tool_call""#));
    }
}
