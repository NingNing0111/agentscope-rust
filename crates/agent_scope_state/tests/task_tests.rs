//! Integration tests for Task and TaskContext.
//! T108

use std::collections::HashMap;

use agent_scope_state::task::{Task, TaskContext, TaskError, TaskState};

fn make_task(subject: &str) -> Task {
    Task::new(subject.into(), format!("desc: {subject}"), HashMap::new())
}

#[test]
fn test_task_creation_fields() {
    let task = make_task("Implement login");
    assert_eq!(task.subject, "Implement login");
    assert_eq!(task.description, "desc: Implement login");
    assert!(matches!(task.state, TaskState::Pending));
    assert!(!task.id.is_empty());
    assert!(task.owner.is_none());
    assert!(task.blocks.is_empty());
    assert!(task.blocked_by.is_empty());
    assert!(task.metadata.is_empty());
}

#[test]
fn test_task_state_serialization_snake_case() {
    assert_eq!(
        serde_json::to_string(&TaskState::Pending).unwrap(),
        r#""pending""#
    );
    assert_eq!(
        serde_json::to_string(&TaskState::InProgress).unwrap(),
        r#""in_progress""#
    );
    assert_eq!(
        serde_json::to_string(&TaskState::Completed).unwrap(),
        r#""completed""#
    );
}

#[test]
fn test_task_state_roundtrip() {
    let states = [
        TaskState::Pending,
        TaskState::InProgress,
        TaskState::Completed,
    ];
    for state in states {
        let json = serde_json::to_string(&state).unwrap();
        let restored: TaskState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, state);
    }
}

#[test]
fn test_task_json_roundtrip_with_dependencies() {
    let mut task = make_task("task1");
    task.owner = Some("alice".into());
    task.state = TaskState::InProgress;
    task.blocks.push("task2".into());
    task.blocks.push("task3".into());
    task.blocked_by.push("task0".into());

    let json = serde_json::to_string(&task).unwrap();
    let restored: Task = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.subject, "task1");
    assert_eq!(restored.owner, Some("alice".into()));
    assert_eq!(restored.state, TaskState::InProgress);
    assert_eq!(restored.blocks, vec!["task2", "task3"]);
    assert_eq!(restored.blocked_by, vec!["task0"]);
}

// ── TaskContext ──────────────────────────────────────────────────────

#[test]
fn test_task_context_add_and_retrieve() {
    let mut ctx = TaskContext::new();
    assert!(ctx.tasks.is_empty());

    let task = make_task("task1");
    let id = task.id.clone();
    ctx.add_task(task);

    assert_eq!(ctx.tasks.len(), 1);
    assert!(ctx.get_task(&id).is_some());
    assert_eq!(ctx.get_task(&id).unwrap().subject, "task1");
}

#[test]
fn test_task_context_get_nonexistent_returns_none() {
    let ctx = TaskContext::new();
    assert!(ctx.get_task("nonexistent").is_none());
}

#[test]
fn test_update_task_state_success() {
    let mut ctx = TaskContext::new();
    let task = make_task("task1");
    let id = task.id.clone();
    ctx.add_task(task);

    ctx.update_task_state(&id, TaskState::InProgress).unwrap();
    assert_eq!(ctx.get_task(&id).unwrap().state, TaskState::InProgress);

    ctx.update_task_state(&id, TaskState::Completed).unwrap();
    assert_eq!(ctx.get_task(&id).unwrap().state, TaskState::Completed);
}

#[test]
fn test_update_task_state_nonexistent_errors() {
    let mut ctx = TaskContext::new();
    let result = ctx.update_task_state("no-such-task", TaskState::InProgress);
    assert!(result.is_err());
    match result.unwrap_err() {
        TaskError::NotFound { task_id } => assert_eq!(task_id, "no-such-task"),
        _ => panic!("expected NotFound error"),
    }
}

#[test]
fn test_tasks_by_state() {
    let mut ctx = TaskContext::new();

    let mut t1 = make_task("pending task");
    t1.state = TaskState::Pending;
    let mut t2 = make_task("in-progress task");
    t2.state = TaskState::InProgress;
    let mut t3 = make_task("completed task");
    t3.state = TaskState::Completed;

    ctx.add_task(t1);
    ctx.add_task(t2);
    ctx.add_task(t3);

    assert_eq!(ctx.tasks_by_state(TaskState::Pending).len(), 1);
    assert_eq!(ctx.tasks_by_state(TaskState::InProgress).len(), 1);
    assert_eq!(ctx.tasks_by_state(TaskState::Completed).len(), 1);
}

#[test]
fn test_tasks_by_owner() {
    let mut ctx = TaskContext::new();

    let mut t1 = make_task("alice task");
    t1.owner = Some("alice".into());
    let mut t2 = make_task("bob task");
    t2.owner = Some("bob".into());
    let t3 = make_task("unassigned");

    ctx.add_task(t1);
    ctx.add_task(t2);
    ctx.add_task(t3);

    assert_eq!(ctx.tasks_by_owner("alice").len(), 1);
    assert_eq!(ctx.tasks_by_owner("bob").len(), 1);
    assert_eq!(ctx.tasks_by_owner("charlie").len(), 0);
}

#[test]
fn test_get_task_mut_allows_modification() {
    let mut ctx = TaskContext::new();
    let task = make_task("mutable task");
    let id = task.id.clone();
    ctx.add_task(task);

    {
        let task = ctx.get_task_mut(&id).unwrap();
        task.subject = "modified".into();
    }

    assert_eq!(ctx.get_task(&id).unwrap().subject, "modified");
}

#[test]
fn test_task_context_default_is_empty() {
    let ctx = TaskContext::default();
    assert!(ctx.tasks.is_empty());
}
