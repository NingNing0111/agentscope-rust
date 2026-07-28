# Data Model: Tool System

**Feature**: 006-tool-system | **Date**: 2026-07-29

## Entities

### 1. Tool (trait)

The core abstraction for any executable tool.

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique name used for registration and calling.
    fn name(&self) -> &str;

    /// Human-readable description for the model.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's parameters.
    /// Format: `{"type": "object", "properties": {...}, "required": [...]}`
    fn input_schema(&self) -> serde_json::Value;

    /// Whether this tool can be safely called concurrently.
    /// Default: true
    fn is_concurrency_safe(&self) -> bool {
        true
    }

    /// Whether this tool has no side effects.
    /// Default: false
    fn is_read_only(&self) -> bool {
        false
    }

    /// Execute the tool with the given JSON input.
    fn call(
        &self,
        input: serde_json::Value,
    ) -> Result<ToolExecOutput, ToolError>;
}
```

**Relationships**:
- `Tool` → `FunctionTool` (implements)
- `Tool` → `ToolKit` (owning via `Box<dyn Tool>`)

---

### 2. ToolExecOutput (enum)

The result of a tool execution — complete or streaming.

```rust
pub enum ToolExecOutput {
    /// One-shot execution result.
    Complete(ToolResultBlock),
    /// Streaming execution — multiple chunks.
    Stream(Pin<Box<dyn Stream<Item = Result<ToolResultBlock, ToolError>> + Send>>),
}
```

**States**:
- `Complete`: done, single `ToolResultBlock` with `is_last: true` and `state: Success`
- `Stream`: in-progress, caller consumes stream; each `ToolResultBlock` has `is_last` set by tool impl

---

### 3. ToolChunk (type alias)

```rust
pub type ToolChunk = agent_scope_message::ToolResultBlock;
```

**Modification**: `ToolResultBlock` gains `is_last: bool` field (`#[serde(default)]`, default `false`).

```rust
pub struct ToolResultBlock {
    // ...existing fields...
    /// True when this is the final chunk in a stream.
    #[serde(default)]
    pub is_last: bool,
}
```

---

### 4. ToolError (enum)

Type-safe error taxonomy for tool operations.

```rust
#[derive(Debug, Clone, Error)]
pub enum ToolError {
    /// Tool not found in ToolKit.
    #[error("tool '{tool_name}' not found")]
    NotFound { tool_name: String },

    /// Input deserialization failed.
    #[error("invalid input for tool '{tool_name}': {reason}")]
    InvalidInput { tool_name: String, reason: String },

    /// Tool execution failed (panic or runtime error).
    #[error("tool '{tool_name}' execution failed: {reason}")]
    Execution { tool_name: String, reason: String },

    /// Tool execution was interrupted.
    #[error("tool '{tool_name}' interrupted")]
    Interrupted { tool_name: String },
}
```

**Error Category Map** (per Constitution Art.13):
| ToolError Variant | Error Category |
|-------------------|---------------|
| `NotFound` | `ToolError` (mapping error) |
| `InvalidInput` | `ValidationError` |
| `Execution` | `ToolError` |
| `Interrupted` | `CancellationError` |

---

### 5. IntoChunk (trait)

Return-value conversion trait for `FunctionTool` handlers.

```rust
pub trait IntoChunk: Send + 'static {
    fn into_chunk(self, tool_name: &str) -> ToolResultBlock;
}
```

**Implementations**:
| Type | Behavior |
|------|----------|
| `String` | Wraps as `ToolResultBlock { output: Text(s), state: Success, is_last: true }` |
| `ToolResultBlock` | Pass-through, sets `is_last: true` if not already set |
| `ToolChunk` | Same as `ToolResultBlock` (they're the same type) |

---

### 6. FunctionTool (struct)

Adapts a handler function + JSON Schema into a `Tool`.

```rust
pub struct FunctionTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
    handler: Box<dyn FunctionToolHandler>,
}
```

**Constructors**:
- `FunctionTool::new::<T: JsonSchema>(name, description, handler)` — auto-derives schema from `T`
- `FunctionTool::new_with_schema(name, description, schema, handler)` — manual schema (escape hatch)

**Internal**: `FunctionToolHandler` trait for type-erased handler execution:
```rust
trait FunctionToolHandler: Send + Sync {
    fn call(&self, input: serde_json::Value) -> Result<ToolExecOutput, ToolError>;
}
```

---

### 7. ToolKit (struct)

Tool registry, schema exporter, and call dispatcher.

```rust
pub struct ToolKit {
    tools: HashMap<String, Box<dyn Tool>>,
}
```

**Operations**:

| Method | Behavior |
|--------|----------|
| `new()` | Empty ToolKit |
| `register(tool: impl Tool + 'static)` | Insert/replace tool by name |
| `get_tool_schemas()` | `Vec<JsonValue>` in OpenAI function format |
| `call_tool(tool_call: &ToolCallBlock)` | Lookup + deserialize input + call tool |
| `clear()` | Remove all tools |
| `len()` / `is_empty()` | Query count |
| `contains(name)` | Check existence |
| `remove(name)` | Remove single tool |

**Relationships**:
- `ToolKit` → `0..*` `Tool` (via `Box<dyn Tool>`)
- `ToolKit` ↔ `ChatModel` (via schema export + `ToolCallBlock` dispatch)

---

### 8. ToolCallBlock (existing, `agent_scope_message`)

Already implemented in `crates/agent_scope_message/src/block.rs`:

```rust
pub struct ToolCallBlock {
    pub id: String,
    pub name: String,
    pub input: String,       // Raw JSON string
    pub state: ToolCallState,
    pub suggested_rules: Vec<PermissionRule>,
    pub created_at: String,
    pub finished_at: Option<String>,
}
```

---

## State Transitions

### Tool Execution Lifecycle (Complete path)

```
Input(JsonValue)
  │
  ▼
Tool::call()
  ├── deserialize JsonValue → T (via schemars::schema_for!)
  │   └── fail → ToolError::InvalidInput { tool_name, reason }
  ├── execute handler(T)
  │   └── panic → ToolError::Execution { tool_name, reason }
  │   └── Ok → T.into_chunk(tool_name)
  └── return Ok(ToolExecOutput::Complete(chunk))
```

### ToolKit Call Dispatch

```
ToolKit::call_tool(tool_call: &ToolCallBlock)
  │
  ├── lookup tool_call.name in self.tools
  │   └── miss → Err(ToolError::NotFound)
  ├── parse tool_call.input as JsonValue
  │   └── fail → Err(ToolError::InvalidInput)
  ├── tool.call(json_value)
  │   └── Err(e) → propagate
  └── Ok(ToolExecOutput)
```

---

## Validation Rules

1. **name uniqueness**: ToolKit enforces name-as-key; duplicate registration overwrites
2. **Schema validity**: `FunctionTool::new()` validates that `schemars::schema_for!(T)` produces valid JSON (compile-time via static typing + runtime `expect` for serde failure)
3. **Input validation**: `call()` validates JSON can deserialize into `T` before executing handler
4. **Stream is_last**: Each stream chunk SHOULD have `is_last` set appropriately by tool impl; not enforced by framework
