//! Unit contract tests for the built-in task planning tools.
//!
//! Verifies names, descriptions, input schemas, output text and error
//! contracts against `contracts/task-tools.md` (which mirrors the Python
//! reference output text verbatim).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use agent_scope_agent::task_tools::{
    TASK_TOOL_NAMES, TaskCreateTool, TaskGetTool, TaskListTool, TaskUpdateTool,
};
use agent_scope_message::{ToolOutput, ToolResultState};
use agent_scope_state::{AgentState, TaskState};
use agent_scope_tool::{Tool, ToolError, ToolExecOutput};
use serde_json::json;

fn make_state() -> Arc<RwLock<AgentState>> {
    Arc::new(RwLock::new(AgentState::new()))
}

fn state_with_tasks() -> Arc<RwLock<AgentState>> {
    let state = make_state();
    {
        let mut s = state.write().unwrap();
        for (id, subject, owner, blocks, blocked_by) in [
            (
                "1",
                "Do A",
                Some("alice"),
                vec!["2".to_string()],
                Vec::new(),
            ),
            ("2", "Do B", None, Vec::new(), vec!["1".to_string()]),
        ] {
            let mut task =
                agent_scope_state::Task::new(subject.into(), "desc".into(), HashMap::new());
            task.id = id.to_string();
            task.owner = owner.map(String::from);
            task.blocks = blocks;
            task.blocked_by = blocked_by;
            s.tasks_context.add_task(task);
        }
    }
    state
}

/// Extract the text + state from a complete tool result.
fn unwrap_text(out: ToolExecOutput) -> (String, ToolResultState) {
    match out {
        ToolExecOutput::Complete(block) => match block.output {
            ToolOutput::Text(t) => (t, block.state),
            _ => panic!("expected Text output"),
        },
        _ => panic!("expected Complete output"),
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

#[test]
fn test_task_tool_names_constant() {
    assert_eq!(
        TASK_TOOL_NAMES,
        ["TaskCreate", "TaskGet", "TaskList", "TaskUpdate"]
    );
}

// ---------------------------------------------------------------------------
// TaskCreate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_create_schema_and_description() {
    let tool = TaskCreateTool::new(make_state());
    assert_eq!(tool.name(), "TaskCreate");
    assert!(
        tool.description()
            .starts_with("Use this tool to create a structured task list")
    );
    let schema = tool.input_schema();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("subject")));
    assert!(required.contains(&json!("description")));
}

#[tokio::test]
async fn test_task_create_assigns_sequential_ids() {
    let state = make_state();
    let tool = TaskCreateTool::new(Arc::clone(&state));

    let (text, st) = unwrap_text(
        tool.call(json!({"subject": "First", "description": "d"}))
            .await
            .unwrap(),
    );
    assert_eq!(st, ToolResultState::Success);
    assert_eq!(text, "Task (id=1) created successfully: First");

    let (text, _) = unwrap_text(
        tool.call(json!({"subject": "Second", "description": "d"}))
            .await
            .unwrap(),
    );
    assert_eq!(text, "Task (id=2) created successfully: Second");

    let s = state.read().unwrap();
    assert_eq!(s.tasks_context.tasks.len(), 2);
    assert_eq!(s.tasks_context.tasks[0].id, "1");
    assert_eq!(s.tasks_context.tasks[0].state, TaskState::Pending);
}

#[tokio::test]
async fn test_task_create_with_metadata() {
    let state = make_state();
    let tool = TaskCreateTool::new(Arc::clone(&state));
    tool.call(json!({"subject": "S", "description": "d", "metadata": {"k": "v", "n": 1}}))
        .await
        .unwrap();
    let s = state.read().unwrap();
    assert_eq!(s.tasks_context.tasks[0].metadata["k"], "v");
    assert_eq!(s.tasks_context.tasks[0].metadata["n"], 1);
}

#[tokio::test]
async fn test_task_create_missing_required_field() {
    let tool = TaskCreateTool::new(make_state());
    let err = tool.call(json!({"subject": "only"})).await.unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput { .. }));
}

