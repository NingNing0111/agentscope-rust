# Data Model: AgentScope Foundation Layer

**Feature**: 002-foundation-layer | **Date**: 2026-07-28

## Entity Overview

```text
types (0 deps)
  ├── ReplyFinishedReason (enum)
  ├── ErrorType (enum)
  ├── ErrorInfo (struct)
  ├── JSONPrimitive (type alias)
  ├── JSONSerializableObject (type alias)
  ├── Embedding (type alias)
  ├── AgentHookTypes (type alias)
  └── ReActAgentHookTypes (type alias)

message (depends on: types)
  ├── TextBlock (struct)
  ├── ThinkingBlock (struct)
  ├── HintBlock (struct)
  ├── Base64Source (struct)
  ├── URLSource (struct)
  ├── DataBlock (struct)
  ├── ToolCallState (enum)
  ├── ToolCallBlock (struct)
  ├── ToolResultState (enum)
  ├── ToolResultBlock (struct)
  ├── ContentBlock (enum — tagged union)
  ├── ContentBlockTypes (enum/type alias)
  ├── Usage (struct)
  ├── Msg (struct)
  └── Factory fns: UserMsg, AssistantMsg, SystemMsg

event (depends on: message, types)
  ├── EventType (enum — 28 variants)
  ├── EventBase (struct — base fields)
  ├── ReplyStartEvent, ReplyEndEvent
  ├── ModelCallStartEvent, ModelCallEndEvent
  ├── TextBlockStart/Delta/EndEvent
  ├── DataBlockStart/Delta/EndEvent
  ├── ThinkingBlockStart/Delta/EndEvent
  ├── HintBlockEvent
  ├── ToolCallStart/Delta/EndEvent
  ├── ToolResultStart/TextDelta/DataDelta/EndEvent
  ├── ExceedMaxItersEvent
  ├── RequireUserConfirmEvent
  ├── UserConfirmResultEvent (dep: ConfirmResult)
  ├── UserInterruptEvent
  ├── RequireExternalExecutionEvent
  ├── ExternalExecutionResultEvent
  ├── CustomEvent
  └── AgentEvent (enum — tagged union, 27 variants)

state (depends on: message, types)
  ├── ReadCacheEntry (struct)
  ├── ToolContext (struct)
  ├── Task (struct)
  ├── TaskContext (struct)
  ├── ReplyContext (struct)
  ├── PermissionContext (placeholder: type alias = HashMap<String, serde_json::Value>)
  ├── PermissionRule (placeholder: minimal struct)
  └── AgentState (struct)
```

---

## Types 模块

### ReplyFinishedReason

| 成员 | JSON 值 |
|------|---------|
| `COMPLETED` | `"completed"` |
| `INTERRUPTED` | `"interrupted"` |
| `EXCEED_MAX_ITERS` | `"exceed_max_iters"` |
| `ERROR` | `"error"` |

**Serde**: `#[serde(rename_all = "snake_case")]`

### ErrorType

| 成员 | JSON 值 | HTTP 语义 |
|------|---------|----------|
| `AUTHENTICATION` | `"authentication"` | 401 |
| `PERMISSION` | `"permission"` | 403 |
| `RATE_LIMIT` | `"rate_limit"` | 429 |
| `INVALID_REQUEST` | `"invalid_request"` | 400/422 |
| `UPSTREAM` | `"upstream"` | 5xx |
| `CONNECTION` | `"connection"` | 网络错误 |
| `INTERNAL` | `"internal"` | 框架内部错误 |
| `UNKNOWN` | `"unknown"` | 兜底 |

**Serde**: `#[serde(rename_all = "snake_case")]`

### ErrorInfo

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `type` | `ErrorType` | 是 | `ErrorType::UNKNOWN` | 稳定分类键 |
| `message` | `String` | 是 | — | 人类可读描述 |

### JSONPrimitive / JSONSerializableObject

Rust 等效：
- `JSONPrimitive` → 不需要 type alias（Rust 无 recursive type alias for union）
- `JSONSerializableObject` → `serde_json::Value`

替代方案：定义递归 enum `JsonValue`，但 `serde_json::Value` 已被社区广泛使用且满足需求。

### Embedding

```rust
type Embedding = Vec<f64>;
```

### AgentHookTypes

6 个 hook 点字面量：`"pre_reply"`, `"post_reply"`, `"pre_print"`, `"post_print"`, `"pre_observe"`, `"post_observe"`

### ReActAgentHookTypes

在 AgentHookTypes 基础上增加 4 个：`"pre_reasoning"`, `"post_reasoning"`, `"pre_acting"`, `"post_acting"`

