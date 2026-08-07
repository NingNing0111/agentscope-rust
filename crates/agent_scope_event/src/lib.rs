//! AgentScope Foundation Layer — Event system.
//!
//! 33 event types covering reply lifecycle, model calls, content block streaming,
//! tool execution, user interaction, and external execution.

#![deny(unsafe_code)]

pub mod append_event;
pub mod base;
pub mod block_events;
pub mod control_events;
pub mod custom;
pub mod event_type;
pub mod model_events;
pub mod reply_events;
pub mod session_events;
pub mod tool_events;

// Re-export EventBase and EventType
pub use base::EventBase;
pub use event_type::EventType;

// Re-export all event structs
pub use block_events::{
    DataBlockDeltaEvent, DataBlockEndEvent, DataBlockStartEvent, HintBlockEvent,
    TextBlockDeltaEvent, TextBlockEndEvent, TextBlockStartEvent, ThinkingBlockDeltaEvent,
    ThinkingBlockEndEvent, ThinkingBlockStartEvent,
};
pub use control_events::{
    ConfirmResult, ExceedMaxItersEvent, ExternalExecutionResultEvent,
    RequireExternalExecutionEvent, RequireUserConfirmEvent, UserConfirmResultEvent,
    UserInterruptEvent,
};
pub use custom::CustomEvent;
pub use model_events::{ModelCallEndEvent, ModelCallStartEvent};
pub use reply_events::{ReplyEndEvent, ReplyStartEvent};
pub use session_events::{
    SessionClosedEvent, SessionCreatedEvent, SessionLoadedEvent, SessionSavedEvent,
    SessionTrimmedEvent,
};
pub use tool_events::{
    ToolCallDeltaEvent, ToolCallEndEvent, ToolCallStartEvent, ToolResultDataDeltaEvent,
    ToolResultEndEvent, ToolResultStartEvent, ToolResultTextDeltaEvent,
};

pub use append_event::{AppendEvent, AppendEventError};

use serde::{Deserialize, Serialize};

