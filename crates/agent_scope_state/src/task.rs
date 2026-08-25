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
    agent_scope_utils::id::generate_id()
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

    /// Derive the next sequential task id.
    ///
    /// Existing ids that look numeric are considered; any non-numeric ids
    /// (e.g. legacy UUIDs) are ignored. Returns `"1"` for an empty set.
    /// Aligns with Python `TaskCreate.call` in `tool/_task/_create_task.py`.
    pub fn next_sequential_id(&self) -> String {
        let max_numeric = self
            .tasks
            .iter()
            .filter_map(|t| t.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        (max_numeric + 1).to_string()
    }

    /// Permanently remove a task and clean all dangling references.
    ///
    /// Removes the task with the given `id` from the collection, then removes
    /// that id from every other task's `blocks` and `blocked_by` lists.
    /// Returns `true` when the task was found and deleted.
    /// Aligns with Python `TaskUpdate` status `deleted` handling.
    pub fn delete_task(&mut self, id: &str) -> bool {
        let Some(index) = self.tasks.iter().position(|t| t.id == id) else {
            return false;
        };
        self.tasks.remove(index);
        for task in &mut self.tasks {
            task.blocks.retain(|b| b != id);
            task.blocked_by.retain(|b| b != id);
        }
        true
    }

    /// Update the block relationship between two tasks bidirectionally.
    ///
    /// Adds `blocked_by_id` to `block_id`'s `blocks` and `block_id` to
    /// `blocked_by_id`'s `blocked_by` (deduplicated). If either id does not
    /// reference an existing task, the write is skipped entirely (no dangling
    /// references). Aligns with Python `_TaskToolBase._update_block_relation`.
    pub fn update_block_relation(&mut self, block_id: &str, blocked_by_id: &str) {
        let both_exist = self.tasks.iter().any(|t| t.id == block_id)
            && self.tasks.iter().any(|t| t.id == blocked_by_id);
        if !both_exist {
            return;
        }
        for task in &mut self.tasks {
            if task.id == block_id && !task.blocks.iter().any(|b| b == blocked_by_id) {
                task.blocks.push(blocked_by_id.to_string());
            }
            if task.id == blocked_by_id && !task.blocked_by.iter().any(|b| b == block_id) {
                task.blocked_by.push(block_id.to_string());
            }
        }
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

    #[test]
    fn test_next_sequential_id_empty() {
        let ctx = TaskContext::new();
        assert_eq!(ctx.next_sequential_id(), "1");
    }

    #[test]
    fn test_next_sequential_id_numeric_max_plus_one() {
        let mut ctx = TaskContext::new();
        for id in ["1", "2", "5"] {
            let mut task = Task::new(format!("task{id}"), "desc".into(), HashMap::new());
            task.id = id.to_string();
            ctx.add_task(task);
        }
        assert_eq!(ctx.next_sequential_id(), "6");
    }

    #[test]
    fn test_next_sequential_id_ignores_non_numeric() {
        let mut ctx = TaskContext::new();
        let mut t1 = Task::new("a".into(), "desc".into(), HashMap::new());
        t1.id = "abc-uuid-like".to_string();
        let mut t2 = Task::new("b".into(), "desc".into(), HashMap::new());
        t2.id = "3".to_string();
        ctx.add_task(t1);
        ctx.add_task(t2);
        assert_eq!(ctx.next_sequential_id(), "4");
    }

    #[test]
    fn test_delete_task_cleans_references() {
        let mut ctx = TaskContext::new();
        let mut t1 = Task::new("t1".into(), "d".into(), HashMap::new());
        t1.id = "1".to_string();
        t1.blocks = vec!["2".to_string()];
        let mut t2 = Task::new("t2".into(), "d".into(), HashMap::new());
        t2.id = "2".to_string();
        t2.blocked_by = vec!["1".to_string()];
        t2.blocks = vec!["3".to_string()];
        let mut t3 = Task::new("t3".into(), "d".into(), HashMap::new());
        t3.id = "3".to_string();
        t3.blocked_by = vec!["2".to_string()];
        ctx.add_task(t1);
        ctx.add_task(t2);
        ctx.add_task(t3);

        assert!(ctx.delete_task("2"));
        assert!(ctx.get_task("2").is_none());
        assert!(ctx.get_task("1").unwrap().blocks.is_empty());
        assert!(ctx.get_task("3").unwrap().blocked_by.is_empty());
    }

    #[test]
    fn test_delete_task_not_found() {
        let mut ctx = TaskContext::new();
        assert!(!ctx.delete_task("nonexistent"));
    }

    #[test]
    fn test_update_block_relation_bidirectional() {
        let mut ctx = TaskContext::new();
        let mut t1 = Task::new("t1".into(), "d".into(), HashMap::new());
        t1.id = "1".to_string();
        let mut t2 = Task::new("t2".into(), "d".into(), HashMap::new());
        t2.id = "2".to_string();
        ctx.add_task(t1);
        ctx.add_task(t2);

        ctx.update_block_relation("1", "2");
        assert_eq!(ctx.get_task("1").unwrap().blocks, vec!["2"]);
        assert_eq!(ctx.get_task("2").unwrap().blocked_by, vec!["1"]);

        // Deduplicated on repeat
        ctx.update_block_relation("1", "2");
        assert_eq!(ctx.get_task("1").unwrap().blocks.len(), 1);
        assert_eq!(ctx.get_task("2").unwrap().blocked_by.len(), 1);
    }

    #[test]
    fn test_update_block_relation_ignores_missing_ids() {
        let mut ctx = TaskContext::new();
        let mut t1 = Task::new("t1".into(), "d".into(), HashMap::new());
        t1.id = "1".to_string();
        ctx.add_task(t1);

        ctx.update_block_relation("1", "ghost");
        assert!(ctx.get_task("1").unwrap().blocks.is_empty());
        ctx.update_block_relation("ghost", "1");
        assert!(ctx.get_task("1").unwrap().blocked_by.is_empty());
    }
}
