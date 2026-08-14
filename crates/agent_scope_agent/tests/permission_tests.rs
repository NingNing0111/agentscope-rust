//! Permission integration tests for tool execution.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_scope_agent::{
    Agent, AgentConfig, ContextConfig, PermissionContext, PermissionRule, ReActAgent, ReActConfig,
};
use agent_scope_event::AgentEvent;
use agent_scope_message::{ToolCallBlock, ToolCallState, ToolResultState, factory::user_msg};
use agent_scope_tool::{FunctionTool, ToolKit};
use futures::StreamExt;

mod mocks;

use mocks::{MockStreamingModel, ScriptedModel, ScriptedResponse};

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

fn agent_with_scripted_permission(
    rule: PermissionRule,
    calls: Arc<AtomicUsize>,
    script: Vec<ScriptedResponse>,
) -> ReActAgent {
    let model = Arc::new(ScriptedModel::new("scripted", script));

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

fn agent_with_permission(rule: PermissionRule, calls: Arc<AtomicUsize>) -> ReActAgent {
    agent_with_scripted_permission(
        rule,
        calls,
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "dangerous_tool".into(),
                input: r#"{"cmd":"rm -rf /tmp/demo"}"#.into(),
            },
            ScriptedResponse::Text("done".into()),
        ],
    )
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
    // Feature 032 (Python-aligned): Ask pauses the reply — the stream ends
    // WITHOUT a denied tool result and without a ReplyEnd, so the host can
    // resume with a UserConfirmResultEvent.
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolResultEnd(end) if end.state == ToolResultState::Denied)),
        "暂停时不应喂 denied 给模型"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::ReplyEnd(_))),
        "暂停时不应有 ReplyEnd"
    );
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

#[tokio::test]
async fn ask_rule_blocks_execution_in_batch_reply_by_pausing() {
    let calls = Arc::new(AtomicUsize::new(0));
    let agent = agent_with_scripted_permission(
        PermissionRule::ask("dangerous_tool"),
        Arc::clone(&calls),
        vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "dangerous_tool".into(),
                input: r#"{"cmd":"rm -rf /tmp/demo"}"#.into(),
            },
            ScriptedResponse::Text("done".into()),
        ],
    );

    let reply = agent
        .reply(Some(vec![user_msg("user", "run it").unwrap()]))
        .await
        .unwrap();

    // Feature 032 (Python-aligned): batch Ask pauses the reply — the tool is
    // NOT executed and NO denied result is fed back; the tool call stays
    // `asking` in context so the host can resume with a UserConfirmResultEvent.
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let state = agent.state();
    assert!(
        !state.context.iter().any(|msg| {
            msg.content.iter().any(|block| {
                matches!(
                    block,
                    agent_scope_message::ContentBlock::ToolResult(result)
                        if result.state == ToolResultState::Denied
                )
            })
        }),
        "batch Ask 不应喂 denied tool_result"
    );
    assert!(
        state.context.iter().any(|msg| {
            msg.content.iter().any(|block| {
                matches!(
                    block,
                    agent_scope_message::ContentBlock::ToolCall(tc)
                        if tc.state == ToolCallState::Asking
                )
            })
        }),
        "batch Ask 应把 tool_call 置为 asking"
    );
    assert_eq!(reply.name, "assistant");
}

#[tokio::test]
async fn ask_rule_streaming_emits_confirmation_and_pauses_without_internal_wait() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut chunk = agent_scope_model::ChatResponse::default();
    chunk
        .content
        .push(agent_scope_message::ContentBlock::ToolCall(
            ToolCallBlock::new(
                "tc1".into(),
                "dangerous_tool".into(),
                r#"{"cmd":"rm -rf /tmp/demo"}"#.into(),
            ),
        ));
    let model = Arc::new(MockStreamingModel::new("streaming", vec![chunk]));
    let mut toolkit = ToolKit::new();
    toolkit.register(counted_tool("dangerous_tool", Arc::clone(&calls)));
    let mut permission_context = PermissionContext::default();
    permission_context.add_rule(PermissionRule::ask("dangerous_tool"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(toolkit)
        .permission_context(permission_context)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "run it").unwrap()]))
        .await
        .unwrap();
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::RequireUserConfirm(confirm)
            if confirm.tool_calls.len() == 1 && confirm.tool_calls[0].name == "dangerous_tool"
    )));
    // Feature 032: Ask pauses the streaming reply — no denied result, no ReplyEnd.
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolResultEnd(end) if end.state == ToolResultState::Denied)),
        "暂停时不应喂 denied 给模型"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::ReplyEnd(_))),
        "暂停时不应有 ReplyEnd"
    );
}

#[tokio::test]
async fn react_agent_rejects_custom_tool_using_reserved_task_tool_name() {
    let model = Arc::new(ScriptedModel::new("scripted", vec![]));
    let mut toolkit = ToolKit::new();
    toolkit.register(counted_tool("TaskCreate", Arc::new(AtomicUsize::new(0))));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(toolkit)
        .build()
        .unwrap();

    let err = match ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    ) {
        Ok(_) => panic!("expected reserved task tool name to be rejected"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        agent_scope_agent::AgentError::InvalidConfig { field, message }
            if field == "toolkit" && message.contains("reserved built-in task tool name 'TaskCreate'")
    ));
}

#[tokio::test]
async fn reserved_task_tool_name_can_be_custom_when_task_tools_disabled() {
    let calls = Arc::new(AtomicUsize::new(0));
    let model = Arc::new(ScriptedModel::new("scripted", vec![]));
    let mut toolkit = ToolKit::new();
    toolkit.register(counted_tool("TaskCreate", Arc::clone(&calls)));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(toolkit)
        .task_tools_enabled(false)
        .build()
        .unwrap();

    ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();
}
