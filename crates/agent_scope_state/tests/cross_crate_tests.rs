//! Cross-crate serialization consistency tests.
//! T110
//!
//! Verifies that Msg serialized from message crate correctly deserializes
//! in state crate, and that Event serialized from event crate correctly
//! applies to Msg in message crate.

use agent_scope_event::AppendEvent;
use agent_scope_event::base::EventBase;
use agent_scope_event::{
    AgentEvent, ModelCallEndEvent, ReplyEndEvent, TextBlockDeltaEvent, TextBlockEndEvent,
    TextBlockStartEvent,
};
use agent_scope_message::block::{ContentBlock, TextBlock, ToolCallBlock};
use agent_scope_message::msg::{Msg, Role, Usage};
use agent_scope_state::AgentState;
use agent_scope_types::{ErrorInfo, ErrorType, ReplyFinishedReason};

#[test]
fn test_msg_serialized_in_message_crate_deserializes_in_state_crate() {
    // Create a Msg in the message crate
    let mut msg = Msg::new(
        "agent".into(),
        vec![
            ContentBlock::Text(TextBlock::new("Hello".into())),
            ContentBlock::ToolCall(ToolCallBlock::new(
                "tc-1".into(),
                "search".into(),
                r#"{"q":"test"}"#.into(),
            )),
        ],
        Role::Assistant,
    )
    .unwrap();
    msg.usage = Some(Usage {
        input_tokens: 100,
        output_tokens: 50,
    });
    msg.finished_reason = Some(ReplyFinishedReason::Completed);

    let json = serde_json::to_string(&msg).unwrap();

    // Deserialize in the state crate context (using AgentState's Vec<Msg>)
    let restored: Msg = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.name, "agent");
    assert_eq!(restored.role, Role::Assistant);
    assert_eq!(restored.content.len(), 2);
    assert_eq!(restored.usage.unwrap().input_tokens, 100);
    assert_eq!(
        restored.finished_reason,
        Some(ReplyFinishedReason::Completed)
    );
}

#[test]
fn test_msg_roundtrip_through_state_context() {
    // Create Msg → serialize → push into AgentState → serialize → deserialize → verify
    let mut state = AgentState::new();
    state.reply_context.reply_id = "r1".into();

    let blocks = vec![ContentBlock::Text(TextBlock::new("context message".into()))];
    state.append_context("agent", blocks).unwrap();

    let state_json = serde_json::to_string(&state).unwrap();
    let restored_state: AgentState = serde_json::from_str(&state_json).unwrap();

    assert_eq!(restored_state.context_length(), 1);
    assert_eq!(
        restored_state.context[0].get_text_content(" ").unwrap(),
        "context message"
    );
}

#[test]
fn test_event_from_event_crate_applies_to_msg_from_message_crate() {
    // Create event in event crate
    let base = EventBase::new();
    let event = AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
        base,
        reply_id: "reply-001".into(),
        block_id: "block-001".into(),
        delta: "Hello".into(),
    });

    // Create Msg in message crate and pre-populate with TextBlockStart
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
    msg.id = "reply-001".into();

    // Use event from event crate to modify Msg from message crate
    let base2 = EventBase::new();
    msg.append_event(&AgentEvent::TextBlockStart(TextBlockStartEvent {
        base: base2,
        reply_id: "reply-001".into(),
        block_id: "block-001".into(),
    }))
    .unwrap();
    msg.append_event(&event).unwrap();

    assert_eq!(msg.get_text_content(" ").unwrap(), "Hello");
}

#[test]
fn test_full_cross_crate_event_to_msg_pipeline() {
    // Event crate: create a complete streaming pipeline
    let mut msg = Msg::new("assistant".into(), vec![], Role::Assistant).unwrap();
    msg.id = "reply-full".into();

    let base = EventBase::new();

    // All events created from event crate types
    msg.append_event(&AgentEvent::TextBlockStart(TextBlockStartEvent {
        base: base.clone(),
        reply_id: "reply-full".into(),
        block_id: "b-1".into(),
    }))
    .unwrap();

    msg.append_event(&AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
        base: base.clone(),
        reply_id: "reply-full".into(),
        block_id: "b-1".into(),
        delta: "Hello from event crate!".into(),
    }))
    .unwrap();

    msg.append_event(&AgentEvent::TextBlockEnd(TextBlockEndEvent {
        base: base.clone(),
        reply_id: "reply-full".into(),
        block_id: "b-1".into(),
        text: Some("Hello from event crate!".into()),
    }))
    .unwrap();

    msg.append_event(&AgentEvent::ModelCallEnd(ModelCallEndEvent {
        base: base.clone(),
        reply_id: "reply-full".into(),
        input_tokens: 50,
        output_tokens: 20,
        finished_reason: ReplyFinishedReason::Completed,
    }))
    .unwrap();

    msg.append_event(&AgentEvent::ReplyEnd(ReplyEndEvent {
        base,
        session_id: "s-full".into(),
        reply_id: "reply-full".into(),
        finished_reason: ReplyFinishedReason::Completed,
        error: None,
    }))
    .unwrap();

    // Verify cross-crate consistency
    assert_eq!(
        msg.get_text_content(" ").unwrap(),
        "Hello from event crate!"
    );
    assert!(matches!(
        msg.finished_reason,
        Some(ReplyFinishedReason::Completed)
    ));
    assert!(msg.usage.is_some());
    assert_eq!(msg.usage.unwrap().input_tokens, 50);
}

#[test]
fn test_error_info_serialized_in_types_crate_used_in_message_crate() {
    // ErrorInfo is defined in agent_scope_types but used in agent_scope_message
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();

    let error = ErrorInfo {
        error_type: ErrorType::Upstream,
        message: "Provider unavailable".into(),
    };
    msg.error = Some(error);

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""type":"upstream""#));
    assert!(json.contains(r#""message":"Provider unavailable""#));

    let restored: Msg = serde_json::from_str(&json).unwrap();
    assert!(restored.error.is_some());
    assert_eq!(
        restored.error.as_ref().unwrap().error_type,
        ErrorType::Upstream
    );
}

#[test]
fn test_state_contains_message_is_serializable() {
    // AgentState from state crate contains Vec<Msg> from message crate
    let state = AgentState::with_session_id("cross-crate-test".into());
    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains(r#""session_id":"cross-crate-test""#));

    let restored: AgentState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.session_id, "cross-crate-test");
    assert!(restored.context.is_empty());
}
