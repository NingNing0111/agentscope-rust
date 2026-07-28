//! Task — agent task tracking.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    InProgress,
    Completed,
}

fn default_task_id() -> String {
    uuid::Uuid::new_v4().as_simple().to_string()
}

fn default_task_state() -> TaskState {
    TaskState::Pending
}

fn default_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// A task that an agent can work on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub subject: String,
    pub description: String,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    #[serde(default = "default_timestamp")]
    pub created_at: String,
    #[serde(default = "default_task_state")]
    pub state: TaskState,
    #[serde(default = "default_task_id")]
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
}

impl Task {
    pub fn new(
        subject: String,
        description: String,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            subject,
            description,
            metadata,
            created_at: default_timestamp(),
            state: TaskState::Pending,
            id: default_task_id(),
            owner: None,
            blocks: Vec::new(),
            blocked_by: Vec::new(),
        }
    }
}

/// Error when task operations fail.
#[derive(Debug, Clone)]
pub enum TaskError {
    NotFound { task_id: String },
    InvalidStateTransition { from: TaskState, to: TaskState },
}

/// Collection of tasks with query methods.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskContext {
    pub tasks: Vec<Task>,
}

impl TaskContext {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    pub fn get_task(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn get_task_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    pub fn update_task_state(&mut self, id: &str, state: TaskState) -> Result<(), TaskError> {
        let task =
            self.tasks
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or_else(|| TaskError::NotFound {
                    task_id: id.to_string(),
                })?;
        task.state = state;
        Ok(())
    }

    pub fn tasks_by_state(&self, state: TaskState) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.state == state).collect()
    }

    pub fn tasks_by_owner(&self, owner: &str) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.owner.as_deref() == Some(owner))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new(
            "Implement login".into(),
            "Add auth flow".into(),
            HashMap::new(),
        );
        assert_eq!(task.subject, "Implement login");
        assert!(matches!(task.state, TaskState::Pending));
        assert!(!task.id.is_empty());
    }

    #[test]
    fn test_task_state_serialization() {
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
    fn test_task_context_operations() {
        let mut ctx = TaskContext::new();
        let task1 = Task::new("task1".into(), "desc".into(), HashMap::new());
        let task2 = Task::new("task2".into(), "desc".into(), HashMap::new());

        ctx.add_task(task1);
        ctx.add_task(task2);

        assert_eq!(ctx.tasks.len(), 2);

        let pending = ctx.tasks_by_state(TaskState::Pending);
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_update_task_state() {
        let mut ctx = TaskContext::new();
        let task = Task::new("t1".into(), "desc".into(), HashMap::new());
        let id = task.id.clone();
        ctx.add_task(task);

        ctx.update_task_state(&id, TaskState::InProgress).unwrap();
        assert_eq!(ctx.get_task(&id).unwrap().state, TaskState::InProgress);
    }

    #[test]
    fn test_task_blocks_and_blocked_by() {
        let mut task = Task::new("t1".into(), "desc".into(), HashMap::new());
        task.blocks.push("t2".to_string());
        task.blocked_by.push("t0".to_string());

        let json = serde_json::to_string(&task).unwrap();
        let restored: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.blocks, vec!["t2"]);
        assert_eq!(restored.blocked_by, vec!["t0"]);
    }
}
