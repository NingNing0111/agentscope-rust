# Contracts: Tool System

**Feature**: 006-tool-system | **Date**: 2026-07-29

This directory defines the interface contracts for the `agent_scope_tool` crate.

## Contract 1: Tool Trait

**File**: `crates/agent_scope_tool/src/tool_trait.rs`

The `Tool` trait is the abstract interface that all tools conform to. It is the core extension point of the Tool System.

### Trait Definition

```rust
/// Core abstraction for executable tools.
///
/// A Tool has metadata (name, description, input_schema) and an execution method (call).
/// It aligns with AgentScope Python's `ToolBase`.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the unique name of this tool.
    ///
    /// Used as the key in ToolKit and as `function.name` in the OpenAI schema.
    fn name(&self) -> &str;

    /// Returns a human-readable description.
    ///
    /// Included in the tool schema sent to the model to help it decide when to call.
    fn description(&self) -> &str;

    /// Returns the JSON Schema for this tool's input parameters.
    ///
    /// Format: `{"type": "object", "properties": {...}, "required": [...]}`
    fn input_schema(&self) -> serde_json::Value;

    /// Whether this tool can be safely called from multiple async tasks concurrently.
    ///
    /// Default: `true`
    fn is_concurrency_safe(&self) -> bool {
        true
    }

    /// Whether this tool has no observable side effects.
    ///
    /// Default: `false`
    fn is_read_only(&self) -> bool {
        false
    }

    /// Execute the tool with the given JSON input.
    ///
    /// # Arguments
    /// * `input` - A `serde_json::Value` representing the tool's parameters.
    ///   Must be a JSON object matching `input_schema()`.
    ///
    /// # Returns
    /// * `Ok(ToolExecOutput::Complete(chunk))` — one-shot result
    /// * `Ok(ToolExecOutput::Stream(stream))` — streaming result
    /// * `Err(ToolError)` — various failure modes
    fn call(&self, input: serde_json::Value) -> Result<ToolExecOutput, ToolError>;
}
```

### Contract Guarantees

| Guarantee | Description |
|-----------|-------------|
| Thread Safety | `Send + Sync` — can be shared across threads via `Arc<dyn Tool>` |
| No Unsafe | Zero `unsafe` code in any implementation |
| Idempotent Metadata | `name()`, `description()`, `input_schema()` always return the same values for the same instance |
| Panic Boundary | `call()` implementations MUST NOT propagate panics to the caller |
| Error Typed | All failures are through `ToolError`, not `Box<dyn Error>` or string matching |

---

## Contract 2: ToolKit

**File**: `crates/agent_scope_tool/src/toolkit.rs`

### Struct

```rust
/// A registry of Tool instances with schema export and call dispatch.
///
/// Aligns with AgentScope Python's `Toolkit`.
#[derive(Default)]
pub struct ToolKit {
    tools: HashMap<String, Box<dyn Tool>>,
}
```

### Methods

