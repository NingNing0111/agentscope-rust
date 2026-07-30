# Contract: MemoryMiddleware

**Feature**: 009-memory-system  
**Contract Type**: Middleware integration contract  
**Depends on**: Middleware trait (`agent_scope_agent::middleware::Middleware`), Memory trait (`agent_scope_memory::Memory`)

## Structure

```rust
pub struct MemoryMiddleware {
    memory: Arc<dyn Memory>,
    config: MemoryConfig,
    retrieval_handle: Mutex<Option<tokio::task::JoinHandle<Result<Option<String>, MemoryError>>>>,
    cached_user_input: Mutex<Option<String>>,
}

impl MemoryMiddleware {
    pub fn new(memory: Arc<dyn Memory>, config: MemoryConfig) -> Self;
    pub fn with_config(workdir: &str, memory_dir: &str, config: MemoryConfig) -> Self;
}
```

## Middleware Trait Implementation

MemoryMiddleware implements `Middleware` (from `agent_scope_agent`). Adds one new hook + uses two existing hooks:

### Hook: `on_system_prompt` (NEW — added to Middleware trait)

```rust
/// Append memory instructions and bounded MEMORY.md to the system prompt.
async fn on_system_prompt(
    &self,
    agent_name: &str,
    current_prompt: &mut String,
) -> Result<(), AgentError>;
```

**Behavior**:
1. Read `MEMORY.md` via `memory.get_index_content()`.
2. If empty, inject: "Your MEMORY.md is currently empty..."
3. Truncate to `max_index_tokens`; append `<<<TRUNCATED>>>` notice if truncated.
4. Inject memory usage instructions (from config) + truncated `MEMORY.md` into `current_prompt`.

**Preconditions**: Memory directory exists (created idempotently by `get_index_content()`).

**Postconditions**: `current_prompt` is augmented with memory context.

---

### Hook: `pre_reply` (EXISTING — used for async retrieval)

```rust
async fn pre_reply(
    &self,
    agent_name: &str,
    input: &mut Option<Vec<Msg>>,
) -> Result<(), AgentError>;
```

**Behavior**:
1. If `config.retrieval_async == false`: return immediately (no-op).
2. Extract user text from `input` messages.
3. Clone `self.memory` and `self.config` for the spawned task.
4. Spawn `tokio::spawn(async { memory.retrieve_relevant(query, model, max_results).await })`.
5. Store `JoinHandle` in `self.retrieval_handle`.

**Preconditions**: `input` is `Some(Vec<Msg>)`.

**Postconditions**: Retrieval task is running in background.

---

### Hook: `pre_reasoning` (EXISTING — used for retrieval result injection)

```rust
async fn pre_reasoning(
    &self,
    agent_name: &str,
    messages: &mut Vec<Msg>,
    tools: &mut Option<Vec<JsonValue>>,
) -> Result<(), AgentError>;
```

**Behavior**:
1. Lock `retrieval_handle`.
2. If handle exists and `is_finished()`:
   - Take the handle, await result.
   - If result is `Ok(Some(content))`: push a `ContentBlock::Hint(HintBlock::new(HintContent::Text(content)))` into the last user message or create a new one.
   - If result is `Err(_)` or `None`: no injection.
3. If handle exists but not finished: leave it for next `pre_reasoning` call (ReAct loop iterates).

**Preconditions**: `pre_reply` was called earlier in the same reply lifecycle.

**Postconditions**: Retrieval result injected as `HintBlock` if available.

---

## Hook Invocation Order in ReActAgent

```text
user calls agent.reply(input)
  │
  ├── pre_reply(input)         ← MemoryMiddleware: spawn retrieval task
  │
  ├── on_system_prompt(prompt) ← MemoryMiddleware: inject MEMORY.md + instructions
  │
  ├── [ReAct loop]
  │     ├── pre_reasoning(msgs, tools) ← MemoryMiddleware: poll retrieval + inject HintBlock
  │     ├── model.call(msgs)
  │     ├── post_reasoning(response)
  │     ├── pre_acting(tool_call)
  │     └── post_acting(result)
  │
  └── post_reply(result)
```

**Key constraint**: `on_system_prompt` must be called AFTER `pre_reply` (so retrieval task is spawned before system prompt is built) and BEFORE the first model call. The ReActAgent must be updated to call all middleware hooks in this exact order.

## Middleware Trait Extension

The existing `Middleware` trait in `agent_scope_agent/src/middleware.rs` gains one method:

```rust
/// Called after pre_reply, before the first model call.
/// Appends memory/index context to the system prompt.
async fn on_system_prompt(
    &self,
    _agent_name: &str,
    _current_prompt: &mut String,
) -> Result<(), AgentError> {
    Ok(())
}
```

This is backward-compatible — the default no-op means existing middleware implementations don't need changes.

## Error Handling

- Retrieval task failure → silent skip (no HintBlock), logged at `warn` level.
- Index read failure → inject empty index placeholder, logged at `warn`.
- `on_system_prompt` failure → propagate `AgentError` (blocks the reply, as memory context is critical).
