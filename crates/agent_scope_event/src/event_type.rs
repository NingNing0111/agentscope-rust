//! EventType enumeration — all 28 event type variants.

use serde::{Deserialize, Serialize};

/// Stream-based event type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    ReplyStart,
    ReplyEnd,
    ModelCallStart,
    ModelCallEnd,
    TextBlockStart,
    TextBlockDelta,
    TextBlockEnd,
    DataBlockStart,
    DataBlockDelta,
    DataBlockEnd,
    ThinkingBlockStart,
    ThinkingBlockDelta,
    ThinkingBlockEnd,
    HintBlock,
    ToolCallStart,
    ToolCallDelta,
    ToolCallEnd,
    ToolResultStart,
    ToolResultTextDelta,
    ToolResultDataDelta,
    ToolResultEnd,
    ExceedMaxIters,
    RequireUserConfirm,
    UserConfirmResult,
    UserInterrupt,
    RequireExternalExecution,
    ExternalExecutionResult,
    Custom,
    SessionCreated,
    SessionClosed,
    SessionSaved,
    SessionLoaded,
    SessionTrimmed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_serialization() {
        let pairs = vec![
            (EventType::ReplyStart, r#""REPLY_START""#),
            (EventType::ReplyEnd, r#""REPLY_END""#),
            (EventType::ModelCallStart, r#""MODEL_CALL_START""#),
            (EventType::ModelCallEnd, r#""MODEL_CALL_END""#),
            (EventType::TextBlockStart, r#""TEXT_BLOCK_START""#),
            (EventType::TextBlockDelta, r#""TEXT_BLOCK_DELTA""#),
            (EventType::TextBlockEnd, r#""TEXT_BLOCK_END""#),
            (EventType::ToolCallStart, r#""TOOL_CALL_START""#),
            (EventType::Custom, r#""CUSTOM""#),
        ];
        for (variant, expected) in pairs {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected, "mismatch for {:?}", variant);
        }
    }

    #[test]
    fn test_event_type_all_variants_roundtrip() {
        let variants = vec![
            EventType::ReplyStart,
            EventType::ReplyEnd,
            EventType::ModelCallStart,
            EventType::ModelCallEnd,
            EventType::TextBlockStart,
            EventType::TextBlockDelta,
            EventType::TextBlockEnd,
            EventType::DataBlockStart,
            EventType::DataBlockDelta,
            EventType::DataBlockEnd,
            EventType::ThinkingBlockStart,
            EventType::ThinkingBlockDelta,
            EventType::ThinkingBlockEnd,
            EventType::HintBlock,
            EventType::ToolCallStart,
            EventType::ToolCallDelta,
            EventType::ToolCallEnd,
            EventType::ToolResultStart,
            EventType::ToolResultTextDelta,
            EventType::ToolResultDataDelta,
            EventType::ToolResultEnd,
            EventType::ExceedMaxIters,
            EventType::RequireUserConfirm,
            EventType::UserConfirmResult,
            EventType::UserInterrupt,
            EventType::RequireExternalExecution,
            EventType::ExternalExecutionResult,
            EventType::Custom,
            EventType::SessionCreated,
            EventType::SessionClosed,
            EventType::SessionSaved,
            EventType::SessionLoaded,
            EventType::SessionTrimmed,
        ];
        assert_eq!(variants.len(), 33, "should have 33 event types");
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let restored: EventType = serde_json::from_str(&json).unwrap();
            assert_eq!(v, restored);
        }
    }
}
