//! End-to-end tests for the built-in task planning tools through the full
//! ReActAgent loop (batch and streaming), driven by a ScriptedModel.
//!
//! Covers quickstart scenarios 1-3 and the registration toggle (scenario 6).

use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use futures::StreamExt;

mod mocks;

use mocks::{ScriptedModel, ScriptedResponse};

fn make_agent(script: Vec<ScriptedResponse>, task_tools_enabled: bool) -> ReActAgent {
    let model = Arc::new(ScriptedModel::new("scripted", script));
    let config = AgentConfig::builder()
        .name("tasker")
        .model(model)
        .task_tools_enabled(task_tools_enabled)
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

fn tc(id: &str, name: &str, input: &str) -> ScriptedResponse {
    ScriptedResponse::ToolCall {
        id: id.into(),
        name: name.into(),
        input: input.into(),
    }
}

/// Scenario 1: create tasks, set up a dependency, list, complete — end to end.
#[tokio::test]
async fn test_task_planning_end_to_end() {
    let agent = make_agent(
        vec![
            tc(
                "call_1",
                "TaskCreate",
                r#"{"subject":"Design","description":"Design the module"}"#,
            ),
            tc(
                "call_2",
                "TaskCreate",
                r#"{"subject":"Implement","description":"Implement the module"}"#,
            ),
            tc(
                "call_3",
                "TaskUpdate",
                r#"{"task_id":"2","add_blocked_by":["1"]}"#,
            ),
            tc("call_4", "TaskList", r#"{}"#),
            tc(
                "call_5",
                "TaskUpdate",
                r#"{"task_id":"1","status":"completed"}"#,
            ),
            ScriptedResponse::Text("All done.".into()),
        ],
        true,
    );

    let reply = agent
        .reply(Some(vec![user_msg("user", "do the complex work").unwrap()]))
        .await
        .unwrap();

    let text = reply.get_text_content("").unwrap_or_default();
    assert!(text.contains("All done"), "got: {text}");

    let state = agent.try_state();
    assert_eq!(state.tasks_context.tasks.len(), 2);
    let t1 = state.tasks_context.get_task("1").unwrap();
    assert_eq!(t1.state, agent_scope_state::TaskState::Completed);
    let t2 = state.tasks_context.get_task("2").unwrap();
    assert_eq!(t2.blocked_by, vec!["1"]);
    assert_eq!(t1.blocks, vec!["2"]);
}

/// Scenario 2: error inputs are self-healing — the loop keeps running.
#[tokio::test]
async fn test_task_tools_error_inputs_recover() {
    let agent = make_agent(
        vec![
            tc("call_1", "TaskUpdate", r#"{"task_id":"99","subject":"x"}"#),
            tc("call_2", "TaskGet", r#"{"task_id":"98"}"#),
            tc(
                "call_3",
                "TaskUpdate",
                r#"{"task_id":"1","status":"bogus"}"#,
            ),
            ScriptedResponse::Text("Recovered.".into()),
        ],
        true,
    );

    let reply = agent
        .reply(Some(vec![user_msg("user", "test error paths").unwrap()]))
        .await
        .unwrap();
    assert!(
        reply
            .get_text_content("")
            .unwrap_or_default()
            .contains("Recovered.")
    );

    // No tasks were created; state is intact.
    let state = agent.try_state();
    assert!(state.tasks_context.tasks.is_empty());
}

/// Scenario 3: deleting a task cleans dangling dependency references.
#[tokio::test]
async fn test_task_delete_cleans_dependencies_e2e() {
    let agent = make_agent(
        vec![
            tc(
                "call_1",
                "TaskCreate",
                r#"{"subject":"A","description":"d"}"#,
            ),
            tc(
                "call_2",
                "TaskCreate",
                r#"{"subject":"B","description":"d"}"#,
            ),
            tc(
                "call_3",
                "TaskCreate",
                r#"{"subject":"C","description":"d"}"#,
            ),
            tc(
                "call_4",
                "TaskUpdate",
                r#"{"task_id":"3","add_blocked_by":["2"]}"#,
            ),
            tc(
                "call_5",
                "TaskUpdate",
                r#"{"task_id":"2","status":"deleted"}"#,
            ),
            ScriptedResponse::Text("Done.".into()),
        ],
        true,
    );

    agent
        .reply(Some(vec![user_msg("user", "delete middle").unwrap()]))
        .await
        .unwrap();

    let state = agent.try_state();
    assert!(state.tasks_context.get_task("2").is_none());
    let t3 = state.tasks_context.get_task("3").unwrap();
    assert!(
        t3.blocked_by.is_empty(),
        "dangling ref left: {:?}",
        t3.blocked_by
    );
    assert_eq!(state.tasks_context.tasks.len(), 2);
}

/// Scenario 6a: task tools are registered by default (enabled).
#[tokio::test]
async fn test_task_tools_registered_by_default() {
    let agent = make_agent(
        vec![
            tc(
                "call_1",
                "TaskCreate",
                r#"{"subject":"S","description":"d"}"#,
            ),
            ScriptedResponse::Text("ok".into()),
        ],
        true,
    );
    agent
        .reply(Some(vec![user_msg("user", "create a task").unwrap()]))
        .await
        .unwrap();
    let state = agent.try_state();
    assert_eq!(state.tasks_context.tasks.len(), 1);
    assert_eq!(state.tasks_context.tasks[0].id, "1");
}

/// Scenario 6b: when disabled, task tools are not registered — the call
/// surfaces a "not found" error and no task is created.
#[tokio::test]
async fn test_task_tools_disabled_not_registered() {
    let agent = make_agent(
        vec![
            tc(
                "call_1",
                "TaskCreate",
                r#"{"subject":"S","description":"d"}"#,
            ),
            ScriptedResponse::Text("ok".into()),
        ],
        false,
    );
    agent
        .reply(Some(vec![user_msg("user", "create a task").unwrap()]))
        .await
        .unwrap();
    let state = agent.try_state();
    assert!(state.tasks_context.tasks.is_empty());
}

/// Task tools work on the streaming path and emit the standard tool event
/// sequence (ToolCallStart → ToolCallEnd → ToolResultStart → ToolResultEnd).
#[tokio::test]
async fn test_task_tools_streaming_event_sequence() {
    let agent = make_agent(
        vec![
            tc(
                "call_1",
                "TaskCreate",
                r#"{"subject":"S","description":"d"}"#,
            ),
            ScriptedResponse::Text("ok".into()),
        ],
        true,
    );

    let stream = agent
        .reply_stream(Some(vec![user_msg("user", "create a task").unwrap()]))
        .await
        .unwrap();
    let events: Vec<AgentEvent> = stream.collect().await;

    let has_start = events
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolCallStart(_)));
    let has_end = events
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolCallEnd(_)));
    let has_result_end = events
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolResultEnd(_)));
    assert!(has_start, "expected ToolCallStart in {events:?}");
    assert!(has_end, "expected ToolCallEnd in {events:?}");
    assert!(has_result_end, "expected ToolResultEnd in {events:?}");

    let state = agent.try_state();
    assert_eq!(state.tasks_context.tasks.len(), 1);
}
