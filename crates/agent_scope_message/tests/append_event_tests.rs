//! Integration tests for append_event text streaming, tool call lifecycle,
//! data block streaming, and edge cases.
//! T104

use agent_scope_event::base::EventBase;
use agent_scope_event::{
    AppendEvent, DataBlockDeltaEvent, DataBlockEndEvent, DataBlockStartEvent, ExceedMaxItersEvent,
    ModelCallEndEvent, ModelCallStartEvent, ReplyEndEvent, ReplyStartEvent, TextBlockDeltaEvent,
    TextBlockEndEvent, TextBlockStartEvent, ThinkingBlockDeltaEvent, ThinkingBlockEndEvent,
    ThinkingBlockStartEvent, ToolCallDeltaEvent, ToolCallEndEvent, ToolCallStartEvent,
    UserInterruptEvent,
};
use agent_scope_message::block::{BlockType, ContentBlock, DataSource};
use agent_scope_message::msg::{Msg, Role};
use agent_scope_types::ReplyFinishedReason;

fn make_base() -> EventBase {
    EventBase::new()
}

// ── Full text streaming sequence ─────────────────────────────────────

#[test]
fn test_full_text_streaming_sequence() {
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
    msg.id = "reply-001".into();
    let base = make_base();

    // REPLY_START
    msg.append_event(&agent_scope_event::AgentEvent::ReplyStart(
        ReplyStartEvent {
            base: base.clone(),
            session_id: "s-1".into(),
            reply_id: "reply-001".into(),
            name: "agent".into(),
            role: "assistant".into(),
        },
    ))
    .unwrap();

    // TEXT_BLOCK_START
    msg.append_event(&agent_scope_event::AgentEvent::TextBlockStart(
        TextBlockStartEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
        },
    ))
    .unwrap();

    // Two TEXT_BLOCK_DELTA
    msg.append_event(&agent_scope_event::AgentEvent::TextBlockDelta(
        TextBlockDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            delta: "Hel".into(),
        },
    ))
    .unwrap();
    msg.append_event(&agent_scope_event::AgentEvent::TextBlockDelta(
        TextBlockDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            delta: "lo".into(),
        },
    ))
    .unwrap();

    // TEXT_BLOCK_END
    msg.append_event(&agent_scope_event::AgentEvent::TextBlockEnd(
        TextBlockEndEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
        },
    ))
    .unwrap();

    // MODEL_CALL_END
    msg.append_event(&agent_scope_event::AgentEvent::ModelCallEnd(
        ModelCallEndEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            input_tokens: 100,
            output_tokens: 50,
            finished_reason: ReplyFinishedReason::Completed,
        },
    ))
    .unwrap();

    // REPLY_END
    msg.append_event(&agent_scope_event::AgentEvent::ReplyEnd(ReplyEndEvent {
        base: base.clone(),
        session_id: "s-1".into(),
        reply_id: "reply-001".into(),
        finished_reason: ReplyFinishedReason::Completed,
        error: None,
    }))
    .unwrap();

    // Verify final state
    assert_eq!(msg.get_text_content(" ").unwrap(), "Hello");
    assert!(msg.has_content_blocks(Some(BlockType::Text)));
    assert_eq!(msg.content.len(), 1);
    assert!(matches!(
        msg.finished_reason,
        Some(ReplyFinishedReason::Completed)
    ));
    assert!(msg.usage.is_some());
}

// ── Tool call lifecycle ──────────────────────────────────────────────

#[test]
fn test_tool_call_streaming_lifecycle() {
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
    msg.id = "reply-001".into();
    let base = make_base();

    // TOOL_CALL_START
    msg.append_event(&agent_scope_event::AgentEvent::ToolCallStart(
        ToolCallStartEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-001".into(),
            tool_call_name: "search".into(),
        },
    ))
    .unwrap();

    // Two TOOL_CALL_DELTA
    msg.append_event(&agent_scope_event::AgentEvent::ToolCallDelta(
        ToolCallDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-001".into(),
            delta: "{\"q\":\"".into(),
        },
    ))
    .unwrap();
    msg.append_event(&agent_scope_event::AgentEvent::ToolCallDelta(
        ToolCallDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-001".into(),
            delta: "test\"}".into(),
        },
    ))
    .unwrap();

    // TOOL_CALL_END
    msg.append_event(&agent_scope_event::AgentEvent::ToolCallEnd(
        ToolCallEndEvent {
            base,
            reply_id: "reply-001".into(),
            tool_call_id: "tc-001".into(),
        },
    ))
    .unwrap();

    // Verify ToolCallBlock state transitions
    if let ContentBlock::ToolCall(ref tc) = msg.content[0] {
        assert_eq!(tc.id, "tc-001");
        assert_eq!(tc.name, "search");
        // Delta concatenation: `{"q":"` + `test"}` = `{"q":"test"}`
        assert_eq!(tc.input, "{\"q\":\"test\"}");
        assert_eq!(
            tc.state,
            agent_scope_message::state::ToolCallState::Submitted
        );
        assert!(tc.finished_at.is_some());
    } else {
        panic!("expected ToolCall block");
    }
}

