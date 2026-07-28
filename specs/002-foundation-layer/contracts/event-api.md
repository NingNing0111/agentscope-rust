# Event Module API Contract

**Module**: `agent_scope::event` | **Dependencies**: `agent_scope::message`, `agent_scope::types`

## Public Types

### EventType Enum

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    ReplyStart,
    ReplyEnd,
    ModelCallStart,
    ModelCallEnd,
    TextBlockStart,
    TextBlockDelta,
    TextBlockEnd,
    DataBlockStart,
    DataBlockDelta,
    DataBlockEnd,
    ThinkingBlockStart,
    ThinkingBlockDelta,
    ThinkingBlockEnd,
    HintBlock,
    ToolCallStart,
    ToolCallDelta,
    ToolCallEnd,
    ToolResultStart,
    ToolResultTextDelta,
    ToolResultDataDelta,
    ToolResultEnd,
    ExceedMaxIters,
    RequireUserConfirm,
    UserConfirmResult,
    UserInterrupt,
    RequireExternalExecution,
    ExternalExecutionResult,
    Custom,
}
```

Serialized as uppercase strings: `EventType::ReplyStart → "REPLY_START"`.

### EventBase (trait? or struct?)

```rust
// Decision: struct with shared fields, composed into each event struct via #[serde(flatten)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBase {
    pub id: String,           // default: Uuid::new_v4().as_simple().to_string()
    pub created_at: String,   // default: Utc::now().to_rfc3339()
    pub metadata: HashMap<String, Value>, // default: empty
}
```

### All Event Structs

| Event Struct | `type` literal | Key Fields | Serde |
|-------------|---------------|------------|-------|
| `ReplyStartEvent` | `"REPLY_START"` | `session_id`, `reply_id`, `name`, `role` | Tagged enum, flatten base |
| `ReplyEndEvent` | `"REPLY_END"` | `session_id`, `reply_id`, `finished_reason`, `error` | " |
| `ModelCallStartEvent` | `"MODEL_CALL_START"` | `reply_id`, `model_name` | " |
| `ModelCallEndEvent` | `"MODEL_CALL_END"` | `reply_id`, `input_tokens`, `output_tokens`, `finished_reason` | " |
| `TextBlockStartEvent` | `"TEXT_BLOCK_START"` | `reply_id`, `block_id` | " |
| `TextBlockDeltaEvent` | `"TEXT_BLOCK_DELTA"` | `reply_id`, `block_id`, `delta` | " |
| `TextBlockEndEvent` | `"TEXT_BLOCK_END"` | `reply_id`, `block_id` | " |
| `DataBlockStartEvent` | `"DATA_BLOCK_START"` | `reply_id`, `block_id`, `media_type` | " |
| `DataBlockDeltaEvent` | `"DATA_BLOCK_DELTA"` | `reply_id`, `block_id`, `data`, `media_type` | " |
| `DataBlockEndEvent` | `"DATA_BLOCK_END"` | `reply_id`, `block_id` | " |
| `ThinkingBlockStartEvent` | `"THINKING_BLOCK_START"` | `reply_id`, `block_id` | " |
| `ThinkingBlockDeltaEvent` | `"THINKING_BLOCK_DELTA"` | `reply_id`, `block_id`, `delta` | " |
| `ThinkingBlockEndEvent` | `"THINKING_BLOCK_END"` | `reply_id`, `block_id` | " |
| `HintBlockEvent` | `"HINT_BLOCK"` | `reply_id`, `block_id`, `source`, `hint` | " |
| `ToolCallStartEvent` | `"TOOL_CALL_START"` | `reply_id`, `tool_call_id`, `tool_call_name` | " |
| `ToolCallDeltaEvent` | `"TOOL_CALL_DELTA"` | `reply_id`, `tool_call_id`, `delta` | " |
| `ToolCallEndEvent` | `"TOOL_CALL_END"` | `reply_id`, `tool_call_id` | " |
| `ToolResultStartEvent` | `"TOOL_RESULT_START"` | `reply_id`, `tool_call_id`, `tool_call_name` | " |
| `ToolResultTextDeltaEvent` | `"TOOL_RESULT_TEXT_DELTA"` | `reply_id`, `tool_call_id`, `delta` | " |
| `ToolResultDataDeltaEvent` | `"TOOL_RESULT_DATA_DELTA"` | `reply_id`, `tool_call_id`, `block_id`, `media_type`, `data`, `url` | " |
| `ToolResultEndEvent` | `"TOOL_RESULT_END"` | `reply_id`, `tool_call_id`, `state`, `metadata` | " |
| `ExceedMaxItersEvent` | `"EXCEED_MAX_ITERS"` | `reply_id`, `name` | " |
| `RequireUserConfirmEvent` | `"REQUIRE_USER_CONFIRM"` | `reply_id`, `tool_calls` | " |
| `UserConfirmResultEvent` | `"USER_CONFIRM_RESULT"` | `reply_id`, `confirm_results` | " |
| `UserInterruptEvent` | `"USER_INTERRUPT"` | `reply_id` | " |
| `RequireExternalExecutionEvent` | `"REQUIRE_EXTERNAL_EXECUTION"` | `reply_id`, `tool_calls` | " |
| `ExternalExecutionResultEvent` | `"EXTERNAL_EXECUTION_RESULT"` | `reply_id`, `execution_results` | " |
| `CustomEvent` | `"CUSTOM"` | `name`, `value` | " |

### AgentEvent (Tagged Union)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    #[serde(rename = "REPLY_START")]
    ReplyStart(ReplyStartEvent),
    // ... all 27 variants
    #[serde(rename = "CUSTOM")]
    Custom(CustomEvent),
}
```

