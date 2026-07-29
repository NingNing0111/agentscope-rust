//! Event sequence verification tests — US1/US2.
//!
//! Tests that events are emitted in the correct order per AgentScope protocol.

use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_message::factory::user_msg;
use futures::StreamExt;

mod mocks;
use mocks::{MockModel, ScriptedModel, ScriptedResponse};

/// Get a string tag for each event variant for ordering checks.
fn event_tag(event: &agent_scope_event::AgentEvent) -> &'static str {
    match event {
        agent_scope_event::AgentEvent::ReplyStart(_) => "REPLY_START",
        agent_scope_event::AgentEvent::ReplyEnd(_) => "REPLY_END",
        agent_scope_event::AgentEvent::ModelCallStart(_) => "MODEL_CALL_START",
        agent_scope_event::AgentEvent::ModelCallEnd(_) => "MODEL_CALL_END",
        agent_scope_event::AgentEvent::TextBlockStart(_) => "TEXT_BLOCK_START",
        agent_scope_event::AgentEvent::TextBlockDelta(_) => "TEXT_BLOCK_DELTA",
        agent_scope_event::AgentEvent::TextBlockEnd(_) => "TEXT_BLOCK_END",
        agent_scope_event::AgentEvent::ToolCallStart(_) => "TOOL_CALL_START",
        agent_scope_event::AgentEvent::ToolCallEnd(_) => "TOOL_CALL_END",
        agent_scope_event::AgentEvent::ToolResultStart(_) => "TOOL_RESULT_START",
        agent_scope_event::AgentEvent::ToolResultEnd(_) => "TOOL_RESULT_END",
        agent_scope_event::AgentEvent::ExceedMaxIters(_) => "EXCEED_MAX_ITERS",
        agent_scope_event::AgentEvent::UserInterrupt(_) => "USER_INTERRUPT",
        _ => "OTHER",
    }
}

/// T025: Event sequence for text reply — verify exact order.
#[tokio::test]
async fn test_text_reply_event_sequence() {
    let model = Arc::new(MockModel::new("mock", "Hello!"));
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

    let input = user_msg("user", "Hi").unwrap();
    let stream = agent.reply_stream(Some(vec![input])).await.unwrap();

    tokio::pin!(stream);
    let tags: Vec<&str> = {
        let mut tags = Vec::new();
        while let Some(event) = stream.next().await {
            tags.push(event_tag(&event));
        }
        tags
    };

    // Verify order: REPLY_START → MODEL_CALL_START → MODEL_CALL_END →
    // TEXT_BLOCK_START → TEXT_BLOCK_DELTA → TEXT_BLOCK_END → REPLY_END
    assert!(
        tags.starts_with(&["REPLY_START"]),
        "Expected REPLY_START first, got: {tags:?}"
    );
    assert!(
        tags.contains(&"MODEL_CALL_START"),
        "Missing MODEL_CALL_START in {tags:?}"
    );
    assert!(
        tags.contains(&"MODEL_CALL_END"),
        "Missing MODEL_CALL_END in {tags:?}"
    );
    assert!(
        tags.contains(&"TEXT_BLOCK_START"),
        "Missing TEXT_BLOCK_START"
    );
    assert!(
        tags.contains(&"TEXT_BLOCK_DELTA"),
        "Missing TEXT_BLOCK_DELTA"
    );
    assert!(tags.contains(&"TEXT_BLOCK_END"), "Missing TEXT_BLOCK_END");
    assert!(
        tags.ends_with(&["REPLY_END"]),
        "Expected REPLY_END last, got: {tags:?}"
    );
}

/// T037: Tool lifecycle events — ToolCallStart → ToolCallEnd → ToolResultStart → ToolResultEnd.
#[tokio::test]
async fn test_tool_lifecycle_event_sequence() {
    use agent_scope_tool::{FunctionTool, ToolKit};
    use schemars::JsonSchema;
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize, JsonSchema)]
    struct CalcInput {
        a: i32,
        b: i32,
    }
    async fn calc_handler(input: CalcInput) -> String {
        format!("{}", input.a + input.b)
    }

    let calc_tool = FunctionTool::new("calculator", "Add two numbers", calc_handler);
    let mut toolkit = ToolKit::new();
    toolkit.register(calc_tool);

    let script = vec![
        ScriptedResponse::ToolCall {
            id: "tc1".into(),
            name: "calculator".into(),
            input: r#"{"a":1,"b":2}"#.into(),
        },
        ScriptedResponse::Text("The answer is 3".into()),
    ];
    let model = Arc::new(ScriptedModel::new("scripted", script));
    let config = AgentConfig::builder()
        .name("tool-agent")
        .model(model)
        .toolkit(toolkit)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let input = user_msg("user", "What is 1+2?").unwrap();
    let stream = agent.reply_stream(Some(vec![input])).await.unwrap();

    tokio::pin!(stream);
    let tags: Vec<&str> = {
        let mut tags = Vec::new();
        while let Some(event) = stream.next().await {
            tags.push(event_tag(&event));
        }
        tags
    };

    assert!(
        tags.contains(&"TOOL_CALL_START"),
        "Missing TOOL_CALL_START in {tags:?}"
    );
    assert!(tags.contains(&"TOOL_CALL_END"), "Missing TOOL_CALL_END");
    assert!(
        tags.contains(&"TOOL_RESULT_START"),
        "Missing TOOL_RESULT_START"
    );
    assert!(tags.contains(&"TOOL_RESULT_END"), "Missing TOOL_RESULT_END");

    // Tool events must appear after MODEL_CALL and before final REPLY_END
    let tool_start_pos = tags.iter().position(|t| *t == "TOOL_CALL_START").unwrap();
    let model_end_pos = tags.iter().position(|t| *t == "MODEL_CALL_END").unwrap();
    assert!(
        tool_start_pos > model_end_pos,
        "TOOL_CALL_START must be after MODEL_CALL_END"
    );
}
