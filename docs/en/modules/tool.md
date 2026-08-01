# Tool System

> One-liner: wrap async Rust functions into structured tools that agents can call — defining the `Tool` trait, the `FunctionTool` adapter, the `ToolKit` registry, and the full ToolCall/ToolResult lifecycle.

## 1. Overview

This module covers the `agent_scope_tool` crate, which sits between models and agents: models emit `ToolCallBlock`s, the tool system validates input, dispatches to registered tools, returns `ToolResultBlock`s, and exports OpenAI-compatible function schemas upward.

**When to use**:

- Registering custom tools for an agent
- Wrapping existing async Rust functions as `FunctionTool`
- Exporting tool schemas for model tool selection
- Handling one-shot or streaming tool results
- Managing Skill directories, Skill objects, and the SkillViewer tool

**Prerequisites**:
- [Message & Basic Types](./message-types.md) — the `ToolCallBlock` / `ToolResultBlock` data structures
- [Event & Streaming](./event-streaming.md) — the `ToolCall*` / `ToolResult*` event sequence
- [Agent System](./agent.md) — how agents consume `ToolKit`

## 2. Core Concepts & Main Public Types

### 2.1 The `Tool` Trait

`Tool` is the central extension point and requires `Send + Sync`, so implementations can be shared via `Arc<dyn Tool>` or boxed trait objects:

| Method | Description |
|--------|-------------|
| `name() -> &str` | Unique tool name; both the registry key and `function.name` in exported schemas |
| `description() -> &str` | Human-readable description shown to the model |
| `input_schema() -> JsonValue` | JSON Schema describing the input parameters |
| `is_concurrency_safe() -> bool` | Whether concurrent calls are safe, default `true` |
| `is_read_only() -> bool` | Whether the tool has no observable side effects, default `false` |
| `call(input: JsonValue) -> Result<ToolExecOutput, ToolError>` | Execution entry point |

Contract guarantees:

- Stable metadata: `name` / `description` / `input_schema` should be stable values
- Typed failures: all failures go through `ToolError`
- Panic boundary: `call()` must not let panics escape to the caller

### 2.2 `ToolExecOutput`

Tool execution results are normalized into two forms:

```rust
pub enum ToolExecOutput {
    Complete(ToolResultBlock),
    Stream(Pin<Box<dyn Stream<Item = Result<ToolResultBlock, ToolError>> + Send>>),
}
```

- `Complete`: one-shot completion, typical for synchronous or short operations
- `Stream`: streaming output; the framework **does not auto-accumulate**, so the caller must consume the stream and rely on `is_last` to detect completion

### 2.3 `ToolError`

The tool layer's typed error model:

| Variant | Trigger condition |
|---------|-------------------|
| `NotFound { tool_name }` | The tool is not registered in `ToolKit` |
| `InvalidInput { tool_name, reason }` | Input JSON cannot be deserialized into the tool's parameter type, or the raw input string is not valid JSON |
| `Execution { tool_name, reason }` | Tool execution failed, including handler panic |
| `Interrupted { tool_name }` | The tool was interrupted |
| `SkillNotFound { skill_name }` | The requested skill does not exist |

### 2.4 `FunctionTool`

`FunctionTool` adapts ordinary async functions into `Tool`s:

- `FunctionTool::new(name, description, handler)`: automatically derives JSON Schema from the parameter type `T: JsonSchema + DeserializeOwned`
- `FunctionTool::new_with_schema(name, description, schema, handler)`: manual-schema escape hatch

Its internal behavior:

1. Deserialize `JsonValue` into the handler input type `T`
2. Catch handler panics (`catch_unwind`)
3. Convert the return value into `ToolResultBlock` through `IntoChunk`

Built-in `IntoChunk` implementations:

- `String` → `ToolOutput::Text`, `state: Success`, `is_last: true`
- `ToolResultBlock` → pass-through, but force `is_last = true`

### 2.5 `ToolKit`

`ToolKit` is the registry and dispatch center:

| Capability | Description |
|------------|-------------|
| `new()` | Creates the registry and auto-registers the `SkillViewer` tool in the default `basic` group |
| `register(tool)` | Registers a tool; duplicate names overwrite (matching Python AgentScope behavior) |
| `remove(name)` / `clear()` | Remove one or all tools |
| `contains(name)` / `len()` / `is_empty()` | Queries |
| `get_tool_schemas()` | Exports an OpenAI-compatible function schema array |
| `call_tool(&ToolCallBlock)` | Parses the `input` JSON and dispatches by `name` |
| `add_skill_dir()` / `add_skill()` / `add_skill_loader()` | Registers skill sources |
| `list_skills()` / `get_skill_instructions()` | Enumerates skills and renders the `<agent-skills>` prompt fragment |

`ToolKit::call_tool()` dispatch flow:

1. Look up the tool by `tool_call.name`, returning `NotFound` if absent
2. Parse `tool_call.input` as `JsonValue`, returning `InvalidInput` on parse failure
3. Call `tool.call(input).await`

### 2.6 ToolCall / ToolResult Lifecycle

Working with the message and event layers, the observable lifecycle of one tool invocation is:

```text
ToolCallBlock(Pending)
→ ToolCallStart / ToolCallDelta* / ToolCallEnd
→ ToolResultStart / ToolResultTextDelta* / ToolResultDataDelta*
→ ToolResultEnd(Success | Error | Interrupted | Denied)
```

Within the foundation message model:

- `ToolCallBlock.input` is a **raw JSON string**; the Tool layer is where parsing happens
- `ToolCallState`: `pending` → `asking` → `allowed` → `submitted` → `finished`
- `ToolResultState`: `running` → `success` / `error` / `interrupted` / `denied`
- `ToolResultBlock.is_last` marks the final chunk in a streaming tool result