### ConfirmResult

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmResult {
    pub confirmed: bool,
    pub tool_call: ToolCallBlock,
    pub rules: Option<Vec<PermissionRule>>, // 占位类型
}
```

## JSON Serialization Contract

### TextBlockDeltaEvent 示例

```json
{
  "type": "TEXT_BLOCK_DELTA",
  "id": "<uuid>",
  "created_at": "<iso8601>",
  "metadata": {},
  "reply_id": "<reply-uuid>",
  "block_id": "<block-uuid>",
  "delta": "Hel"
}
```

### ToolResultEndEvent 示例

```json
{
  "type": "TOOL_RESULT_END",
  "id": "<uuid>",
  "created_at": "<iso8601>",
  "metadata": {},
  "reply_id": "<reply-uuid>",
  "tool_call_id": "<tool-call-uuid>",
  "state": "success",
  "metadata": {}
}
```

### RequireUserConfirmEvent 示例

```json
{
  "type": "REQUIRE_USER_CONFIRM",
  "id": "<uuid>",
  "created_at": "<iso8601>",
  "metadata": {},
  "reply_id": "<reply-uuid>",
  "tool_calls": [
    {
      "type": "tool_call",
      "id": "<tool-call-uuid>",
      "name": "delete_file",
      "input": "{\"path\":\"/etc/hosts\"}",
      "state": "asking",
      "suggested_rules": [],
      "created_at": "<iso8601>",
      "finished_at": null
    }
  ]
}
```

## Event Lifecycle Constraints

| 事件序列规则 | 约束 |
|-------------|------|
| REPLY_START → REPLY_END | REPLY_END 前必须有对应的 REPLY_START |
| MODEL_CALL_START → MODEL_CALL_END | MODEL_CALL_END 前必须有对应的 MODEL_CALL_START |
| BLOCK_START → BLOCK_DELTA... → BLOCK_END | Delta 和 End 依赖对应的 Start |
| TOOL_CALL_START → TOOL_CALL_DELTA... → TOOL_CALL_END → TOOL_RESULT_START → ... → TOOL_RESULT_END | 完整工具生命周期 |
| HINT_BLOCK | 一次性事件，无 Start/Delta/End |
| CUSTOM | 任意时间点，无约束 |
| USER_INTERRUPT | 仅对 parked reply 有效 |
| EXCEED_MAX_ITERS | 替换正常的 REPLY_END |
