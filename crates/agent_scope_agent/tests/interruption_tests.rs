//! Interruption and cancellation tests — US4.

use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_message::Role;
use agent_scope_message::factory::user_msg;

mod mocks;
use mocks::{CallGate, MockModel};

/// T061: Interrupt during reasoning returns interruption message.
#[tokio::test]
async fn test_interrupt_returns_interruption_message() {
    let model = Arc::new(MockModel::new("mock", "this should not appear"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = ReActAgent::new(
        config,
        ReActConfig {
            interruption_message: "CUSTOM INTERRUPTED".into(),
            ..Default::default()
        },
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    // Interrupt before reply
    agent.interrupt();

    let input = user_msg("user", "should be interrupted").unwrap();
    let reply = agent.reply(Some(vec![input])).await.unwrap();

    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(
        reply.get_text_content(""),
        Some("CUSTOM INTERRUPTED".to_string())
    );
}

/// T063: Resume after interruption — new reply() works normally.
#[tokio::test]
async fn test_resume_after_interruption() {
    let model = Arc::new(MockModel::new("mock", "normal response"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    // First, interrupt
    agent.interrupt();
    let input = user_msg("user", "first").unwrap();
    let reply1 = agent.reply(Some(vec![input])).await.unwrap();
    assert!(reply1.get_text_content("").unwrap().contains("interrupted"));

    // Resume with a new reply — should work
    let input2 = user_msg("user", "second").unwrap();
    let reply2 = agent.reply(Some(vec![input2])).await.unwrap();
    assert_eq!(
        reply2.get_text_content(""),
        Some("normal response".to_string())
    );
}

/// T064: Interrupt before reply starts — reply end emitted immediately.
#[tokio::test]
async fn test_interrupt_before_reply_starts() {
    let model = Arc::new(MockModel::new("mock", "unused"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    // Interrupt before any reply is made
    agent.interrupt();

    let input = user_msg("user", "hello").unwrap();
    let result = agent.reply(Some(vec![input])).await;
    assert!(
        result.is_ok(),
        "Interrupted reply should still return Ok with message"
    );
    let reply = result.unwrap();
    assert!(reply.get_text_content("").unwrap().contains("interrupted"));
}

/// Regression: interrupting a reply *mid-flight* (while the model call is
/// in-progress) must not leak the flag into the next reply. Previously the
/// flag stayed set after the interrupt was handled, so the next `reply()`
/// returned the interruption message without calling the model.
#[tokio::test]
async fn test_mid_reply_interrupt_does_not_leak_into_next_reply() {
    let gate = CallGate::new();
    let model = Arc::new(MockModel::new("mock", "NORMAL RESPONSE").with_gate(gate.clone()));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = Arc::new(
        ReActAgent::new(
            config,
            ReActConfig::default(),
            ContextConfig::default(),
            vec![],
        )
        .unwrap(),
    );

    // Reply 1 runs in the background and blocks inside the model call.
    let input1 = user_msg("user", "first").unwrap();
    let handle = tokio::spawn({
        let agent = agent.clone();
        async move { agent.reply(Some(vec![input1])).await }
    });

    // Wait until the model call is in-flight, then interrupt mid-reply.
    gate.started.notified().await;
    agent.interrupt();

    // Reply 1 terminates with an interruption message.
    let reply1 = handle.await.unwrap().unwrap();
    assert!(
        reply1.get_text_content("").unwrap().contains("interrupted"),
        "reply 1 should be interrupted"
    );
    gate.release();

    // Reply 2 must run normally and reach the model — no leaked flag.
    let input2 = user_msg("user", "second").unwrap();
    let reply2 = agent.reply(Some(vec![input2])).await.unwrap();
    assert_eq!(
        reply2.get_text_content(""),
        Some("NORMAL RESPONSE".to_string()),
        "reply 2 should call the model normally"
    );
}
