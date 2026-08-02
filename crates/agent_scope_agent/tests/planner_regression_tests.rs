use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, AgentError, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_message::Role;
use agent_scope_message::factory::user_msg;

mod mocks;

use mocks::MockModel;

fn make_echo_agent(response_text: &str) -> ReActAgent {
    let model = Arc::new(MockModel::new("mock", response_text));
    let config = AgentConfig::builder()
        .name("planner-regression")
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

#[tokio::test]
async fn react_agent_text_reply_unchanged_without_planner() {
    let agent = make_echo_agent("plain response");
    let input = user_msg("user", "hello").unwrap();

    let reply = agent.reply(Some(vec![input])).await.unwrap();

    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(
        reply.get_text_content(""),
        Some("plain response".to_string())
    );
}

#[tokio::test]
async fn react_agent_observe_and_reply_none_unchanged_without_planner() {
    let agent = make_echo_agent("context response");
    agent
        .observe(Some(vec![user_msg("user", "remember this").unwrap()]))
        .await
        .unwrap();

    let reply = agent.reply(None).await.unwrap();

    assert_eq!(
        reply.get_text_content(""),
        Some("context response".to_string())
    );
}

#[tokio::test]
async fn react_agent_empty_reply_none_still_errors_without_planner() {
    let agent = make_echo_agent("unused");

    let result = agent.reply(None).await;

    assert!(matches!(result, Err(AgentError::NoContentToReply)));
}
