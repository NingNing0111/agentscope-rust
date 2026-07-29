//! Integration tests for ReActAgent — US1: Basic text agent.
//!
//! Tests: reply, observe, reply_stream, event sequences, and edge cases.

use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, AgentError, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_message::Role;
use agent_scope_message::factory::user_msg;

mod mocks; // re-use the MockModel and ScriptedModel

use mocks::MockModel;

fn make_echo_agent(response_text: &str) -> ReActAgent {
    let model = Arc::new(MockModel::new("mock", response_text));
    let config = AgentConfig::builder()
        .name("echo-bot")
        .model(model)
        .build()
        .unwrap();
    ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap()
}

/// T024: Basic text reply — MockModel returns "Hello, world!", verify final Msg.
#[tokio::test]
async fn test_basic_text_reply() {
    let agent = make_echo_agent("Hello, world!");
    let input = user_msg("user", "Hi!").unwrap();
    let reply = agent.reply(Some(vec![input])).await.unwrap();

    assert_eq!(reply.role, Role::Assistant);
    let text = reply.get_text_content("");
    assert_eq!(text, Some("Hello, world!".to_string()));
}

/// T026: reply(None) with empty context returns Err(NoContentToReply).
#[tokio::test]
async fn test_reply_none_empty_context() {
    let agent = make_echo_agent("ignored");
    let result = agent.reply(None).await;
    assert!(matches!(result, Err(AgentError::NoContentToReply)));
}

/// T027: reply(None) with existing context proceeds normally.
#[tokio::test]
async fn test_reply_none_existing_context() {
    let agent = make_echo_agent("response to existing");
    // First, observe a message to populate context
    let input = user_msg("user", "context message").unwrap();
    agent.observe(Some(vec![input])).await.unwrap();

    // reply(None) should use existing context
    let reply = agent.reply(None).await.unwrap();
    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(
        reply.get_text_content(""),
        Some("response to existing".to_string())
    );
}

/// T028: observe() appends messages to context without triggering reply.
#[tokio::test]
async fn test_observe_appends_context() {
    let agent = make_echo_agent("irrelevant");
    let msg_count_before = agent.try_state().context.len();

    let input = user_msg("user", "observed message").unwrap();
    agent.observe(Some(vec![input])).await.unwrap();

    let msg_count_after = agent.try_state().context.len();
    assert_eq!(msg_count_after, msg_count_before + 1);
}

/// T029: reply_stream() yields all events and final ReplyEnd.
#[tokio::test]
async fn test_reply_stream_yields_events() {
    use futures::StreamExt;

    let agent = make_echo_agent("streamed reply");
    let input = user_msg("user", "hello").unwrap();
    let stream = agent.reply_stream(Some(vec![input])).await.unwrap();

    tokio::pin!(stream);
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should at least include ReplyStart and ReplyEnd
    let has_reply_start = events
        .iter()
        .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyStart(_)));
    let has_reply_end = events
        .iter()
        .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyEnd(_)));
    let has_text = events
        .iter()
        .any(|e| matches!(e, agent_scope_event::AgentEvent::TextBlockDelta(_)));

    assert!(has_reply_start, "should have ReplyStart event");
    assert!(has_reply_end, "should have ReplyEnd event");
    assert!(has_text, "should have TextBlockDelta event");
}

/// T030: Empty model response handled gracefully.
#[tokio::test]
async fn test_empty_model_response() {
    let agent = make_echo_agent(""); // empty text response
    let input = user_msg("user", "anything").unwrap();
    let reply = agent.reply(Some(vec![input])).await.unwrap();

    assert_eq!(reply.role, Role::Assistant);
    // Empty text is still a valid response
}