## 3. Quick Example

The repository's calculator tool is the canonical `FunctionTool` example:

<!-- source: examples/common.rs:L311-L316 -->
```rust
pub fn create_calculator_tool() -> FunctionTool {
    FunctionTool::new(
        "calculator",
        "Evaluate a mathematical expression. Supports +, -, *, /, ^ (power), (), and constants pi/e. Example: \"2 + 3 * (4 - 1) ^ 2\"",
        calc_handler,
    )
}
```

This tool can then be added to a `ToolKit` and injected into a `ReActAgent`. The full call chain appears in `examples/common.rs` (`build_agent()`) and in the tool-call tests in `examples/streaming_tool_test.rs`.

## 4. Usage Patterns

### 4.1 Auto-Deriving Schema from Typed Input

If the parameter type implements `Deserialize + JsonSchema`, you can wrap it directly:

```rust
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SearchInput {
    query: String,
}

async fn search(input: SearchInput) -> String {
    format!("Results for: {}", input.query)
}

let tool = FunctionTool::new("search", "Search the web", search);
```

This is the recommended path: the schema and the Rust type share one source of truth, reducing drift from handwritten JSON Schema.

### 4.2 Registering into `ToolKit`

```rust
let mut tk = ToolKit::new();
tk.register(create_calculator_tool());
let schemas = tk.get_tool_schemas();
```

`schemas` are exported in OpenAI-compatible format:

```json
{
  "type": "function",
  "function": {
    "name": "calculator",
    "description": "...",
    "parameters": { "type": "object", "properties": { ... } }
  }
}
```

### 4.3 Letting an Agent Actually Call the Tool

The streaming tool-call test shows the minimal path for a real agent-triggered tool call:

<!-- source: examples/streaming_tool_test.rs:L229-L241 -->
```rust
async fn run_single_tool_call(
    agent: &impl Agent,
) -> Result<EventTrace, Box<dyn std::error::Error>> {
    let msg = user_msg("user", "Calculate 3.14 * 2.718 using the calculator tool.")
        .map_err(|e| format!("{e:?}"))?;
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;

    let mut trace = EventTrace::new();
    while let Some(event) = stream.next().await {
        trace.record(&event);
    }
```

This shows that once a tool is registered and natural-language input is sent to the agent, the ToolCall/ToolResult lifecycle is driven by Agent + ToolKit.

### 4.4 Manual Dispatch of a `ToolCallBlock`

In scenarios outside an agent loop, you can dispatch directly:

```rust
let call = ToolCallBlock::new(
    "tc-1".into(),
    "calculator".into(),
    r#"{"expression":"2+2"}"#.into(),
);
let output = toolkit.call_tool(&call).await?;
```

Note: `call_tool()` parses `call.input` first, so the string must contain valid JSON.

### 4.5 Skill Integration

Besides ordinary tools, `ToolKit` can register:

- direct Skill objects: `add_skill(skill)`
- local directories: `add_skill_dir(path)`
- custom loaders: `add_skill_loader(loader)`

`ToolKit::new()` auto-registers `SkillViewer`, making available skills exposable to the agent.

## 5. Errors & Unsupported Capabilities

| Error type | Trigger condition |
|------------|-------------------|
| `ToolError::NotFound` | Calling an unregistered tool |
| `ToolError::InvalidInput` | `ToolCallBlock.input` is not valid JSON, or the JSON cannot be deserialized into the target parameter type |
| `ToolError::Execution` | Handler runtime failure or panic (panics are caught and converted) |
| `ToolError::Interrupted` | Tool execution interrupted |
| `ToolError::SkillNotFound` | Skill name does not exist |

**Unsupported capabilities**:

- There is no fixed global `UnsupportedFeature` list; whether a tool supports streaming, concurrency safety, or read-only behavior is defined by that tool's implementation
- If a tool does not support some capability, it should fail explicitly via `ToolError` or a higher-level protocol path, never silently degrade

**FAQ**:

- *Why is tool input a string first and only then parsed as JSON?*: to stay consistent with the stable `ToolCallBlock.input` protocol at the message layer; the Foundation layer does not eagerly parse arguments.
- *What happens if the handler panics?*: `FunctionTool` wraps the future with `catch_unwind`, and the final result is `ToolError::Execution { reason: "handler panicked" }`.

## 6. Compatibility

- **Compatibility level**: **L1** (ToolCall/ToolResult data structures and schema export format); **L2** (registration, dispatch, typed errors, skill integration, and related behavior)
- **Authoritative source**: `specs/001-compatibility-baseline/capability-matrix.json`
- **Known deviations**:
  - The matrix `status` field is currently `NOT_ANALYZED` for all entries; levels on this page are cross-verified against the `tool` category `target_level` + `specs/006-tool-system`, `specs/013-skill-tool-spec` + actual code state.
  - `ToolKit::new()` auto-registers `SkillViewer` into the default group — a Rust-side ergonomics enhancement, so a fresh toolkit is not truly empty.
  - `FunctionTool` auto-derives schema via `schemars`, which is the Rust-idiomatic path under a static type system; Python constructs schemas differently at runtime.
  - `call_tool()` parses `ToolCallBlock.input` only at the Tool layer, preserving the raw-string protocol at the message layer.
- **Unsupported capabilities**: no universal fixed list; capabilities are either explicitly exposed or explicitly rejected by each concrete tool.

## 7. See Also

- [Agent System](./agent.md) — the main consumer of `ToolKit`
- [Event & Streaming](./event-streaming.md) — the ToolCall/ToolResult event lifecycle
- [Message & Basic Types](./message-types.md) — the `ToolCallBlock` / `ToolResultBlock` structures
- [Skill](./skill.md) — the Skill integration part of `ToolKit`