---

## Message 模块

### ContentBlock 联合枚举

使用 serde internally tagged enum：

```text
ContentBlock (enum)
├── Text(TextBlock)           ← tag: "text"
├── Thinking(ThinkingBlock)   ← tag: "thinking"
├── Hint(HintBlock)           ← tag: "hint"
├── Data(DataBlock)           ← tag: "data"
├── ToolCall(ToolCallBlock)   ← tag: "tool_call"
└── ToolResult(ToolResultBlock) ← tag: "tool_result"
```

### TextBlock

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `text` | `String` | 是 | — | `"text"` |
| `id` | `String` | 是 | UUID hex (自动生成) | `"id"` |
| `created_at` | `String` (ISO 8601) | 是 | 当前时间 | `"created_at"` |
| `finished_at` | `Option<String>` | 否 | `None` | `"finished_at"` |

注：`type` 字段由 serde tag 自动注入，JSON 中值为 `"text"`。

### ThinkingBlock

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `thinking` | `String` | 是 | — | `"thinking"` |
| `id` | `String` | 是 | UUID hex | `"id"` |
| `created_at` | `String` | 是 | 当前时间 | `"created_at"` |
| `finished_at` | `Option<String>` | 否 | `None` | `"finished_at"` |
| extras | `HashMap<String, Value>` | 否 | `{}` | 任意透传键 |

注：`#[serde(flatten)] extras` 捕获 provider 特定字段（如 Anthropic 的 `signature`、`redacted_thinking_data`）。type 标签值为 `"thinking"`。

### HintBlock

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `hint` | `HintContent` (enum) | 是 | — | `"hint"` |
| `source` | `Option<String>` | 否 | `None` | `"source"` |
| `id` | `String` | 是 | UUID hex | `"id"` |
| `created_at` | `String` | 是 | 当前时间 | `"created_at"` |
| `finished_at` | `Option<String>` | 否 | 当前时间 | `"finished_at"` |

`HintContent` 子枚举（untagged）：
- `Text(String)` → JSON: `"hello"`
- `Blocks(Vec<HintBlockItem>)` → JSON: `[{"type": "text", "text": "..."}, ...]`

### Base64Source

| 字段 | 类型 | 必填 | JSON Key |
|------|------|------|----------|
| `data` | `String` (base64) | 是 | `"data"` |
| `media_type` | `String` | 是 | `"media_type"` |

注：type 标签值为 `"base64"`。

### URLSource

| 字段 | 类型 | 必填 | JSON Key |
|------|------|------|----------|
| `url` | `String` (RFC 3986 URI) | 是 | `"url"` |
| `media_type` | `String` | 是 | `"media_type"` |

注：type 标签值为 `"url"`。序列化时 `url` 字段为字符串（`#[serde(serialize_with = "...")]` 或使用 `String` 类型）。

### DataBlock

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `source` | `DataSource` (enum) | 是 | — | `"source"` |
| `name` | `Option<String>` | 否 | `None` | `"name"` |
| `id` | `String` | 是 | UUID hex | `"id"` |
| `created_at` | `String` | 是 | 当前时间 | `"created_at"` |
| `finished_at` | `Option<String>` | 否 | `None` | `"finished_at"` |

`DataSource` 子枚举（tagged on `"type"`）：
- `Base64(Base64Source)` — tag: `"base64"`
- `Url(URLSource)` — tag: `"url"`

### ToolCallState

| 成员 | JSON 值 | 说明 |
|------|---------|------|
| `PENDING` | `"pending"` | 初始状态 |
| `ASKING` | `"asking"` | 等待用户确认 |
| `ALLOWED` | `"allowed"` | 允许执行 |
| `SUBMITTED` | `"submitted"` | 已提交外部执行 |
| `FINISHED` | `"finished"` | 已完成 |

**状态转换**:
```
PENDING → ASKING → ALLOWED → SUBMITTED → FINISHED
PENDING ────────────────────────────────→ FINISHED (permission deny)
ASKING ─────────────────────────────────→ FINISHED (user denied)
ALLOWED ────────────────────────────────→ FINISHED (local tool done)
```

### ToolCallBlock

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `id` | `String` | 是 | — | `"id"` |
| `name` | `String` | 是 | — | `"name"` |
| `input` | `String` (raw JSON) | 是 | — | `"input"` |
| `state` | `ToolCallState` | 是 | `PENDING` | `"state"` |
| `suggested_rules` | `Vec<PermissionRule>` | 是 | `[]` | `"suggested_rules"` |
| `created_at` | `String` | 是 | 当前时间 | `"created_at"` |
| `finished_at` | `Option<String>` | 否 | `None` | `"finished_at"` |