// ── Data block streaming with base64 decode-concat-re-encode ─────────

#[test]
fn test_data_block_streaming_base64_concat() {
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
    msg.id = "reply-001".into();
    let base = make_base();

    // DATA_BLOCK_START
    msg.append_event(&agent_scope_event::AgentEvent::DataBlockStart(
        DataBlockStartEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            media_type: "text/plain".into(),
        },
    ))
    .unwrap();

    // DATA_BLOCK_DELTA 1: "Hel" in base64 = "SGVs"
    msg.append_event(&agent_scope_event::AgentEvent::DataBlockDelta(
        DataBlockDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            data: "SGVs".into(),
            media_type: "text/plain".into(),
        },
    ))
    .unwrap();

    // DATA_BLOCK_DELTA 2: "lo" in base64 = "bG8="
    msg.append_event(&agent_scope_event::AgentEvent::DataBlockDelta(
        DataBlockDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            data: "bG8=".into(),
            media_type: "text/plain".into(),
        },
    ))
    .unwrap();

    // DATA_BLOCK_END
    msg.append_event(&agent_scope_event::AgentEvent::DataBlockEnd(
        DataBlockEndEvent {
            base,
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
        },
    ))
    .unwrap();

    // Verify base64 data was properly concatenated
    if let ContentBlock::Data(ref db) = msg.content[0] {
        if let DataSource::Base64(ref bs) = db.source {
            // "Hello" in base64 = "SGVsbG8="
            assert_eq!(bs.data, "SGVsbG8=");
            assert_eq!(bs.media_type, "text/plain");
        } else {
            panic!("Expected Base64 source");
        }
    } else {
        panic!("Expected Data block");
    }
}

// ── Thinking block streaming ─────────────────────────────────────────

#[test]
fn test_thinking_block_streaming_sequence() {
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
    msg.id = "reply-001".into();
    let base = make_base();

    msg.append_event(&agent_scope_event::AgentEvent::ThinkingBlockStart(
        ThinkingBlockStartEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
        },
    ))
    .unwrap();

    msg.append_event(&agent_scope_event::AgentEvent::ThinkingBlockDelta(
        ThinkingBlockDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            delta: "Let me think...".into(),
        },
    ))
    .unwrap();

    msg.append_event(&agent_scope_event::AgentEvent::ThinkingBlockEnd(
        ThinkingBlockEndEvent {
            base,
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
        },
    ))
    .unwrap();

    if let ContentBlock::Thinking(ref tb) = msg.content[0] {
        assert_eq!(tb.thinking, "Let me think...");
        assert!(tb.finished_at.is_some());
    } else {
        panic!("Expected Thinking block");
    }
}

// ── Edge cases ───────────────────────────────────────────────────────

#[test]
fn test_append_event_user_interrupt_sets_finished_reason() {
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
    msg.append_event(&agent_scope_event::AgentEvent::UserInterrupt(
        UserInterruptEvent {
            base: make_base(),
            reply_id: "reply-001".into(),
        },
    ))
    .unwrap();
    assert!(matches!(
        msg.finished_reason,
        Some(ReplyFinishedReason::Interrupted)
    ));
}

#[test]
fn test_append_event_exceed_max_iters() {
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
    msg.append_event(&agent_scope_event::AgentEvent::ExceedMaxIters(
        ExceedMaxItersEvent {
            base: make_base(),
            reply_id: "reply-001".into(),
            name: "agent".into(),
        },
    ))
    .unwrap();
    assert!(matches!(
        msg.finished_reason,
        Some(ReplyFinishedReason::ExceedMaxIters)
    ));
}

