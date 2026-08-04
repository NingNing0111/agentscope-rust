//! Built-in task planning tools — TaskCreate / TaskList / TaskGet / TaskUpdate.
//!
//! Mirrors the Python AgentScope tools in `agentscope/src/agentscope/tool/_task/`
//! (upstream commit `9d1026fa`). The tools operate on the shared
//! [`AgentState::tasks_context`] through an `Arc<RwLock<AgentState>>` handle
//! installed by [`crate::ReActAgent`] at construction time.
//!
//! Tool names, input schemas, descriptions, output text and error semantics
//! align with the Python reference so golden-snapshot/diff tests can compare
//! them character-for-character.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use agent_scope_state::{AgentState, Task, TaskState};
use agent_scope_tool::{Tool, ToolError, ToolExecOutput};
use serde::Deserialize;
use serde_json::Value as JsonValue;

/// Names of the built-in task planning tools.
pub const TASK_TOOL_NAMES: [&str; 4] = ["TaskCreate", "TaskGet", "TaskList", "TaskUpdate"];

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build a one-shot text tool result with the given state.
fn text_chunk(name: &str, text: String, state: ToolResultState) -> ToolExecOutput {
    ToolExecOutput::Complete(ToolResultBlock {
        id: uuid::Uuid::new_v4().as_simple().to_string(),
        name: name.to_string(),
        output: ToolOutput::Text(text),
        state,
        metadata: HashMap::new(),
        created_at: chrono::Utc::now().to_rfc3339(),
        finished_at: None,
        is_last: true,
    })
}

/// Serialized (snake_case) name of a task state, matching the Python model.
fn state_str(state: TaskState) -> &'static str {
    match state {
        TaskState::Pending => "pending",
        TaskState::InProgress => "in_progress",
        TaskState::Completed => "completed",
    }
}