注：`state` 序列化为字符串值（`use_enum_values=True` 等效）。type 标签为 `"tool_call"`。

### ToolResultState

| 成员 | JSON 值 | 说明 |
|------|---------|------|
| `SUCCESS` | `"success"` | 执行成功 |
| `ERROR` | `"error"` | 执行失败 |
| `INTERRUPTED` | `"interrupted"` | 被中断 |
| `DENIED` | `"denied"` | 被拒绝 |
| `RUNNING` | `"running"` | 执行中 |

### ToolResultBlock

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `id` | `String` | 是 | — | `"id"` |
| `name` | `String` | 是 | — | `"name"` |
| `output` | `ToolOutput` (enum) | 是 | — | `"output"` |
| `state` | `ToolResultState` | 是 | `RUNNING` | `"state"` |
| `metadata` | `HashMap<String, Value>` | 是 | `{}` | `"metadata"` |
| `created_at` | `String` | 是 | 当前时间 | `"created_at"` |
| `finished_at` | `Option<String>` | 否 | `None` | `"finished_at"` |

`ToolOutput` 子枚举（untagged）：
- `Text(String)` → JSON: `"some text"`
- `Blocks(Vec<ToolResultBlockItem>)` → JSON: `[{"type": "text", "text": "..."}, ...]`

注：type 标签为 `"tool_result"`。

### Usage

| 字段 | 类型 | 必填 | JSON Key |
|------|------|------|----------|
| `input_tokens` | `i64` | 是 | `"input_tokens"` |
| `output_tokens` | `i64` | 是 | `"output_tokens"` |

注：使用 `i64` 而非 `u64` 以匹配 Python `int` 类型（无符号不匹配），同时避免 JSON 序列化溢出问题。

### Msg

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `name` | `String` | 是 | — | `"name"` |
| `content` | `Vec<ContentBlock>` | 是 | — | `"content"` |
| `role` | `Role` (enum) | 是 | — | `"role"` |
| `id` | `String` | 是 | UUID hex | `"id"` |
| `metadata` | `HashMap<String, Value>` | 是 | `{}` | `"metadata"` |
| `created_at` | `String` | 是 | 当前时间 | `"created_at"` |
| `usage` | `Option<Usage>` | 否 | `None` | `"usage"` |
| `finished_at` | `Option<String>` | 否 | `None` | `"finished_at"` |
| `finished_reason` | `Option<ReplyFinishedReason>` | 否 | `None` | `"finished_reason"` |
| `structured_output` | `Option<Value>` | 否 | `None` | `"structured_output"` |
| `error` | `Option<ErrorInfo>` | 否 | `None` | `"error"` |

**Role** 枚举: `#[serde(rename_all = "lowercase")]` → `User`, `Assistant`, `System`

**验证规则**:
| Role | 允许的 ContentBlock 类型 |
|------|--------------------------|
| `User` | `Text`, `Data` |
| `System` | `Text` |
| `Assistant` | 所有类型 |

**工厂函数**:
- `UserMsg(name, content, ...)` → `Role::User`, finished_at 默认等于 created_at
- `AssistantMsg(name, content, ...)` → `Role::Assistant`, finished_at 默认 None
- `SystemMsg(name, content, ...)` → `Role::System`, finished_at 默认等于 created_at

---

## Event 模块

### EventType (28 variants)