// ---------------------------------------------------------------------------
// TaskList
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_list_empty() {
    let tool = TaskListTool::new(make_state());
    let (text, st) = unwrap_text(tool.call(json!({})).await.unwrap());
    assert_eq!(st, ToolResultState::Success);
    assert_eq!(text, "No tasks available.");
}

#[tokio::test]
async fn test_task_list_format_with_owner_and_blocked() {
    let tool = TaskListTool::new(state_with_tasks());
    let (text, st) = unwrap_text(tool.call(json!({})).await.unwrap());
    assert_eq!(st, ToolResultState::Success);
    assert_eq!(
        text,
        "1 [pending] Do A(alice)\n2 [pending] Do B[blocked by 1]"
    );
}

// ---------------------------------------------------------------------------
// TaskGet
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_get_full_details() {
    let tool = TaskGetTool::new(state_with_tasks());
    let (text, st) = unwrap_text(tool.call(json!({"task_id": "1"})).await.unwrap());
    assert_eq!(st, ToolResultState::Success);
    assert_eq!(
        text,
        "Task (id=1): Do A\nStatus: pending\nDescription: desc\nOwner: alice\nBlocks: #2"
    );
}

#[tokio::test]
async fn test_task_get_not_found() {
    let tool = TaskGetTool::new(state_with_tasks());
    let (text, st) = unwrap_text(tool.call(json!({"task_id": "99"})).await.unwrap());
    assert_eq!(st, ToolResultState::Error);
    assert_eq!(text, "Task not found");
}

#[tokio::test]
async fn test_task_get_metadata_rendering() {
    let state = make_state();
    {
        let mut s = state.write().unwrap();
        let mut task = agent_scope_state::Task::new("S".into(), "d".into(), HashMap::new());
        task.id = "1".to_string();
        task.metadata.insert("key".into(), json!("value"));
        s.tasks_context.add_task(task);
    }
    let tool = TaskGetTool::new(state);
    let (text, _) = unwrap_text(tool.call(json!({"task_id": "1"})).await.unwrap());
    assert!(text.contains("Metadata: {'key': 'value'}"), "got: {text}");
}

// ---------------------------------------------------------------------------
// TaskUpdate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_update_subject_description_owner() {
    let tool = TaskUpdateTool::new(state_with_tasks());
    let (text, st) = unwrap_text(
        tool.call(
            json!({"task_id": "1", "subject": "New A", "description": "new desc", "owner": "bob"}),
        )
        .await
        .unwrap(),
    );
    assert_eq!(st, ToolResultState::Success);
    assert_eq!(text, "Update task (id=1) subject, description, owner.");
}

#[tokio::test]
async fn test_task_update_empty_subject_ignored() {
    let tool = TaskUpdateTool::new(state_with_tasks());
    let (text, _) = unwrap_text(
        tool.call(json!({"task_id": "2", "subject": ""}))
            .await
            .unwrap(),
    );
    assert_eq!(
        text,
        "No updates were made to the task (id=2). Make sure you provided at least one field to update and the values are correct."
    );
}

#[tokio::test]
async fn test_task_update_status_completed_appends_hint() {
    let tool = TaskUpdateTool::new(state_with_tasks());
    let (text, _) = unwrap_text(
        tool.call(json!({"task_id": "2", "status": "completed"}))
            .await
            .unwrap(),
    );
    assert!(text.starts_with("Update task (id=2) status."));
    assert!(
        text.ends_with(
            "\n\nTask completed. Call TaskList now to find your next available task or see if your work unblocked others."
        ),
        "got: {text}"
    );
}

#[tokio::test]
async fn test_task_update_blocks_bidirectional() {
    let tool = TaskUpdateTool::new(state_with_tasks());
    // task 1 blocks nothing extra; add_blocks on task 2 -> task 1
    let (text, _) = unwrap_text(
        tool.call(json!({"task_id": "2", "add_blocks": ["1"]}))
            .await
            .unwrap(),
    );
    assert_eq!(text, "Update task (id=2) add_blocks.");

    // Invalid references are ignored entirely
    let (text, _) = unwrap_text(
        tool.call(json!({"task_id": "2", "add_blocked_by": ["1", "ghost"]}))
            .await
            .unwrap(),
    );
    // "1" already in blocked_by, "ghost" doesn't exist -> nothing new added
    assert!(text.contains("No updates were made"));
}