/// Python-style repr of a scalar JSON value (used for task metadata display).
fn py_value_repr(v: &JsonValue) -> String {
    match v {
        JsonValue::Null => "None".into(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::String(s) => format!("'{s}'"),
        JsonValue::Array(a) => format!(
            "[{}]",
            a.iter().map(py_value_repr).collect::<Vec<_>>().join(", ")
        ),
        JsonValue::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, val)| format!("'{k}': {}", py_value_repr(val)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Python `dict`-style repr of a metadata map, e.g. `{'key': 'value'}`.
///
/// Keys are sorted so the output is deterministic across processes — Rust's
/// `HashMap` iteration order is randomized per process, which made the same
/// task's metadata repr differ between runs and broke the module's
/// golden-snapshot/diff-test goal (round-4 F4).
fn py_dict_repr(map: &HashMap<String, JsonValue>) -> String {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    let items: Vec<String> = keys
        .iter()
        .map(|k| format!("'{k}': {}", py_value_repr(&map[*k])))
        .collect();
    format!("{{{}}}", items.join(", "))
}

// ---------------------------------------------------------------------------
// TaskCreate
// ---------------------------------------------------------------------------

/// Create a task for the agent to perform.
pub struct TaskCreateTool {
    state: Arc<RwLock<AgentState>>,
}

impl TaskCreateTool {
    /// Create a new tool bound to the given shared agent state.
    pub fn new(state: Arc<RwLock<AgentState>>) -> Self {
        Self { state }
    }
}

#[derive(Deserialize)]
struct TaskCreateParams {
    subject: String,
    description: String,
    #[serde(default)]
    metadata: Option<serde_json::Map<String, JsonValue>>,
}

#[async_trait::async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "TaskCreate"
    }

    fn description(&self) -> &str {
        "Use this tool to create a structured task list for your current session. This helps you track progress, organize complex tasks, and demonstrate thoroughness to the user.\nIt also helps the user understand the progress of the task and overall progress of their requests.\n\n## When to Use This Tool\nUse this tool proactively in these scenarios:\n\n- Complex multi-step tasks - When a task requires 3 or more distinct steps or actions\n- Non-trivial and complex tasks - Tasks that require careful planning or multiple operations\n- Plan mode - When using plan mode, create a task list to track the work\n- User explicitly requests todo list - When the user directly asks you to use the todo list\n- User provides multiple tasks - When users provide a list of things to be done (numbered or comma-separated)\n- After receiving new instructions - Immediately capture user requirements as tasks\n- When you start working on a task - Mark it as in_progress BEFORE beginning work\n- After completing a task - Mark it as completed and add any new follow-up tasks discovered during implementation\n\n## When NOT to Use This Tool\n\nSkip using this tool when:\n- There is only **one single, straightforward** task\n- The task is trivial and tracking it provides no organizational benefit\n- The task can be completed in less than 3 trivial steps\n- The task is purely conversational or informational\n\nNOTE that you should **NOT** use this tool if there is only one trivial task to do. In this case you are better off just doing the task directly.\n\n## Task Fields\n\n- **subject**: A brief, actionable title in imperative form (e.g., \"Fix authentication bug in login flow\")\n- **description**: What needs to be done\n\nAll tasks are created with status `pending`.\n\n## Tips\n\n- Create tasks with clear, specific subjects that describe the outcome\n- After creating tasks, use TaskUpdate to set up dependencies (blocks/blockedBy) if needed\n- Check TaskList first to avoid creating duplicate tasks"
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subject": { "type": "string", "description": "A brief title for the task" },
                "description": { "type": "string", "description": "What needs to be done" },
                "metadata": { "type": ["object", "null"], "description": "Arbitrary metadata to attach to the task" }
            },
            "required": ["subject", "description"]
        })
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let params: TaskCreateParams =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool_name: self.name().into(),
                reason: e.to_string(),
            })?;

        let mut state = self.state.write().unwrap();
        let id = state.tasks_context.next_sequential_id();
        let mut task = Task::new(
            params.subject.clone(),
            params.description.clone(),
            params
                .metadata
                .map(|m| m.into_iter().collect())
                .unwrap_or_default(),
        );
        task.id = id.clone();
        state.tasks_context.add_task(task);

        Ok(text_chunk(
            self.name(),
            format!("Task (id={id}) created successfully: {}", params.subject),
            ToolResultState::Success,
        ))
    }
}

// ---------------------------------------------------------------------------
// TaskList
// ---------------------------------------------------------------------------

/// List tasks for the agent to perform.
pub struct TaskListTool {
    state: Arc<RwLock<AgentState>>,
}

impl TaskListTool {
    /// Create a new tool bound to the given shared agent state.
    pub fn new(state: Arc<RwLock<AgentState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "TaskList"
    }