```rust
impl ToolKit {
    /// Creates an empty ToolKit.
    pub fn new() -> Self;

    /// Registers a tool. If a tool with the same name exists, it is replaced.
    ///
    /// # Arguments
    /// * `tool` - Any type implementing `Tool + 'static`.
    pub fn register(&mut self, tool: impl Tool + 'static);

    /// Returns OpenAI-compatible function schema for all registered tools.
    ///
    /// Output format:
    /// ```json
    /// [{
    ///   "type": "function",
    ///   "function": {
    ///     "name": "...",
    ///     "description": "...",
    ///     "parameters": { "type": "object", "properties": {...}, "required": [...] }
    ///   }
    /// }]
    /// ```
    pub fn get_tool_schemas(&self) -> Vec<serde_json::Value>;

    /// Dispatches a tool call to the named tool.
    ///
    /// # Arguments
    /// * `tool_call` - The `ToolCallBlock` from a model response.
    ///
    /// # Returns
    /// * `Ok(ToolExecOutput)` — the tool's execution result
    /// * `Err(ToolError::NotFound)` — if no tool matches `tool_call.name`
    /// * `Err(ToolError::InvalidInput)` — if `tool_call.input` is not valid JSON
    /// * `Err(ToolError::Execution)` — if the tool's handler panics
    pub fn call_tool(
        &self,
        tool_call: &agent_scope_message::ToolCallBlock,
    ) -> Result<ToolExecOutput, ToolError>;

    /// Removes all registered tools.
    pub fn clear(&mut self);

    /// Returns the number of registered tools.
    pub fn len(&self) -> usize;

    /// Returns true if no tools are registered.
    pub fn is_empty(&self) -> bool;

    /// Checks if a tool with the given name is registered.
    pub fn contains(&self, name: &str) -> bool;

    /// Removes a specific tool by name.
    pub fn remove(&mut self, name: &str) -> Option<Box<dyn Tool>>;
}
```

### Contract Guarantees

| Guarantee | Description |
|-----------|-------------|
| Name Override | `register()` with duplicate name overwrites (idempotent) |
| Empty Safe | `get_tool_schemas()` on empty ToolKit returns `[]` |
| Missing Safe | `call_tool()` for missing tool returns `Err(NotFound)`, never panics |
| Static Dispatch | Tool name → handler is O(1) HashMap lookup |

---

## Contract 3: FunctionTool

**File**: `crates/agent_scope_tool/src/function.rs`

### Struct

```rust
/// Adapts an async handler function into a Tool.
///
/// Uses `schemars::JsonSchema` to automatically derive the input schema from `T`.
pub struct FunctionTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
    handler: Box<dyn FunctionToolHandler>,
}
```

### Constructors

```rust
impl FunctionTool {
    /// Creates a FunctionTool with auto-derived schema.
    ///
    /// # Type Parameters
    /// * `T` - Must implement `schemars::JsonSchema` + `Deserialize<'de>`.
    ///   Its schema is derived at construction time.
    /// * `F` - Handler function type.
    /// * `R` - Return type implementing `IntoChunk`.
    pub fn new<F, Fut, T, R>(
        name: impl Into<String>,
        description: impl Into<String>,
        handler: F,
    ) -> Self
    where
        T: schemars::JsonSchema + for<'de> Deserialize<'de> + Send + 'static,
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send,
        R: IntoChunk;

    /// Creates a FunctionTool with manually provided schema (escape hatch).
    ///
    /// The caller is responsible for ensuring the schema matches the handler's
    /// expected input type. No validation is performed.
    pub fn new_with_schema<F, Fut, R>(
        name: impl Into<String>,
        description: impl Into<String>,
        schema: serde_json::Value,
        handler: F,
    ) -> Self
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send,
        R: IntoChunk;
}
```

### IntoChunk Trait

```rust
/// Converts handler return values into ToolResultBlock.
///
/// Implementations:
/// - `String` → `ToolResultBlock { output: Text(s), state: Success, is_last: true }`
/// - `ToolResultBlock` → passthrough
pub trait IntoChunk: Send + 'static {
    fn into_chunk(self, tool_name: &str) -> ToolResultBlock;
}
```

---

## Contract 4: Error Types

**File**: `crates/agent_scope_tool/src/tool_trait.rs`

```rust
/// Typed errors for all tool operations.
///
/// Aligns with the Constitution's Error Model (Art.13).
#[derive(Debug, Clone, Error)]
pub enum ToolError {
    #[error("tool '{tool_name}' not found")]
    NotFound { tool_name: String },

    #[error("invalid input for tool '{tool_name}': {reason}")]
    InvalidInput { tool_name: String, reason: String },

    #[error("tool '{tool_name}' execution failed: {reason}")]
    Execution { tool_name: String, reason: String },

    #[error("tool '{tool_name}' was interrupted")]
    Interrupted { tool_name: String },
}
```

---

## Contract 5: Crate Dependencies

**File**: `crates/agent_scope_tool/Cargo.toml`

```toml
[package]
name = "agent_scope_tool"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "AgentScope Tool System — Tool trait, FunctionTool adapter, ToolKit registry"

[dependencies]
agent_scope_message = { path = "../agent_scope_message" }
agent_scope_model = { path = "../agent_scope_model" }
serde.workspace = true
serde_json.workspace = true
schemars.workspace = true
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
futures = "0.3"
thiserror = "2"
```

### Dependency Graph

```
agent_scope_tool
├── agent_scope_message (ToolCallBlock, ToolResultBlock)
├── agent_scope_model (ToolChoice for US3 validation)
├── schemars (JSON Schema derivation)
├── serde / serde_json
├── async-trait
├── tokio
├── futures (Stream)
└── thiserror (Error derive)
```

**Direction**: `agent_scope_tool` → `agent_scope_message`, `agent_scope_model` (one-way, no cycles).
