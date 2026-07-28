//! Integration tests for AgentState creation and manipulation.
//! T107

use agent_scope_message::block::{ContentBlock, TextBlock, ToolCallBlock};
use agent_scope_message::msg::{Msg, Role};
use agent_scope_message::state::ToolCallState;
use agent_scope_state::{AgentState, AppendContextError, ReplyContext, SummaryContent};

#[test]
fn test_agent_state_creation_has_auto_session_id() {
    let state = AgentState::new();
    assert!(!state.session_id.is_empty());
    assert_eq!(state.context_length(), 0);
    assert_eq!(state.reply_context.cur_iter, 0);
}

#[test]
fn test_agent_state_with_custom_session_id() {
    let state = AgentState::with_session_id("my-session".into());
    assert_eq!(state.session_id, "my-session");
}

#[test]
fn test_agent_state_default() {
    let state = AgentState::default();
    assert!(!state.session_id.is_empty());
}

#[test]
fn test_append_context_creates_new_assistant_message() {
    let mut state = AgentState::new();
    state.reply_context.reply_id = "reply-001".into();

    let blocks = vec![ContentBlock::Text(TextBlock::new("hello".into()))];
    state.append_context("agent", blocks).unwrap();

    assert_eq!(state.context_length(), 1);
    let msg = &state.context[0];
    assert_eq!(msg.name, "agent");
    assert_eq!(msg.role, Role::Assistant);
    assert_eq!(msg.get_text_content(" ").unwrap(), "hello");
}

#[test]
fn test_append_context_appends_to_existing_tail_message() {
    let mut state = AgentState::new();
    let reply_id = "reply-001";
    state.reply_context.reply_id = reply_id.into();

    // Create a matching tail assistant message
    let mut msg = Msg::new(
        "agent".into(),
        vec![ContentBlock::Text(TextBlock::new("part1".into()))],
        Role::Assistant,
    )
    .unwrap();
    msg.id = reply_id.into();
    state.context.push(msg);

    // Append more text (note the leading space in " part2")
    let blocks = vec![ContentBlock::Text(TextBlock::new("part2".into()))];
    state.append_context("agent", blocks).unwrap();

    assert_eq!(state.context_length(), 1); // still one message
    assert_eq!(
        state.context[0].get_text_content(" ").unwrap(),
        "part1 part2"
    );
}

#[test]
fn test_append_context_different_name_creates_new_message() {
    let mut state = AgentState::new();
    state.reply_context.reply_id = "reply-001".into();

    // Create a message from 'agent1'
    let mut msg = Msg::new(
        "agent1".into(),
        vec![ContentBlock::Text(TextBlock::new("from 1".into()))],
        Role::Assistant,
    )
    .unwrap();
    msg.id = "reply-001".into();
    state.context.push(msg);

    // Append from 'agent2' (different name)
    let blocks = vec![ContentBlock::Text(TextBlock::new("from 2".into()))];
    state.append_context("agent2", blocks).unwrap();

    assert_eq!(state.context_length(), 2); // new message created
}

#[test]
fn test_append_context_rejects_on_context_full() {
    let mut state = AgentState::new();
    state.set_max_context_messages(Some(1));

    state
        .append_context(
            "agent",
            vec![ContentBlock::Text(TextBlock::new("msg1".into()))],
        )
        .unwrap();

    let result = state.append_context(
        "agent",
        vec![ContentBlock::Text(TextBlock::new("msg2".into()))],
    );
    assert!(result.is_err());
    match result.unwrap_err() {
        AppendContextError::ContextFull { max_messages, .. } => {
            assert_eq!(max_messages, 1);
        }
    }
}

#[test]
fn test_has_awaiting_tool_calls_no_messages() {
    let state = AgentState::new();
    assert!(!state.has_awaiting_tool_calls("agent"));
}

#[test]
fn test_has_awaiting_tool_calls_detects_asking_state() {
    let mut state = AgentState::new();
    let reply_id = "reply-001";
    state.reply_context.reply_id = reply_id.into();

    let mut tc = ToolCallBlock::new("tc-1".into(), "search".into(), "{}".into());
    tc.state = ToolCallState::Asking;

    let mut msg = Msg::new(
        "agent".into(),
        vec![ContentBlock::ToolCall(tc)],
        Role::Assistant,
    )
    .unwrap();
    msg.id = reply_id.into();
    state.context.push(msg);

    assert!(state.has_awaiting_tool_calls("agent"));
    let awaiting = state.get_awaiting_tool_calls("agent");
    assert_eq!(awaiting.len(), 1);
    assert_eq!(awaiting[0].name, "search");
}

