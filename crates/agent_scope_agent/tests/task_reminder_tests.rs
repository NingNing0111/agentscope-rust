//! Tests for the task reminder injection (quickstart scenario 5) plus the
//! loop wiring, verifying `contracts/task-reminder.md`.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use agent_scope_agent::task_reminder::maybe_inject_task_reminder;
use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_message::{ContentBlock, Msg, Role, ToolCallBlock};
use agent_scope_state::{AgentState, Task, TaskState};

mod mocks;

use mocks::{ScriptedModel, ScriptedResponse};

fn task(id: &str, state: TaskState) -> Task {
    let mut t = Task::new(format!("task {id}"), "desc".into(), HashMap::new());
    t.id = id.to_string();
    t.state = state;
    t
}

fn state_with(tasks: Vec<Task>, context: Vec<Msg>) -> Arc<RwLock<AgentState>> {
    let mut s = AgentState::new();
    for t in tasks {
        s.tasks_context.add_task(t);
    }
    s.context = context;
    Arc::new(RwLock::new(s))
}

fn assistant_msg(blocks: Vec<ContentBlock>) -> Msg {
    Msg::new("assistant".into(), blocks, Role::Assistant).unwrap()
}

fn plain_user_msg(text: &str) -> Msg {
    Msg::new(
        "user".into(),
        vec![ContentBlock::Text(agent_scope_message::TextBlock::new(
            text.into(),
        ))],
        Role::User,
    )
    .unwrap()
}

const SOURCE: &str = r#"{"label": "System", "sublabel": "Runtime State"}"#;

fn last_content(state: &RwLock<AgentState>) -> Option<Vec<ContentBlock>> {
    state
        .read()
        .unwrap()
        .context
        .last()
        .map(|m| m.content.clone())
}

#[test]
fn test_injects_when_unfinished_and_unaware() {
    let state = state_with(
        vec![
            task("1", TaskState::Pending),
            task("2", TaskState::InProgress),
        ],
        vec![plain_user_msg("hello")],
    );
    assert!(maybe_inject_task_reminder(&state, "agent"));
    let content = last_content(&state).expect("reminder appended");
    let block = content
        .iter()
        .find_map(|b| match b {
            ContentBlock::Hint(h) => Some(h),
            _ => None,
        })
        .expect("hint block present");
    assert_eq!(block.source.as_deref(), Some(SOURCE));
    match &block.hint {
        agent_scope_message::HintContent::Text(t) => {
            assert!(
                t.contains("<tasks>You have 1 in-progress tasks and 1 pending tasks. Use `TaskList` to view them if you don't know.</tasks>"),
                "got: {t}"
            );
            assert!(t.starts_with("<system-reminder>"));
            assert!(t.ends_with("</system-reminder>"));
        }
        _ => panic!("expected text hint"),
    }
}

#[test]
fn test_no_inject_when_all_completed() {
    let state = state_with(
        vec![task("1", TaskState::Completed)],
        vec![plain_user_msg("hi")],
    );
    assert!(!maybe_inject_task_reminder(&state, "agent"));
    assert_eq!(state.read().unwrap().context.len(), 1);
}

#[test]
fn test_no_inject_when_no_tasks() {
    let state = state_with(vec![], vec![plain_user_msg("hi")]);
    assert!(!maybe_inject_task_reminder(&state, "agent"));
}

#[test]
fn test_no_inject_when_aware_by_tool_call() {
    let tc = ToolCallBlock::new("c1".into(), "TaskCreate".into(), "{}".into());
    let context = vec![
        plain_user_msg("hi"),
        assistant_msg(vec![ContentBlock::ToolCall(tc)]),
    ];
    let state = state_with(vec![task("1", TaskState::Pending)], context);
    assert!(!maybe_inject_task_reminder(&state, "agent"));
    assert_eq!(state.read().unwrap().context.len(), 2);
}

#[test]
fn test_no_inject_when_aware_by_previous_reminder() {
    use agent_scope_message::{HintBlock, HintContent};
    let reminder = assistant_msg(vec![ContentBlock::Hint(HintBlock {
        hint: HintContent::Text(
            "<tasks>You have 0 in-progress tasks and 1 pending tasks.</tasks>".into(),
        ),
        source: Some(SOURCE.to_string()),
        id: "h1".into(),
        created_at: String::new(),
        finished_at: None,
    })]);
    let state = state_with(
        vec![task("1", TaskState::Pending)],
        vec![plain_user_msg("hi"), reminder],
    );
    assert!(!maybe_inject_task_reminder(&state, "agent"));
}

#[test]
fn test_injects_only_once_when_called_again() {
    let state = state_with(
        vec![task("1", TaskState::Pending)],
        vec![plain_user_msg("hi")],
    );
    assert!(maybe_inject_task_reminder(&state, "agent"));
    // The just-injected reminder makes the context aware → no second injection
    assert!(!maybe_inject_task_reminder(&state, "agent"));
}

#[tokio::test]
async fn test_disabled_flag_via_loop_does_not_inject() {
    // Loop wiring: with the flag off, no reminder is injected even when the
    // model leaves unfinished tasks behind in later replies.
    let model = Arc::new(ScriptedModel::new(
        "scripted",
        vec![
            ScriptedResponse::ToolCall {
                id: "c1".into(),
                name: "TaskCreate".into(),
                input: r#"{"subject":"S","description":"d"}"#.into(),
            },
            ScriptedResponse::Text("ok".into()),
        ],
    ));
    let config = AgentConfig::builder()
        .name("a")
        .model(model)
        .task_tools_enabled(false)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();
    agent
        .reply(Some(vec![
            agent_scope_message::factory::user_msg("user", "x").unwrap(),
        ]))
        .await
        .unwrap();

    // Nothing in the context is a reminder hint with our source.
    let ctx = agent.try_state().context.clone();
    assert!(!ctx.iter().any(|m| {
        m.content
            .iter()
            .any(|b| matches!(b, ContentBlock::Hint(h) if h.source.as_deref() == Some(SOURCE)))
    }));
}