#[test]
fn test_append_event_model_call_end_accumulates_tokens() {
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();

    msg.append_event(&agent_scope_event::AgentEvent::ModelCallEnd(
        ModelCallEndEvent {
            base: make_base(),
            reply_id: "reply-001".into(),
            input_tokens: 100,
            output_tokens: 50,
            finished_reason: ReplyFinishedReason::Completed,
        },
    ))
    .unwrap();

    msg.append_event(&agent_scope_event::AgentEvent::ModelCallEnd(
        ModelCallEndEvent {
            base: make_base(),
            reply_id: "reply-001".into(),
            input_tokens: 50,
            output_tokens: 25,
            finished_reason: ReplyFinishedReason::Completed,
        },
    ))
    .unwrap();

    assert_eq!(msg.usage.as_ref().unwrap().input_tokens, 150);
    assert_eq!(msg.usage.as_ref().unwrap().output_tokens, 75);
}

#[test]
fn test_append_event_reply_end_sets_error_info() {
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
    use agent_scope_types::ErrorInfo;
    use agent_scope_types::ErrorType;

    msg.append_event(&agent_scope_event::AgentEvent::ReplyEnd(ReplyEndEvent {
        base: make_base(),
        session_id: "s-1".into(),
        reply_id: "reply-001".into(),
        finished_reason: ReplyFinishedReason::Error,
        error: Some(ErrorInfo {
            error_type: ErrorType::Upstream,
            message: "Model unavailable".into(),
        }),
    }))
    .unwrap();

    assert!(matches!(
        msg.finished_reason,
        Some(ReplyFinishedReason::Error)
    ));
    assert!(msg.error.is_some());
    let error = msg.error.unwrap();
    assert_eq!(error.error_type, ErrorType::Upstream);
    assert_eq!(error.message, "Model unavailable");
}

#[test]
fn test_append_event_model_call_start_is_noop() {
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
    let result = msg.append_event(&agent_scope_event::AgentEvent::ModelCallStart(
        ModelCallStartEvent {
            base: make_base(),
            reply_id: "reply-001".into(),
            model_name: "gpt-4".into(),
        },
    ));
    assert!(result.is_ok());
    // No state change expected
    assert!(msg.content.is_empty());
}

#[test]
fn test_append_event_control_events_are_noop() {
    use agent_scope_message::block::ToolCallBlock;

    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();

    // RequireUserConfirm
    let result = msg.append_event(&agent_scope_event::AgentEvent::RequireUserConfirm(
        agent_scope_event::RequireUserConfirmEvent {
            base: make_base(),
            reply_id: "r".into(),
            tool_calls: vec![ToolCallBlock::new("tc".into(), "n".into(), "{}".into())],
        },
    ));
    assert!(result.is_ok());

    // Custom event
    let result = msg.append_event(&agent_scope_event::AgentEvent::Custom(
        agent_scope_event::CustomEvent {
            base: make_base(),
            name: "test".into(),
            value: std::collections::HashMap::new(),
        },
    ));
    assert!(result.is_ok());
}

#[test]
fn test_message_with_multiple_content_blocks_mixed() {
    let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
    msg.id = "reply-001".into();
    let base = make_base();

    // Text block
    msg.append_event(&agent_scope_event::AgentEvent::TextBlockStart(
        TextBlockStartEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "t-1".into(),
        },
    ))
    .unwrap();
    msg.append_event(&agent_scope_event::AgentEvent::TextBlockDelta(
        TextBlockDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "t-1".into(),
            delta: "Thinking...".into(),
        },
    ))
    .unwrap();
    msg.append_event(&agent_scope_event::AgentEvent::TextBlockEnd(
        TextBlockEndEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "t-1".into(),
        },
    ))
    .unwrap();

    // Tool call block
    msg.append_event(&agent_scope_event::AgentEvent::ToolCallStart(
        ToolCallStartEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-1".into(),
            tool_call_name: "calculate".into(),
        },
    ))
    .unwrap();
    msg.append_event(&agent_scope_event::AgentEvent::ToolCallDelta(
        ToolCallDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-1".into(),
            delta: r#"{"expr":"2+2"}"#.into(),
        },
    ))
    .unwrap();
    msg.append_event(&agent_scope_event::AgentEvent::ToolCallEnd(
        ToolCallEndEvent {
            base,
            reply_id: "reply-001".into(),
            tool_call_id: "tc-1".into(),
        },
    ))
    .unwrap();

    assert_eq!(msg.content.len(), 2);
    assert!(msg.has_content_blocks(Some(BlockType::Text)));
    assert!(msg.has_content_blocks(Some(BlockType::ToolCall)));
    assert_eq!(msg.get_text_content(" ").unwrap(), "Thinking...");
}