| Variant | JSON 字符串 |
|---------|------------|
| `REPLY_START` | `"REPLY_START"` |
| `REPLY_END` | `"REPLY_END"` |
| `MODEL_CALL_START` | `"MODEL_CALL_START"` |
| `MODEL_CALL_END` | `"MODEL_CALL_END"` |
| `TEXT_BLOCK_START` | `"TEXT_BLOCK_START"` |
| `TEXT_BLOCK_DELTA` | `"TEXT_BLOCK_DELTA"` |
| `TEXT_BLOCK_END` | `"TEXT_BLOCK_END"` |
| `DATA_BLOCK_START` | `"DATA_BLOCK_START"` |
| `DATA_BLOCK_DELTA` | `"DATA_BLOCK_DELTA"` |
| `DATA_BLOCK_END` | `"DATA_BLOCK_END"` |
| `THINKING_BLOCK_START` | `"THINKING_BLOCK_START"` |
| `THINKING_BLOCK_DELTA` | `"THINKING_BLOCK_DELTA"` |
| `THINKING_BLOCK_END` | `"THINKING_BLOCK_END"` |
| `HINT_BLOCK` | `"HINT_BLOCK"` |
| `TOOL_CALL_START` | `"TOOL_CALL_START"` |
| `TOOL_CALL_DELTA` | `"TOOL_CALL_DELTA"` |
| `TOOL_CALL_END` | `"TOOL_CALL_END"` |
| `TOOL_RESULT_START` | `"TOOL_RESULT_START"` |
| `TOOL_RESULT_TEXT_DELTA` | `"TOOL_RESULT_TEXT_DELTA"` |
| `TOOL_RESULT_DATA_DELTA` | `"TOOL_RESULT_DATA_DELTA"` |
| `TOOL_RESULT_END` | `"TOOL_RESULT_END"` |
| `EXCEED_MAX_ITERS` | `"EXCEED_MAX_ITERS"` |
| `REQUIRE_USER_CONFIRM` | `"REQUIRE_USER_CONFIRM"` |
| `USER_CONFIRM_RESULT` | `"USER_CONFIRM_RESULT"` |
| `USER_INTERRUPT` | `"USER_INTERRUPT"` |
| `REQUIRE_EXTERNAL_EXECUTION` | `"REQUIRE_EXTERNAL_EXECUTION"` |
| `EXTERNAL_EXECUTION_RESULT` | `"EXTERNAL_EXECUTION_RESULT"` |
| `CUSTOM` | `"CUSTOM"` |

### EventBase

所有事件类的共享基字段：

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `id` | `String` | 是 | UUID hex | `"id"` |
| `created_at` | `String` (ISO 8601) | 是 | 当前时间 | `"created_at"` |
| `metadata` | `HashMap<String, Value>` | 是 | `{}` | `"metadata"` |

### 各类事件字段摘要

| 事件类 | 特有字段 |
|--------|---------|
| `ReplyStartEvent` | `session_id: String`, `reply_id: String`, `name: String`, `role: String` |
| `ReplyEndEvent` | `session_id: String`, `reply_id: String`, `finished_reason: ReplyFinishedReason`, `error: Option<ErrorInfo>` |
| `ModelCallStartEvent` | `reply_id: String`, `model_name: String` |
| `ModelCallEndEvent` | `reply_id: String`, `input_tokens: i64`, `output_tokens: i64`, `finished_reason: FinishedReason` |
| `TextBlockStartEvent` | `reply_id: String`, `block_id: String` |
| `TextBlockDeltaEvent` | `reply_id: String`, `block_id: String`, `delta: String` |
| `TextBlockEndEvent` | `reply_id: String`, `block_id: String` |
| `DataBlockStartEvent` | `reply_id: String`, `block_id: String`, `media_type: String` |
| `DataBlockDeltaEvent` | `reply_id: String`, `block_id: String`, `data: String`, `media_type: String` |
| `DataBlockEndEvent` | `reply_id: String`, `block_id: String` |
| `ThinkingBlockStartEvent` | `reply_id: String`, `block_id: String` |
| `ThinkingBlockDeltaEvent` | `reply_id: String`, `block_id: String`, `delta: String` |
| `ThinkingBlockEndEvent` | `reply_id: String`, `block_id: String` |
| `HintBlockEvent` | `reply_id: String`, `block_id: String`, `source: Option<String>`, `hint: HintContent` |
| `ToolCallStartEvent` | `reply_id: String`, `tool_call_id: String`, `tool_call_name: String` |
| `ToolCallDeltaEvent` | `reply_id: String`, `tool_call_id: String`, `delta: String` |
| `ToolCallEndEvent` | `reply_id: String`, `tool_call_id: String` |
| `ToolResultStartEvent` | `reply_id: String`, `tool_call_id: String`, `tool_call_name: String` |
| `ToolResultTextDeltaEvent` | `reply_id: String`, `tool_call_id: String`, `delta: String` |
| `ToolResultDataDeltaEvent` | `reply_id: String`, `tool_call_id: String`, `block_id: String`, `media_type: String`, `data: Option<String>`, `url: Option<String>` |
| `ToolResultEndEvent` | `reply_id: String`, `tool_call_id: String`, `state: ToolResultState`, `metadata: HashMap<String, Value>` |
| `ExceedMaxItersEvent` | `reply_id: String`, `name: String` |
| `RequireUserConfirmEvent` | `reply_id: String`, `tool_calls: Vec<ToolCallBlock>` |
| `UserConfirmResultEvent` | `reply_id: String`, `confirm_results: Vec<ConfirmResult>` |
| `UserInterruptEvent` | `reply_id: String` |
| `RequireExternalExecutionEvent` | `reply_id: String`, `tool_calls: Vec<ToolCallBlock>` |
| `ExternalExecutionResultEvent` | `reply_id: String`, `execution_results: Vec<ToolResultBlock>` |
| `CustomEvent` | `name: String`, `value: HashMap<String, Value>` |

