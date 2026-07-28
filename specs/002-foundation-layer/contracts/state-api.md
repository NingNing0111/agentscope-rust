# State Module API Contract

**Module**: `agent_scope::state` | **Dependencies**: `agent_scope::message`, `agent_scope::types`

## Public Types

### Structs

| Struct | 说明 | Serde |
|--------|------|-------|
| `AgentState` | Agent 完整运行时状态 | 普通 struct |
| `ReplyContext` | 当前回复的上下文 | 普通 struct |
| `ToolContext` | 工具上下文（含 LRU 缓存） | 普通 struct |
| `ReadCacheEntry` | 文件读缓存条目 | 普通 struct |
| `TaskContext` | 任务上下文 | 普通 struct |
| `Task` | 单个任务 | 普通 struct |

### Placeholders (待 permission 模块替换)

```rust
pub type PermissionContext = HashMap<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    #[serde(flatten)]
    pub extras: HashMap<String, serde_json::Value>,
}
```

### TaskState Enum

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    InProgress,
    Completed,
}
```

## AgentState API

```rust
impl AgentState {
    /// 创建新的 AgentState
    pub fn new() -> Self

    /// 创建带自定义 session_id 的 AgentState
    pub fn with_session_id(session_id: String) -> Self

    /// 在 context 尾部追加内容块。若尾部 assistant 消息匹配 name+reply_id 则追加；
    /// 否则创建新的 assistant 消息。若达到 max_context_messages 上限则返回 Err。
    pub fn append_context(
        &mut self,
        name: &str,
        blocks: Vec<ContentBlock>,
    ) -> Result<(), AppendContextError>

    /// 检查是否有等待外部输入的 ToolCall
    pub fn has_awaiting_tool_calls(&self, name: &str) -> bool

    /// 获取等待外部输入的 ToolCall 列表
    pub fn get_awaiting_tool_calls(&self, name: &str) -> Vec<&ToolCallBlock>

    /// 设置 context 消息上限（None 表示无限制）
    pub fn set_max_context_messages(&mut self, max: Option<usize>)

    /// 获取当前 context 中的消息数量
    pub fn context_length(&self) -> usize
}

/// 旧格式迁移
impl AgentState {
    /// 从旧格式 JSON（顶层 reply_id/cur_iter）反序列化，自动迁移到新格式
    pub fn from_legacy_json(json: &str) -> Result<Self, serde_json::Error>
}
```

### Error Types

```rust
pub enum AppendContextError {
    ContextFull {
        max_messages: usize,
        current_count: usize,
    },
}
```

## ToolContext API

```rust
impl ToolContext {
    /// 检查文件缓存是否有效（基于 mtime）
    pub fn get_cache(&self, file_path: &str) -> Option<&ReadCacheEntry>

    /// 缓存文件内容，按 LRU 策略驱逐旧条目
    pub fn cache_file(&mut self, file_path: &str, lines: Vec<String>)

    /// 清理不在保留列表中的缓存
    pub fn clean_file_cache(&mut self, reserved_file_paths: &HashSet<String>)
}
```

注：Python 实现中 `get_cache`/`cache_file` 是 async 方法（需要 `aiofiles.os.path.getmtime`）。Rust 实现提供两种方案：
- 同步版本（使用 `std::fs::metadata`）——适合不涉及网络 I/O 的场景
- 异步版本（使用 `tokio::fs::metadata`）——适合在异步上下文中使用

## TaskContext API

```rust
impl TaskContext {
    pub fn new() -> Self
    pub fn add_task(&mut self, task: Task)
    pub fn get_task(&self, id: &str) -> Option<&Task>
    pub fn update_task_state(&mut self, id: &str, state: TaskState) -> Result<(), TaskError>
    pub fn tasks_by_state(&self, state: TaskState) -> Vec<&Task>
    pub fn tasks_by_owner(&self, owner: &str) -> Vec<&Task>
}

impl Task {
    pub fn new(subject: String, description: String, metadata: HashMap<String, Value>) -> Self
}
```

## JSON Serialization Contract

### AgentState 示例（简化版）

```json
{
  "session_id": "<uuid>",
  "summary": "",
  "context": [],
  "max_context_messages": null,
  "reply_context": {
    "reply_id": "<uuid>",
    "cur_iter": 0,
    "structured_schema": null,
    "structured_output": null
  },
  "permission_context": {},
  "tool_context": {
    "max_cache_files": 100,
    "max_cache_bytes": 25000.0,
    "read_file_cache": [],
    "activated_groups": []
  },
  "tasks_context": {
    "tasks": []
  },
  "middle_context": {}
}
```

### Task 示例

```json
{
  "subject": "Implement login",
  "description": "Add user authentication flow",
  "metadata": {"priority": "high"},
  "created_at": "2026-07-28T10:00:00Z",
  "state": "in_progress",
  "id": "<uuid>",
  "owner": "alice",
  "blocks": [],
  "blocked_by": ["<dependency-task-id>"]
}
```

### 旧格式迁移

旧格式（顶层字段）:
```json
{
  "reply_id": "<reply-uuid>",
  "cur_iter": 3,
  "reply_context": {},
  ...
}
```

反序列化时自动合并为:
```json
{
  "reply_context": {
    "reply_id": "<reply-uuid>",
    "cur_iter": 3,
    ...
  },
  ...
}
```
