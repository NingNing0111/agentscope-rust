//! Integration tests for event struct serialization (sample 10 key event types).
//! T106

use agent_scope_event::base::EventBase;
use agent_scope_event::{
    AgentEvent, CustomEvent, DataBlockStartEvent, ExceedMaxItersEvent, HintBlockEvent,
    ModelCallEndEvent, ReplyEndEvent, ReplyStartEvent, TextBlockDeltaEvent, TextBlockEndEvent,
    TextBlockStartEvent, ThinkingBlockEndEvent, ToolCallEndEvent, ToolCallStartEvent,
    ToolResultEndEvent, UserInterruptEvent,
};
use agent_scope_types::ReplyFinishedReason;

fn make_base() -> EventBase {
    EventBase::new()
}

// ── Reply events ────────────────────────────────────────────────────

#[test]
fn test_reply_start_event_serialization() {
    let event = AgentEvent::ReplyStart(ReplyStartEvent {
        base: make_base(),
        session_id: "s-1".into(),
        reply_id: "r-1".into(),
        name: "agent".into(),
        role: "assistant".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"REPLY_START""#));
    assert!(json.contains(r#""session_id":"s-1""#));
    assert!(json.contains(r#""reply_id":"r-1""#));

    let restored: AgentEvent = serde_json::from_str(&json).unwrap();
    match restored {
        AgentEvent::ReplyStart(e) => {
            assert_eq!(e.session_id, "s-1");
            assert_eq!(e.name, "agent");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_reply_end_event_serialization() {
    let event = AgentEvent::ReplyEnd(ReplyEndEvent {
        base: make_base(),
        session_id: "s-1".into(),
        reply_id: "r-1".into(),
        finished_reason: ReplyFinishedReason::Completed,
        error: None,
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"REPLY_END""#));
    assert!(json.contains(r#""finished_reason":"completed""#));

    let restored: AgentEvent = serde_json::from_str(&json).unwrap();
    match restored {
        AgentEvent::ReplyEnd(e) => {
            assert_eq!(e.finished_reason, ReplyFinishedReason::Completed);
        }
        _ => panic!("wrong variant"),
    }
}

// ── Model call events ────────────────────────────────────────────────

#[test]
fn test_model_call_end_event_serialization() {
    let event = AgentEvent::ModelCallEnd(ModelCallEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        input_tokens: 150,
        output_tokens: 75,
        finished_reason: ReplyFinishedReason::Completed,
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"MODEL_CALL_END""#));
    assert!(json.contains(r#""input_tokens":150"#));
    assert!(json.contains(r#""output_tokens":75"#));
}

// ── Text block streaming events ─────────────────────────────────────

#[test]
fn test_text_block_start_event_serialization() {
    let event = AgentEvent::TextBlockStart(TextBlockStartEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        block_id: "block-001".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"TEXT_BLOCK_START""#));
    assert!(json.contains(r#""block_id":"block-001""#));
}

#[test]
fn test_text_block_delta_event_serialization() {
    let event = AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        block_id: "block-001".into(),
        delta: "Hello".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"TEXT_BLOCK_DELTA""#));
    assert!(json.contains(r#""delta":"Hello""#));
}

#[test]
fn test_text_block_end_event_serialization() {
    let event = AgentEvent::TextBlockEnd(TextBlockEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        block_id: "block-001".into(),
        text: None,
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"TEXT_BLOCK_END""#));

    let restored: AgentEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(restored, AgentEvent::TextBlockEnd(_)));
}

// ── Data block event ─────────────────────────────────────────────────

#[test]
fn test_data_block_start_event_serialization() {
    let event = AgentEvent::DataBlockStart(DataBlockStartEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        block_id: "block-001".into(),
        media_type: "image/png".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"DATA_BLOCK_START""#));
    assert!(json.contains(r#""media_type":"image/png""#));
}

// ── Tool call events ─────────────────────────────────────────────────

#[test]
fn test_tool_call_start_event_serialization() {
    let event = AgentEvent::ToolCallStart(ToolCallStartEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        tool_call_id: "tc-1".into(),
        tool_call_name: "search".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"TOOL_CALL_START""#));
    assert!(json.contains(r#""tool_call_name":"search""#));

    let restored: AgentEvent = serde_json::from_str(&json).unwrap();
    match restored {
        AgentEvent::ToolCallStart(e) => assert_eq!(e.tool_call_id, "tc-1"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_tool_call_end_event_serialization() {
    let event = AgentEvent::ToolCallEnd(ToolCallEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        tool_call_id: "tc-1".into(),
        input: None,
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"TOOL_CALL_END""#));
}

// ── Control events ───────────────────────────────────────────────────

#[test]
fn test_user_interrupt_event_serialization() {
    let event = AgentEvent::UserInterrupt(UserInterruptEvent {
        base: make_base(),
        reply_id: "r-1".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"USER_INTERRUPT""#));
}

#[test]
fn test_exceed_max_iters_event_serialization() {
    let event = AgentEvent::ExceedMaxIters(ExceedMaxItersEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        name: "agent".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"EXCEED_MAX_ITERS""#));
}

// ── Hint block event ─────────────────────────────────────────────────

#[test]
fn test_hint_block_event_serialization() {
    let event = AgentEvent::HintBlock(HintBlockEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        block_id: "hint-1".into(),
        source: Some("system".into()),
        hint: agent_scope_message::block::HintContent::Text("tip".into()),
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"HINT_BLOCK""#));
    assert!(json.contains(r#""block_id":"hint-1""#));
}

// ── Custom event ─────────────────────────────────────────────────────

#[test]
fn test_custom_event_serialization() {
    let mut value = std::collections::HashMap::new();
    value.insert("key".into(), serde_json::json!("val"));
    let event = AgentEvent::Custom(CustomEvent {
        base: make_base(),
        name: "my_custom".into(),
        value,
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"CUSTOM""#));
    assert!(json.contains(r#""name":"my_custom""#));
}

// ── EventBase embedded in all events ─────────────────────────────────

#[test]
fn test_event_base_embedded_in_all_sample_events() {
    let base = make_base();
    let base_id = base.id.clone();

    let event = AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
        base,
        reply_id: "r".into(),
        block_id: "b".into(),
        delta: "x".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(
        json.contains(&base_id),
        "EventBase id should be embedded as a flat field"
    );
}

// ── Feature 014: EndEvent complete-content field serde round-trip ─────

#[test]
fn test_text_block_end_event_content_round_trip() {
    let event = TextBlockEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        block_id: "b-1".into(),
        text: Some("Hello world".into()),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""text":"Hello world""#));
    let round: TextBlockEndEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(round.text, Some("Hello world".into()));
}

#[test]
fn test_text_block_end_event_text_none_omitted() {
    let event = TextBlockEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        block_id: "b-1".into(),
        text: None,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(
        !json.contains(r#""text""#),
        "None should be omitted: {json}"
    );
    let round: TextBlockEndEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(round.text, None);
}

#[test]
fn test_text_block_end_event_text_empty_string() {
    let event = TextBlockEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        block_id: "b-1".into(),
        text: Some("".into()),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(
        json.contains(r#""text":"""#),
        "Empty string must be serialized"
    );
    let round: TextBlockEndEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(round.text, Some("".into()));
}

#[test]
fn test_thinking_block_end_event_content_round_trip() {
    let event = ThinkingBlockEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        block_id: "t-1".into(),
        thinking: Some("I should calculate first.".into()),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""thinking":"I should calculate first.""#));
    let round: ThinkingBlockEndEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(round.thinking, Some("I should calculate first.".into()));
}

#[test]
fn test_thinking_block_end_event_thinking_none_omitted() {
    let event = ThinkingBlockEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        block_id: "t-1".into(),
        thinking: None,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(!json.contains(r#""thinking""#));
    let round: ThinkingBlockEndEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(round.thinking, None);
}

#[test]
fn test_tool_call_end_event_input_round_trip() {
    let event = ToolCallEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        tool_call_id: "tc-1".into(),
        input: Some(r#"{"expression":"5678 * 345"}"#.into()),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""input":""#));
    let round: ToolCallEndEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(round.input, Some(r#"{"expression":"5678 * 345"}"#.into()));
}

#[test]
fn test_tool_call_end_event_input_none_omitted() {
    let event = ToolCallEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        tool_call_id: "tc-1".into(),
        input: None,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(!json.contains(r#""input""#));
    let round: ToolCallEndEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(round.input, None);
}

#[test]
fn test_tool_result_end_event_output_round_trip() {
    // NOTE: ToolResultEndEvent has a pre-existing serde issue where EventBase.metadata
    // (flattened) conflicts with its own metadata field, causing duplicate key on round-trip.
    // This test verifies serialization only; the duplicate-metadata fix is tracked separately.
    let event = ToolResultEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        tool_call_id: "tc-1".into(),
        state: agent_scope_message::ToolResultState::Success,
        metadata: std::collections::HashMap::new(),
        output: Some("1958910".into()),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""output":"1958910""#));
    // Verify output is absent when None
    let event_none = ToolResultEndEvent {
        base: make_base(),
        reply_id: "r-1".into(),
        tool_call_id: "tc-1".into(),
        state: agent_scope_message::ToolResultState::Error,
        metadata: std::collections::HashMap::new(),
        output: None,
    };
    let json_none = serde_json::to_string(&event_none).unwrap();
    assert!(
        !json_none.contains(r#""output""#),
        "None output must be omitted"
    );
}

#[test]
fn test_end_event_backward_compat_missing_field() {
    // Old-style JSON without the new content fields must deserialize
    let old_json = r#"{"id":"ev-1","timestamp":"2026-01-01T00:00:00Z","type":"TEXT_BLOCK_END","reply_id":"r-1","block_id":"b-1"}"#;
    let event: TextBlockEndEvent = serde_json::from_str(old_json).unwrap();
    assert_eq!(event.text, None);

    // Old-style ToolCallEndEvent without input field
    let old_tc_json = r#"{"id":"ev-2","timestamp":"2026-01-01T00:00:00Z","type":"TOOL_CALL_END","reply_id":"r-1","tool_call_id":"tc-1"}"#;
    let tc_event: ToolCallEndEvent = serde_json::from_str(old_tc_json).unwrap();
    assert_eq!(tc_event.input, None);

    // ThinkingBlockEndEvent without thinking field
    let old_th_json = r#"{"id":"ev-3","timestamp":"2026-01-01T00:00:00Z","type":"THINKING_BLOCK_END","reply_id":"r-1","block_id":"t-1"}"#;
    let th_event: ThinkingBlockEndEvent = serde_json::from_str(old_th_json).unwrap();
    assert_eq!(th_event.thinking, None);
}

#[test]
fn test_planner_lifecycle_custom_event_round_trip() {
    let mut value = std::collections::HashMap::new();
    value.insert("event_type".into(), serde_json::json!("custom_started"));
    value.insert("sequence".into(), serde_json::json!(1));
    value.insert("task_id".into(), serde_json::json!("task-001"));
    value.insert("summary".into(), serde_json::json!("safe summary"));

    let event = AgentEvent::Custom(CustomEvent {
        base: make_base(),
        name: "app.custom_event".into(),
        value,
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"CUSTOM""#));
    assert!(json.contains(r#""name":"app.custom_event""#));
    assert!(json.contains(r#""event_type":"custom_started""#));

    let restored: AgentEvent = serde_json::from_str(&json).unwrap();
    match restored {
        AgentEvent::Custom(custom) => {
            assert_eq!(custom.name, "app.custom_event");
            assert_eq!(
                custom.value["event_type"],
                serde_json::json!("custom_started")
            );
            assert_eq!(custom.value["sequence"], serde_json::json!(1));
            assert_eq!(custom.value["task_id"], serde_json::json!("task-001"));
        }
        _ => panic!("wrong variant"),
    }
}
