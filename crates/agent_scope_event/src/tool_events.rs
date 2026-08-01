//! Tool call and tool result lifecycle events.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use agent_scope_message::ToolResultState;

use crate::base::EventBase;

// ---------------------------------------------------------------------------
// Tool call events
// ---------------------------------------------------------------------------

/// A tool call has started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallStartEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub tool_call_id: String,
    pub tool_call_name: String,
}

/// Incremental delta for a tool call's input JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDeltaEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub tool_call_id: String,
    pub delta: String,
}

/// A tool call input has been fully received.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEndEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub tool_call_id: String,
    /// Complete tool input accumulated from all ToolCallDelta events.
    /// `Some("")` means known empty; `None` means unknown/unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
}

// ---------------------------------------------------------------------------
// Tool result events
// ---------------------------------------------------------------------------

/// A tool result has started streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultStartEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub tool_call_id: String,
    pub tool_call_name: String,
}

/// Text delta for a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultTextDeltaEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub tool_call_id: String,
    pub delta: String,
}

/// Data/binary delta for a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultDataDeltaEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub tool_call_id: String,
    pub block_id: String,
    pub media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// A tool result has completed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultEndEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub tool_call_id: String,
    pub state: ToolResultState,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Complete observable output accumulated from all ToolResultTextDelta events.
    /// `Some("")` means known empty; `None` means unknown/unavailable.
    /// Must be `None` for error/interrupted states unless the output is known-complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_call_start_event_serialization() {
        let event = ToolCallStartEvent {
            base: EventBase::new(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-001".into(),
            tool_call_name: "search".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""tool_call_id":"tc-001""#));
        assert!(json.contains(r#""tool_call_name":"search""#));
    }

    #[test]
    fn test_tool_result_end_event_serialization() {
        let event = ToolResultEndEvent {
            base: EventBase::new(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-001".into(),
            state: ToolResultState::Success,
            metadata: HashMap::new(),
            output: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""state":"success""#));
    }
}