/// Tagged union of all 33 event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    #[serde(rename = "REPLY_START")]
    ReplyStart(ReplyStartEvent),
    #[serde(rename = "REPLY_END")]
    ReplyEnd(ReplyEndEvent),
    #[serde(rename = "MODEL_CALL_START")]
    ModelCallStart(ModelCallStartEvent),
    #[serde(rename = "MODEL_CALL_END")]
    ModelCallEnd(ModelCallEndEvent),
    #[serde(rename = "TEXT_BLOCK_START")]
    TextBlockStart(TextBlockStartEvent),
    #[serde(rename = "TEXT_BLOCK_DELTA")]
    TextBlockDelta(TextBlockDeltaEvent),
    #[serde(rename = "TEXT_BLOCK_END")]
    TextBlockEnd(TextBlockEndEvent),
    #[serde(rename = "DATA_BLOCK_START")]
    DataBlockStart(DataBlockStartEvent),
    #[serde(rename = "DATA_BLOCK_DELTA")]
    DataBlockDelta(DataBlockDeltaEvent),
    #[serde(rename = "DATA_BLOCK_END")]
    DataBlockEnd(DataBlockEndEvent),
    #[serde(rename = "THINKING_BLOCK_START")]
    ThinkingBlockStart(ThinkingBlockStartEvent),
    #[serde(rename = "THINKING_BLOCK_DELTA")]
    ThinkingBlockDelta(ThinkingBlockDeltaEvent),
    #[serde(rename = "THINKING_BLOCK_END")]
    ThinkingBlockEnd(ThinkingBlockEndEvent),
    #[serde(rename = "HINT_BLOCK")]
    HintBlock(HintBlockEvent),
    #[serde(rename = "TOOL_CALL_START")]
    ToolCallStart(ToolCallStartEvent),
    #[serde(rename = "TOOL_CALL_DELTA")]
    ToolCallDelta(ToolCallDeltaEvent),
    #[serde(rename = "TOOL_CALL_END")]
    ToolCallEnd(ToolCallEndEvent),
    #[serde(rename = "TOOL_RESULT_START")]
    ToolResultStart(ToolResultStartEvent),
    #[serde(rename = "TOOL_RESULT_TEXT_DELTA")]
    ToolResultTextDelta(ToolResultTextDeltaEvent),
    #[serde(rename = "TOOL_RESULT_DATA_DELTA")]
    ToolResultDataDelta(ToolResultDataDeltaEvent),
    #[serde(rename = "TOOL_RESULT_END")]
    ToolResultEnd(ToolResultEndEvent),
    #[serde(rename = "EXCEED_MAX_ITERS")]
    ExceedMaxIters(ExceedMaxItersEvent),
    #[serde(rename = "REQUIRE_USER_CONFIRM")]
    RequireUserConfirm(RequireUserConfirmEvent),
    #[serde(rename = "USER_CONFIRM_RESULT")]
    UserConfirmResult(UserConfirmResultEvent),
    #[serde(rename = "USER_INTERRUPT")]
    UserInterrupt(UserInterruptEvent),
    #[serde(rename = "REQUIRE_EXTERNAL_EXECUTION")]
    RequireExternalExecution(RequireExternalExecutionEvent),
    #[serde(rename = "EXTERNAL_EXECUTION_RESULT")]
    ExternalExecutionResult(ExternalExecutionResultEvent),
    #[serde(rename = "CUSTOM")]
    Custom(CustomEvent),
    #[serde(rename = "SESSION_CREATED")]
    SessionCreated(SessionCreatedEvent),
    #[serde(rename = "SESSION_CLOSED")]
    SessionClosed(SessionClosedEvent),
    #[serde(rename = "SESSION_SAVED")]
    SessionSaved(SessionSavedEvent),
    #[serde(rename = "SESSION_LOADED")]
    SessionLoaded(SessionLoadedEvent),
    #[serde(rename = "SESSION_TRIMMED")]
    SessionTrimmed(SessionTrimmedEvent),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_event_all_variants_have_type_tag() {
        use crate::base::EventBase;
        let base = EventBase::new();

        let events: Vec<AgentEvent> = vec![
            AgentEvent::ReplyStart(ReplyStartEvent {
                base: base.clone(),
                session_id: "s".into(),
                reply_id: "r".into(),
                name: "n".into(),
                role: "assistant".into(),
            }),
            AgentEvent::ReplyEnd(ReplyEndEvent {
                base: base.clone(),
                session_id: "s".into(),
                reply_id: "r".into(),
                finished_reason: agent_scope_types::ReplyFinishedReason::Completed,
                error: None,
            }),
            AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
                base: base.clone(),
                reply_id: "r".into(),
                block_id: "b".into(),
                delta: "d".into(),
            }),
            AgentEvent::UserInterrupt(UserInterruptEvent {
                base,
                reply_id: "r".into(),
            }),
        ];

        let expected_tags = [
            "REPLY_START",
            "REPLY_END",
            "TEXT_BLOCK_DELTA",
            "USER_INTERRUPT",
        ];

        for (event, expected) in events.iter().zip(expected_tags.iter()) {
            let json = serde_json::to_string(event).unwrap();
            assert!(
                json.contains(&format!(r#""type":"{}""#, expected)),
                "expected tag '{}' in: {}",
                expected,
                json
            );
        }
    }

    #[test]
    fn test_agent_event_roundtrip() {
        let base = EventBase::new();
        let event = AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
            base,
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            delta: "Hello".into(),
        });

        let json = serde_json::to_string(&event).unwrap();
        let restored: AgentEvent = serde_json::from_str(&json).unwrap();

        if let AgentEvent::TextBlockDelta(e) = restored {
            assert_eq!(e.delta, "Hello");
            assert_eq!(e.block_id, "block-001");
        } else {
            panic!("wrong variant after roundtrip");
        }
    }
}
