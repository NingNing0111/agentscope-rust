# Message Module API Contract

**Module**: `agent_scope::message` | **Dependencies**: `agent_scope::types`

## Public Types

### Enums

```rust
/// ContentBlock 判别类型字面量
pub enum BlockType {
    Text,
    Thinking,
    Hint,
    Data,
    ToolCall,
    ToolResult,
}

/// 工具调用生命周期状态
pub enum ToolCallState {
    Pending,
    Asking,
    Allowed,
    Submitted,
    Finished,
}

/// 工具执行结果状态
pub enum ToolResultState {
    Success,
    Error,
    Interrupted,
    Denied,
    Running,
}
```

### Structs

#### Data Structures & Serde

以下 struct 均实现 `Serialize`, `Deserialize`, `Debug`, `Clone`：

| Struct | 说明 | Serde Strategy |
|--------|------|---------------|
| `TextBlock` | 纯文本内容块 | Tag `"text"` |
| `ThinkingBlock` | 模型推理内容块 | Tag `"thinking"`, `#[serde(flatten)]` extras |
| `HintBlock` | 提示/指令块 | Tag `"hint"` |
| `DataBlock` | 二进制数据块 | Tag `"data"`, source 为 tagged enum |
| `Base64Source` | base64 数据源 | Tag `"base64"` |
| `URLSource` | URL 数据源 | Tag `"url"` |
| `ToolCallBlock` | 工具调用块 | Tag `"tool_call"` |
| `ToolResultBlock` | 工具结果块 | Tag `"tool_result"` |
| `Usage` | Token 用量 | 普通 struct |
| `Msg` | 消息主结构 | 普通 struct, role 验证 |
| `PermissionRule` | 权限规则占位 | `#[serde(flatten)]` extras |

#### ContentBlock (Tagged Enum)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text(TextBlock),
    #[serde(rename = "thinking")]
    Thinking(ThinkingBlock),
    #[serde(rename = "hint")]
    Hint(HintBlock),
    #[serde(rename = "data")]
    Data(DataBlock),
    #[serde(rename = "tool_call")]
    ToolCall(ToolCallBlock),
    #[serde(rename = "tool_result")]
    ToolResult(ToolResultBlock),
}
```

### Factory Functions

```rust
pub fn user_msg(name: &str, content: impl Into<UserContent>) -> Result<Msg, ValidationError>
pub fn assistant_msg(name: &str, content: impl Into<AssistantContent>) -> Msg
pub fn system_msg(name: &str, content: impl Into<SystemContent>) -> Result<Msg, ValidationError>
```

- `UserContent` = `String | Vec<TextBlock> | Vec<DataBlock> | Vec<ContentBlock>` (构造时验证只含 text/data)
- `AssistantContent` = `String | Vec<ContentBlock>` (无验证)
- `SystemContent` = `String | Vec<TextBlock>` (构造时验证只含 text)

### Msg Methods

```rust
impl Msg {
    /// 创建新的 Msg 并验证 role-content 合法性
    pub fn new(name: String, content: Vec<ContentBlock>, role: Role) -> Result<Self, ValidationError>

    /// 按类型过滤内容块
    pub fn get_content_blocks(&self, block_type: Option<BlockType>) -> Vec<&ContentBlock>

    /// 拼接所有 TextBlock 的文本
    pub fn get_text_content(&self, separator: &str) -> Option<String>

    /// 检查是否存在指定类型的内容块
    pub fn has_content_blocks(&self, block_type: Option<BlockType>) -> bool

    /// 应用事件，增量更新消息内容。返回 Ok(()) 或错误（不匹配时会记录警告但不会失败）
    pub fn append_event(&mut self, event: &AgentEvent) -> Result<(), AppendEventError>
}
```

### Error Types

```rust
pub enum ValidationError {
    InvalidContentForRole {
        role: Role,
        disallowed_types: Vec<BlockType>,
    },
    ContextFull {
        max_messages: usize,
        current_count: usize,
    },
}

pub enum AppendEventError {
    ReplyIdMismatch {
        event_reply_id: String,
        msg_id: String,
    },
    BlockNotFound {
        block_type: BlockType,
        block_id: String,
    },
    UnknownEventType(String),
}
```

## JSON Serialization Contracts

### Msg (Role: assistant) 示例

```json
{
  "name": "alice",
  "content": [
    {"type": "text", "text": "Hello!", "id": "<uuid>", "created_at": "<iso8601>", "finished_at": null},
    {"type": "tool_call", "id": "<uuid>", "name": "search", "input": "{\"q\":\"test\"}", "state": "finished", "suggested_rules": [], "created_at": "<iso8601>", "finished_at": "<iso8601>"},
    {"type": "tool_result", "id": "<uuid>", "name": "search", "output": "results found", "state": "success", "metadata": {}, "created_at": "<iso8601>", "finished_at": "<iso8601>"}
  ],
  "role": "assistant",
  "id": "<uuid>",
  "metadata": {},
  "created_at": "<iso8601>",
  "usage": {"input_tokens": 150, "output_tokens": 50},
  "finished_at": null,
  "finished_reason": null,
  "structured_output": null,
  "error": null
}
```

### Msg (Role: user) 示例

```json
{
  "name": "user",
  "content": [
    {"type": "text", "text": "What is the weather?", "id": "<uuid>", "created_at": "<iso8601>", "finished_at": null}
  ],
  "role": "user",
  "id": "<uuid>",
  "metadata": {},
  "created_at": "<iso8601>",
  "usage": null,
  "finished_at": null,
  "finished_reason": null,
  "structured_output": null,
  "error": null
}
```

### ContentBlock 类型对照

| Python Class | 标签值 | Rust Variant |
|--------------|--------|-------------|
| `TextBlock` | `"text"` | `ContentBlock::Text(...)` |
| `ThinkingBlock` | `"thinking"` | `ContentBlock::Thinking(...)` |
| `HintBlock` | `"hint"` | `ContentBlock::Hint(...)` |
| `DataBlock` | `"data"` | `ContentBlock::Data(...)` |
| `ToolCallBlock` | `"tool_call"` | `ContentBlock::ToolCall(...)` |
| `ToolResultBlock` | `"tool_result"` | `ContentBlock::ToolResult(...)` |

## Validation Rules

| 条件 | 行为 |
|------|------|
| Role == User, ContentBlock 非 Text/Data | → `Err(ValidationError)` |
| Role == System, ContentBlock 非 Text | → `Err(ValidationError)` |
| Role == Assistant | → 无限制 |
| 构造时 content 为空 | → 允许 (factory 函数自动处理默认值) |
| append_event 中 reply_id 不匹配 | → 记录 warning, 跳过事件, 返回 `Ok(())` |
