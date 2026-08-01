//! Permission integration tests for tool execution.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_scope_agent::{
    Agent, AgentConfig, ContextConfig, PermissionContext, PermissionRule, ReActAgent, ReActConfig,
};
use agent_scope_event::AgentEvent;
use agent_scope_message::{ToolResultState, factory::user_msg};
use agent_scope_tool::{FunctionTool, ToolKit};
use futures::StreamExt;

mod mocks;

use mocks::{ScriptedModel, ScriptedResponse};

fn counted_tool(name: &str, calls: Arc<AtomicUsize>) -> FunctionTool {
    FunctionTool::new_with_schema(
        name,
        "Count invocations",
        serde_json::json!({
            "type": "object",
            "properties": {},
        }),
        move |_input| {
            let calls = Arc::clone(&calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                "executed".to_string()
            }
        },
    )
}

fn agent_with_permission(rule: PermissionRule, calls: Arc<AtomicUsize>) -> ReActAgent {
    let model = Arc::new(ScriptedModel::new(
        "scripted",
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "dangerous_tool".into(),
                input: r#"{"cmd":"rm -rf /tmp/demo"}"#.into(),
            },
            ScriptedResponse::Text("done".into()),
        ],
    ));

    let mut toolkit = ToolKit::new();
    toolkit.register(counted_tool("dangerous_tool", calls));

    let mut permission_context = PermissionContext::default();
    permission_context.add_rule(rule);

    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(toolkit)
        .permission_context(permission_context)
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
async fn deny_rule_blocks_tool_execution_in_streaming_reply() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_permission(PermissionRule::deny("dangerous_tool"), Arc::clone(&calls));

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    assert_eq!(calls.load(Ordering::SeqCst), 0, "denied tool must not run");
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolResultEnd(end) if end.state == ToolResultState::Denied
    )));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::RequireUserConfirm(_)))
    );
}

#[tokio::test]
async fn ask_rule_emits_confirmation_and_blocks_execution() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_permission(PermissionRule::ask("dangerous_tool"), Arc::clone(&calls));

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "ask tool must wait instead of running"
    );
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::RequireUserConfirm(confirm)
            if confirm.tool_calls.len() == 1 && confirm.tool_calls[0].name == "dangerous_tool"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolResultEnd(end) if end.state == ToolResultState::Denied
    )));
}

#[tokio::test]
async fn allow_rule_executes_tool() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_permission(PermissionRule::allow("dangerous_tool"), Arc::clone(&calls));

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
        .await
        .unwrap();

    while stream.next().await.is_some() {}

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "allowed tool should run once"
    );
}