### ConfirmResult

| 字段 | 类型 | JSON Key |
|------|------|----------|
| `confirmed` | `bool` | `"confirmed"` |
| `tool_call` | `ToolCallBlock` | `"tool_call"` |
| `rules` | `Option<Vec<PermissionRule>>` | `"rules"` |

### AgentEvent 联合枚举

Internally tagged enum（tag = `"type"`），使用 EventType 的字符串值作为判别，共 27 个 variants（加上 `Custom`）。

---

## State 模块

### AgentState

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `session_id` | `String` | 是 | UUID hex | `"session_id"` |
| `summary` | `SummaryContent` (enum) | 是 | `""` | `"summary"` |
| `context` | `Vec<Msg>` | 是 | `[]` | `"context"` |
| `max_context_messages` | `Option<usize>` | 是 | `None` | `"max_context_messages"` |
| `reply_context` | `ReplyContext` | 是 | — | `"reply_context"` |
| `permission_context` | `PermissionContext` | 是 | — | `"permission_context"` |
| `tool_context` | `ToolContext` | 是 | — | `"tool_context"` |
| `tasks_context` | `TaskContext` | 是 | — | `"tasks_context"` |
| `middle_context` | `HashMap<String, Value>` | 是 | `{}` | `"middle_context"` |

**方法**:
- `append_context(name, blocks)` → `Result<(), AppendError>` — 若 `max_context_messages` 有值且已达限，返回 `AppendError::ContextFull`
- `has_awaiting_tool_calls(name)` → `bool`
- `get_awaiting_tool_calls(name)` → `Vec<&ToolCallBlock>`

### ReplyContext

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `reply_id` | `String` | 是 | UUID hex | `"reply_id"` |
| `cur_iter` | `i32` | 是 | `0` | `"cur_iter"` |
| `structured_schema` | `Option<Value>` (JSON Schema dict) | 否 | `None` | `"structured_schema"` |
| `structured_output` | `Option<Value>` | 否 | `None` | `"structured_output"` |

### ToolContext

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `max_cache_files` | `usize` | 是 | `100` | `"max_cache_files"` |
| `max_cache_bytes` | `f64` | 是 | `25000.0` | `"max_cache_bytes"` |
| `read_file_cache` | `Vec<ReadCacheEntry>` | 是 | `[]` | `"read_file_cache"` |
| `activated_groups` | `Vec<String>` | 是 | `[]` | `"activated_groups"` |

注：Python `ToolContext` 中的 `async` 方法（`get_cache`、`cache_file`、`clean_file_cache`）在 Rust 中使用同步实现——文件系统的 `mtime` 检查在 Rust 中可同步完成，但如果需要异步则通过 `tokio::fs` 实现。

### ReadCacheEntry

| 字段 | 类型 | JSON Key |
|------|------|----------|
| `lines` | `Vec<String>` | `"lines"` |
| `updated_at` | `f64` (Unix timestamp) | `"updated_at"` |
| `bytes` | `f64` | `"bytes"` |
| `file_path` | `String` | `"file_path"` |

### TaskContext

| 字段 | 类型 | JSON Key |
|------|------|----------|
| `tasks` | `Vec<Task>` | `"tasks"` |

### Task

| 字段 | 类型 | 必填 | 默认值 | JSON Key |
|------|------|------|--------|----------|
| `subject` | `String` | 是 | — | `"subject"` |
| `description` | `String` | 是 | — | `"description"` |
| `metadata` | `HashMap<String, Value>` | 是 | — | `"metadata"` |
| `created_at` | `String` (ISO 8601) | 是 | 当前时间 | `"created_at"` |
| `state` | `TaskState` (enum) | 是 | `Pending` | `"state"` |
| `id` | `String` | 是 | UUID hex | `"id"` |
| `owner` | `Option<String>` | 否 | `None` | `"owner"` |
| `blocks` | `Vec<String>` (task IDs) | 是 | `[]` | `"blocks"` |
| `blocked_by` | `Vec<String>` (task IDs) | 是 | `[]` | `"blocked_by"` |

**TaskState**: `Pending`, `InProgress`, `Completed` → JSON: `"pending"`, `"in_progress"`, `"completed"`

### PermissionContext / PermissionRule（占位类型）

```rust
// 占位类型 — 后续由 permission 模块的特性替换为完整定义
pub type PermissionContext = HashMap<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    #[serde(flatten)]
    pub extras: HashMap<String, serde_json::Value>,
}
```