#[test]
fn test_has_awaiting_tool_calls_detects_submitted_without_result() {
    let mut state = AgentState::new();
    let reply_id = "reply-001";
    state.reply_context.reply_id = reply_id.into();

    let mut tc = ToolCallBlock::new("tc-1".into(), "search".into(), "{}".into());
    tc.state = ToolCallState::Submitted;

    let mut msg = Msg::new(
        "agent".into(),
        vec![ContentBlock::ToolCall(tc)],
        Role::Assistant,
    )
    .unwrap();
    msg.id = reply_id.into();
    state.context.push(msg);

    assert!(state.has_awaiting_tool_calls("agent"));
}

#[test]
fn test_has_awaiting_tool_calls_ignores_finished_state() {
    let mut state = AgentState::new();
    let reply_id = "reply-001";
    state.reply_context.reply_id = reply_id.into();

    let mut tc = ToolCallBlock::new("tc-1".into(), "search".into(), "{}".into());
    tc.state = ToolCallState::Finished;

    let mut msg = Msg::new(
        "agent".into(),
        vec![ContentBlock::ToolCall(tc)],
        Role::Assistant,
    )
    .unwrap();
    msg.id = reply_id.into();
    state.context.push(msg);

    assert!(!state.has_awaiting_tool_calls("agent"));
    assert!(state.get_awaiting_tool_calls("agent").is_empty());
}

#[test]
fn test_has_awaiting_tool_calls_wrong_name() {
    let mut state = AgentState::new();
    let reply_id = "reply-001";
    state.reply_context.reply_id = reply_id.into();

    let mut tc = ToolCallBlock::new("tc-1".into(), "search".into(), "{}".into());
    tc.state = ToolCallState::Asking;

    let mut msg = Msg::new(
        "agent_bob".into(),
        vec![ContentBlock::ToolCall(tc)],
        Role::Assistant,
    )
    .unwrap();
    msg.id = reply_id.into();
    state.context.push(msg);

    // Querying with wrong name should return false
    assert!(!state.has_awaiting_tool_calls("agent_alice"));
}

#[test]
fn test_set_max_context_messages() {
    let mut state = AgentState::new();
    assert!(state.max_context_messages.is_none());

    state.set_max_context_messages(Some(50));
    assert_eq!(state.max_context_messages, Some(50));

    state.set_max_context_messages(None);
    assert!(state.max_context_messages.is_none());
}

#[test]
fn test_context_length_tracks_messages() {
    let mut state = AgentState::new();
    assert_eq!(state.context_length(), 0);

    state
        .append_context(
            "agent",
            vec![ContentBlock::Text(TextBlock::new("a".into()))],
        )
        .unwrap();
    assert_eq!(state.context_length(), 1);

    state
        .append_context(
            "agent2",
            vec![ContentBlock::Text(TextBlock::new("b".into()))],
        )
        .unwrap();
    assert_eq!(state.context_length(), 2);
}

// ── ReplyContext ─────────────────────────────────────────────────────

#[test]
fn test_reply_context_default() {
    let rc = ReplyContext::default();
    assert!(!rc.reply_id.is_empty());
    assert_eq!(rc.cur_iter, 0);
    assert!(rc.structured_schema.is_none());
    assert!(rc.structured_output.is_none());
}

// ── SummaryContent ───────────────────────────────────────────────────

#[test]
fn test_summary_content_text_variant() {
    let sc = SummaryContent::Text("summary text".into());
    let json = serde_json::to_string(&sc).unwrap();
    assert_eq!(json, r#""summary text""#);

    let restored: SummaryContent = serde_json::from_str(&json).unwrap();
    match restored {
        SummaryContent::Text(t) => assert_eq!(t, "summary text"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_summary_content_default_is_empty_text() {
    let sc = SummaryContent::default();
    match sc {
        SummaryContent::Text(t) => assert!(t.is_empty()),
        _ => panic!("expected Text"),
    }
}