#[tokio::test]
async fn test_task_update_metadata_merge_and_null_delete() {
    let state = make_state();
    {
        let mut s = state.write().unwrap();
        let mut task = agent_scope_state::Task::new("S".into(), "d".into(), HashMap::new());
        task.id = "1".to_string();
        task.metadata.insert("keep".into(), json!(1));
        task.metadata.insert("drop".into(), json!("x"));
        s.tasks_context.add_task(task);
    }
    let tool = TaskUpdateTool::new(Arc::clone(&state));
    let (text, _) = unwrap_text(
        tool.call(json!({"task_id": "1", "metadata": {"drop": null, "add": true}}))
            .await
            .unwrap(),
    );
    assert_eq!(text, "Update task (id=1) metadata.");
    let s = state.read().unwrap();
    let t = s.tasks_context.get_task("1").unwrap();
    assert!(!t.metadata.contains_key("drop"));
    assert_eq!(t.metadata["add"], true);
    assert_eq!(t.metadata["keep"], 1);
}

#[tokio::test]
async fn test_task_update_deleted_removes_and_cleans() {
    let state = state_with_tasks();
    let tool = TaskUpdateTool::new(Arc::clone(&state));
    let (text, st) = unwrap_text(
        tool.call(json!({"task_id": "1", "status": "deleted"}))
            .await
            .unwrap(),
    );
    assert_eq!(st, ToolResultState::Success);
    assert_eq!(text, "Task (id=1) has been deleted.");
    let s = state.read().unwrap();
    assert!(s.tasks_context.get_task("1").is_none());
    // task 2's blocked_by reference to "1" is cleaned up
    assert!(s.tasks_context.get_task("2").unwrap().blocked_by.is_empty());
}

#[tokio::test]
async fn test_task_update_not_found() {
    let tool = TaskUpdateTool::new(state_with_tasks());
    let (text, st) = unwrap_text(
        tool.call(json!({"task_id": "99", "subject": "x"}))
            .await
            .unwrap(),
    );
    assert_eq!(st, ToolResultState::Error);
    assert_eq!(text, "TaskNotFoundError: The task (id=99) does not exist.");
}

#[tokio::test]
async fn test_task_update_invalid_status() {
    let tool = TaskUpdateTool::new(state_with_tasks());
    let err = tool
        .call(json!({"task_id": "1", "status": "bogus"}))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput { .. }));
}

#[tokio::test]
async fn test_task_update_descriptions_match_reference() {
    let create = TaskCreateTool::new(make_state());
    let list = TaskListTool::new(make_state());
    let get = TaskGetTool::new(make_state());
    let update = TaskUpdateTool::new(make_state());
    assert!(
        list.description()
            .starts_with("Use this tool to list all tasks")
    );
    assert!(
        get.description()
            .starts_with("Use this tool to retrieve a task")
    );
    assert!(
        update
            .description()
            .starts_with("Use this tool to update a task")
    );
    assert!(create.description().contains("## Task Fields"));
    assert!(update.description().contains("## Status Workflow"));
}

// ---------------------------------------------------------------------------
// Permission allowlist
// ---------------------------------------------------------------------------

#[test]
fn test_task_tools_always_allowed() {
    use agent_scope_agent::permission::{
        PermissionBehavior, PermissionContext, PermissionEngine, PermissionMode,
    };
    let engine = PermissionEngine::with_context(PermissionContext::new(
        PermissionMode::Explore, // strictest mode — still must allow task tools
    ));
    for name in TASK_TOOL_NAMES {
        let decision = engine.check_decision(name, &json!({}));
        assert_eq!(
            decision.behavior,
            PermissionBehavior::Allow,
            "tool {name} must always be allowed"
        );
        assert_eq!(
            decision.decision_reason.as_deref(),
            Some(format!("{name} is always allowed to be called.").as_str())
        );
    }
}