    fn description(&self) -> &str {
        "Use this tool to list all tasks in the task list.\n\n## When to Use This Tool\n- To see what tasks are available to work on (status: 'pending', no owner, not blocked)\n- To check overall progress on the project\n- To find tasks that are blocked and need dependencies resolved\n- After completing a task, to check for newly unblocked work or claim the next available task\n- **Prefer working on tasks in ID order** (lowest ID first) when multiple tasks are available, as earlier tasks often set up context for later ones\n\n## Output\n\nReturns a summary of each task:\n- **id**: Task identifier (use with TaskGet, TaskUpdate)\n- **subject**: Brief description of the task\n- **status**: 'pending', 'in_progress', or 'completed'\n- **owner**: Agent ID if assigned, empty if available\n- **blockedBy**: List of open task IDs that must be resolved first (tasks with blockedBy cannot be claimed until dependencies resolve)\n\nUse TaskGet with a specific task ID to view full details including description and comments."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    async fn call(&self, _input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let state = self.state.read().unwrap();
        if state.tasks_context.tasks.is_empty() {
            return Ok(text_chunk(
                self.name(),
                "No tasks available.".into(),
                ToolResultState::Success,
            ));
        }

        let mut lines = Vec::new();
        for task in &state.tasks_context.tasks {
            let owner = task
                .owner
                .as_deref()
                .map(|o| format!("({o})"))
                .unwrap_or_default();
            let blocked = if task.blocked_by.is_empty() {
                String::new()
            } else {
                format!("[blocked by {}]", task.blocked_by.join(", "))
            };
            lines.push(format!(
                "{} [{}] {}{}{}",
                task.id,
                state_str(task.state),
                task.subject,
                owner,
                blocked
            ));
        }

        Ok(text_chunk(
            self.name(),
            lines.join("\n"),
            ToolResultState::Success,
        ))
    }
}

// ---------------------------------------------------------------------------
// TaskGet
// ---------------------------------------------------------------------------

/// Retrieve a task by its ID from the task list.
pub struct TaskGetTool {
    state: Arc<RwLock<AgentState>>,
}

impl TaskGetTool {
    /// Create a new tool bound to the given shared agent state.
    pub fn new(state: Arc<RwLock<AgentState>>) -> Self {
        Self { state }
    }
}

#[derive(Deserialize)]
struct TaskGetParams {
    task_id: String,
}

#[async_trait::async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "TaskGet"
    }

    fn description(&self) -> &str {
        "Use this tool to retrieve a task by its ID from the task list.\n\n## When to Use This Tool\n\n- When you need the full description and context before starting work on a task\n- To understand task dependencies (what it blocks, what blocks it)\n- After being assigned a task, to get complete requirements\n\n## Output\n\nReturns full task details:\n- **subject**: Task title\n- **description**: Detailed requirements and context\n- **status**: 'pending', 'in_progress', or 'completed'\n- **blocks**: Tasks waiting on this one to complete\n- **blockedBy**: Tasks that must complete before this one can start\n\n## Tips\n\n- After fetching a task, verify its blockedBy list is empty before beginning work.\n- Use TaskList to see all tasks in summary form."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "The ID of the task to retrieve" }
            },
            "required": ["task_id"]
        })
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let params: TaskGetParams =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool_name: self.name().into(),
                reason: e.to_string(),
            })?;

        let state = self.state.read().unwrap();
        let Some(task) = state.tasks_context.get_task(&params.task_id) else {
            return Ok(text_chunk(
                self.name(),
                "Task not found".into(),
                ToolResultState::Error,
            ));
        };

        let mut lines = vec![
            format!("Task (id={}): {}", task.id, task.subject),
            format!("Status: {}", state_str(task.state)),
            format!("Description: {}", task.description),
        ];
        if let Some(owner) = &task.owner {
            lines.push(format!("Owner: {owner}"));
        }
        if !task.blocked_by.is_empty() {
            let blocked_by = task
                .blocked_by
                .iter()
                .map(|i| format!("#{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("Blocked by: {blocked_by}"));
        }
        if !task.blocks.is_empty() {
            let blocks = task
                .blocks
                .iter()
                .map(|i| format!("#{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("Blocks: {blocks}"));
        }
        if !task.metadata.is_empty() {
            lines.push(format!("Metadata: {}", py_dict_repr(&task.metadata)));
        }

        Ok(text_chunk(
            self.name(),
            lines.join("\n"),
            ToolResultState::Success,
        ))
    }
}

// ---------------------------------------------------------------------------
// TaskUpdate
// ---------------------------------------------------------------------------

/// Status values accepted by the TaskUpdate tool. `Deleted` removes the task
/// permanently (it never enters `TaskState`, matching the Python behavior).
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum TaskUpdateStatusInput {
    Pending,
    InProgress,
    Completed,
    Deleted,
}

#[derive(Deserialize)]
struct TaskUpdateParams {
    task_id: String,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    add_blocks: Option<Vec<String>>,
    #[serde(default)]
    status: Option<TaskUpdateStatusInput>,
    #[serde(default)]
    add_blocked_by: Option<Vec<String>>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    metadata: Option<serde_json::Map<String, JsonValue>>,
}

/// Update a task in the task list.
pub struct TaskUpdateTool {
    state: Arc<RwLock<AgentState>>,
}

impl TaskUpdateTool {
    /// Create a new tool bound to the given shared agent state.
    pub fn new(state: Arc<RwLock<AgentState>>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "TaskUpdate"
    }

    fn description(&self) -> &str {
        "Use this tool to update a task in the task list.\n\n## When to Use This Tool\n\n**Mark tasks as resolved:**\n- When you have completed the work described in a task\n- When a task is no longer needed or has been superseded\n- IMPORTANT: Always mark your assigned tasks as resolved when you finish them\n- After resolving, call TaskList to find your next task\n\n- ONLY mark a task as completed when you have FULLY accomplished it\n- If you encounter errors, blockers, or cannot finish, keep the task as in_progress\n- When blocked, create a new task describing what needs to be resolved\n- Never mark a task as completed if:\n  - Tests are failing\n  - Implementation is partial\n  - You encountered unresolved errors\n  - You couldn't find necessary files or dependencies\n\n**Delete tasks:**\n- When a task is no longer relevant or was created in error\n- Setting status to `deleted` permanently removes the task\n\n**Update task details:**\n- When requirements change or become clearer\n- When establishing dependencies between tasks\n\n## Fields You Can Update\n\n- **status**: The task status (see Status Workflow below)\n- **subject**: Change the task title (imperative form, e.g., \"Run tests\")\n- **description**: Change the task description\n- **owner**: Change the task owner (agent name)\n- **metadata**: Merge metadata keys into the task (set a key to null to delete it)\n- **add_blocks**: Mark tasks that cannot start until this one completes\n- **add_blocked_by**: Mark tasks that must complete before this one can start\n\n## Status Workflow\n\nStatus progresses: `pending` → `in_progress` → `completed`\n\nUse `deleted` to permanently remove a task.\n\n## Staleness\n\nMake sure to read a task's latest state using `TaskGet` before updating it.\n\n## Examples\n\nMark task as in progress when starting work:\n```json\n{\"task_id\": \"1\", \"status\": \"in_progress\"}\n```\n\nMark task as completed after finishing work:\n```json\n{\"task_id\": \"1\", \"status\": \"completed\"}\n```\n\nDelete a task:\n```json\n{\"task_id\": \"1\", \"status\": \"deleted\"}\n```\n\nClaim a task by setting owner:\n```json\n{\"task_id\": \"1\", \"owner\": \"my-name\"}\n```\n\nSet up task dependencies:\n```json\n{\"task_id\": \"2\", \"add_blocked_by\": [\"1\"]}\n```"
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "The task id." },
                "subject": { "type": ["string", "null"], "description": "New subject for the task" },
                "description": { "type": ["string", "null"], "description": "New description for the task" },
                "add_blocks": { "type": ["array", "null"], "items": { "type": "string" }, "description": "Task IDs that this task blocks" },
                "status": { "type": ["string", "null"], "enum": ["pending", "in_progress", "completed", "deleted"], "description": "New status for the task" },
                "add_blocked_by": { "type": ["array", "null"], "items": { "type": "string" }, "description": "Task IDs that block this task" },
                "owner": { "type": ["string", "null"], "description": "New owner for the task" },
                "metadata": { "type": ["object", "null"], "description": "Metadata keys to merge into the task. Set a key to null to delete it." }
            },
            "required": ["task_id"]
        })
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let params: TaskUpdateParams =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool_name: self.name().into(),
                reason: e.to_string(),
            })?;

        let mut state = self.state.write().unwrap();

        let Some(index) = state
            .tasks_context
            .tasks
            .iter()
            .position(|t| t.id == params.task_id)
        else {
            return Ok(text_chunk(
                self.name(),
                format!(
                    "TaskNotFoundError: The task (id={}) does not exist.",
                    params.task_id
                ),
                ToolResultState::Error,
            ));
        };

        let mut updated_fields: Vec<&'static str> = Vec::new();

        // subject — empty string counts as "not provided" (Python truthiness)
        if let Some(subject) = &params.subject
            && !subject.is_empty()
        {
            state.tasks_context.tasks[index].subject = subject.clone();
            updated_fields.push("subject");
        }

        // description — provided (even empty) updates
        if let Some(description) = &params.description {
            state.tasks_context.tasks[index].description = description.clone();
            updated_fields.push("description");
        }

        // add_blocks — only non-empty lists are processed; references to
        // non-existent ids are ignored
        if let Some(new_blocks) = &params.add_blocks
            && !new_blocks.is_empty()
        {
            let existed: Vec<String> = state
                .tasks_context
                .tasks
                .iter()
                .map(|t| t.id.clone())
                .collect();
            let current = state.tasks_context.tasks[index].blocks.clone();
            let mut added_any = false;
            for block_id in new_blocks {
                // Guard against a task blocking itself (a self-cycle would make
                // it permanently blocked).
                if block_id != &params.task_id
                    && !current.contains(block_id)
                    && existed.contains(block_id)
                {
                    state
                        .tasks_context
                        .update_block_relation(&params.task_id, block_id);
                    added_any = true;
                }
            }
            if added_any {
                updated_fields.push("add_blocks");
            }
        }

        // add_blocked_by — processed even when empty (Python `is not None`)
        if let Some(new_blocked_by) = &params.add_blocked_by {
            let existed: Vec<String> = state
                .tasks_context
                .tasks
                .iter()
                .map(|t| t.id.clone())
                .collect();
            let current = state.tasks_context.tasks[index].blocked_by.clone();
            let mut added_any = false;
            for blocked_by_id in new_blocked_by {
                // Guard against a task blocking itself.
                if blocked_by_id != &params.task_id
                    && !current.contains(blocked_by_id)
                    && existed.contains(blocked_by_id)
                {
                    state
                        .tasks_context
                        .update_block_relation(blocked_by_id, &params.task_id);
                    added_any = true;
                }
            }
            if added_any {
                updated_fields.push("add_blocked_by");
            }
        }

        // status — deleted returns immediately (no further field processing)
        if let Some(status) = &params.status {
            match status {
                TaskUpdateStatusInput::Deleted => {
                    state.tasks_context.delete_task(&params.task_id);
                    return Ok(text_chunk(
                        self.name(),
                        format!("Task (id={}) has been deleted.", params.task_id),
                        ToolResultState::Success,
                    ));
                }
                TaskUpdateStatusInput::Pending => {
                    state.tasks_context.tasks[index].state = TaskState::Pending;
                }
                TaskUpdateStatusInput::InProgress => {
                    state.tasks_context.tasks[index].state = TaskState::InProgress;
                }
                TaskUpdateStatusInput::Completed => {
                    state.tasks_context.tasks[index].state = TaskState::Completed;
                }
            }
            updated_fields.push("status");
        }

        // owner
        if let Some(owner) = &params.owner {
            state.tasks_context.tasks[index].owner = Some(owner.clone());
            updated_fields.push("owner");
        }

        // metadata — merge; a null value deletes the key; only non-empty maps
        // are processed (Python truthiness)
        if let Some(meta) = &params.metadata
            && !meta.is_empty()
        {
            let task_meta = &mut state.tasks_context.tasks[index].metadata;
            for (k, v) in meta {
                if v.is_null() {
                    task_meta.remove(k);
                } else {
                    task_meta.insert(k.clone(), v.clone());
                }
            }
            updated_fields.push("metadata");
        }

        if updated_fields.is_empty() {
            return Ok(text_chunk(
                self.name(),
                format!(
                    "No updates were made to the task (id={}). Make sure you provided at least one field to update and the values are correct.",
                    params.task_id
                ),
                ToolResultState::Success,
            ));
        }

        let mut res = format!(
            "Update task (id={}) {}.",
            params.task_id,
            updated_fields.join(", ")
        );
        if state.tasks_context.tasks[index].state == TaskState::Completed {
            res += "\n\nTask completed. Call TaskList now to find your next available task or see if your work unblocked others.";
        }

        Ok(text_chunk(self.name(), res, ToolResultState::Success))
    }
}
