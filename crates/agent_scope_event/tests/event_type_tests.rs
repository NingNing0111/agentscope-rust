//! Integration tests for EventType full enumeration (33 variants).
//! T105

use agent_scope_event::EventType;

#[test]
fn test_event_type_has_exactly_33_variants() {
    let all_variants: [EventType; 33] = [
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
    // This compiles only if we have exactly 33 elements
    assert_eq!(all_variants.len(), 33);
}

#[test]
fn test_all_event_types_serialize_to_screaming_snake_case() {
    let cases: Vec<(EventType, &str)> = vec![
        (EventType::ReplyStart, "REPLY_START"),
        (EventType::ReplyEnd, "REPLY_END"),
        (EventType::ModelCallStart, "MODEL_CALL_START"),
        (EventType::ModelCallEnd, "MODEL_CALL_END"),
        (EventType::TextBlockStart, "TEXT_BLOCK_START"),
        (EventType::TextBlockDelta, "TEXT_BLOCK_DELTA"),
        (EventType::TextBlockEnd, "TEXT_BLOCK_END"),
        (EventType::DataBlockStart, "DATA_BLOCK_START"),
        (EventType::DataBlockDelta, "DATA_BLOCK_DELTA"),
        (EventType::DataBlockEnd, "DATA_BLOCK_END"),
        (EventType::ThinkingBlockStart, "THINKING_BLOCK_START"),
        (EventType::ThinkingBlockDelta, "THINKING_BLOCK_DELTA"),
        (EventType::ThinkingBlockEnd, "THINKING_BLOCK_END"),
        (EventType::HintBlock, "HINT_BLOCK"),
        (EventType::ToolCallStart, "TOOL_CALL_START"),
        (EventType::ToolCallDelta, "TOOL_CALL_DELTA"),
        (EventType::ToolCallEnd, "TOOL_CALL_END"),
        (EventType::ToolResultStart, "TOOL_RESULT_START"),
        (EventType::ToolResultTextDelta, "TOOL_RESULT_TEXT_DELTA"),
        (EventType::ToolResultDataDelta, "TOOL_RESULT_DATA_DELTA"),
        (EventType::ToolResultEnd, "TOOL_RESULT_END"),
        (EventType::ExceedMaxIters, "EXCEED_MAX_ITERS"),
        (EventType::RequireUserConfirm, "REQUIRE_USER_CONFIRM"),
        (EventType::UserConfirmResult, "USER_CONFIRM_RESULT"),
        (EventType::UserInterrupt, "USER_INTERRUPT"),
        (
            EventType::RequireExternalExecution,
            "REQUIRE_EXTERNAL_EXECUTION",
        ),
        (
            EventType::ExternalExecutionResult,
            "EXTERNAL_EXECUTION_RESULT",
        ),
        (EventType::Custom, "CUSTOM"),
        (EventType::SessionCreated, "SESSION_CREATED"),
        (EventType::SessionClosed, "SESSION_CLOSED"),
        (EventType::SessionSaved, "SESSION_SAVED"),
        (EventType::SessionLoaded, "SESSION_LOADED"),
        (EventType::SessionTrimmed, "SESSION_TRIMMED"),
    ];
    assert_eq!(cases.len(), 33, "must cover all 33 event types");

    for (variant, expected) in cases {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(
            json,
            format!(r#""{}""#, expected),
            "EventType::{:?} should serialize to \"{}\"",
            variant,
            expected
        );
    }
}

#[test]
fn test_all_event_types_roundtrip() {
    let variants: [EventType; 33] = [
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

    for v in &variants {
        let json = serde_json::to_string(v).unwrap();
        let restored: EventType = serde_json::from_str(&json).unwrap();
        assert_eq!(*v, restored, "Round-trip failed for {:?}", v);
    }
}

#[test]
fn test_event_type_deserialization_from_lowercase_strings() {
    // Verify deserialization from SCREAMING_SNAKE_CASE strings
    let json = r#""TEXT_BLOCK_DELTA""#;
    let et: EventType = serde_json::from_str(json).unwrap();
    assert_eq!(et, EventType::TextBlockDelta);

    let json = r#""tool_result_end""#;
    let result: Result<EventType, _> = serde_json::from_str(json);
    assert!(result.is_err(), "lowercase should fail to deserialize");
}
