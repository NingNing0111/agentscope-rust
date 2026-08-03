# Agent System / Agent

> One-liner: `agent_scope_agent` is the orchestration layer between models, messages, tools, events, memory, and session state — exposing `reply()` / `reply_stream()` / `observe()` through the unified `Agent` trait, and implementing the reasoning → acting multi-step tool loop with `ReActAgent`.

## 1. Module Overview (Overview)

This module corresponds to the `agent_scope_agent` crate. It does not implement concrete model providers directly, and it does not define tool input/output formats directly. Instead, it composes the following modules:

- [Model Abstraction](./model.md): calls models through `Arc<dyn ChatModel>`
- [Message & Basic Types](./message-types.md): represents context and replies with `Msg` / `ContentBlock`
- [Tool System](./tool.md): registers and executes tools through `ToolKit`
- [Event & Streaming](./event-streaming.md): exposes observable runtime traces through `AgentEvent`
- Memory: injects long-term memory through `MemoryMiddleware`
- Session management: stores context, session id, and reply context through `AgentState`

**When to use**: building a conversational Agent; registering tools for an Agent; consuming real-time event streams; attaching middleware before/after replies; configuring permissions for tool execution; enabling context compression and memory augmentation.

**Prerequisites**: read [Model Abstraction](./model.md), [Tool System](./tool.md), and [Event & Streaming](./event-streaming.md) first. If you only want to run something quickly, start with [Getting Started](../getting-started.md).

**Complete interactive example**: the repository provides `agent_demo` as an end-to-end Agent module showcase at `examples/agent-demo/main.rs`. It reads real DashScope credentials from `.env`/`API_KEY`, renders the event stream through `ReActAgent::reply_stream()`, and demonstrates `ToolKit`, `FunctionTool`, and `PermissionContext`:

```bash
cargo run --example agent_demo -- --model qwen-plus --show-events
```

## 2. Core Concepts & Main Public Types (Core Concepts)

### 2.1 `Agent` trait

`Agent` is the common interface for all Agent types. The main current implementation is `ReActAgent`:

| Method | Description |
|--------|-------------|
| `reply(input)` | Non-streaming entry point; returns the final assistant `Msg` |
| `reply_stream(input)` | Streaming entry point; returns `Stream<Item = AgentEvent>` |
| `observe(input)` | Appends messages to context without triggering a model reply |
| `name()` | Returns the configured Agent name |
| `state()` | Trait-level state access; for `ReActAgent`, use `try_state()` |

`reply(None)` means “continue replying from existing context”. If the context is empty, it returns `AgentError::NoContentToReply`.

### 2.2 `ReActAgent`

`ReActAgent` is the main Agent type today. Internally it executes a reasoning → acting loop:

```text
用户输入 / 已有上下文
→ middleware.pre_reply
→ middleware.on_system_prompt
→ loop(max_iters):
   → middleware.pre_reasoning
   → model.call(messages, tool_schemas, tool_choice)
   → middleware.post_reasoning
   → 如果模型返回文本：累积为最终回复
   → 如果模型返回 ToolCallBlock：权限检查 → middleware.pre_acting → ToolKit.call_tool → middleware.post_acting
   → 工具结果追加回上下文，进入下一轮 reasoning
→ middleware.post_reply
→ 返回最终 Msg 或事件流收尾
```

Important behaviors:

- The model may return non-streaming `Complete(ChatResponse)` or streaming `Stream(...)`; in the non-streaming `reply()` path, `ReActAgent` accumulates the complete response with `StreamAccumulator`.
- A single `ReActAgent` allows only one active `reply()` or `reply_stream()` at a time; starting a second concurrent reply returns `AgentError::AlreadyStreaming`.
- `interrupt()` can interrupt an in-progress reply; the framework emits `UserInterrupt` and closes with `ReplyEnd(finished_reason: interrupted)`.
- `try_state()` provides lock-aware state access; do not call `ReActAgent`'s `state()` directly, because it panics.

### 2.3 `AgentConfig`

`AgentConfig` is the construction-time configuration and is built with a builder:

| Field / builder | Description |
|-----------------|-------------|
| `name(...)` | Agent name, required; used in messages and events |
| `system_prompt(...)` | System prompt; may be empty |
| `model(...)` | `Arc<dyn ChatModel>`, required |
| `toolkit(...)` | Optional tool registry |
| `permission_context(...)` / `permission_mode(...)` | Permission context for tool execution |
| `with_stream_channel_capacity(...)` | Streaming event channel capacity; `None` means unbounded, `Some(n)` must have `n > 0` |

The minimal construction requires `name` and `model`. If either required field is missing, `build()` returns `AgentError::InvalidConfig`.

### 2.4 `ReActConfig`

Controls ReAct loop behavior:

| Field | Default | Description |
|-------|---------|-------------|
| `max_iters` | `20` | Maximum reasoning/acting iterations per reply; must be greater than 0 |
| `stop_on_reject` | `false` | Whether to stop when tool permission is rejected |
| `interruption_message` | `"The execution was interrupted."` | Assistant text returned on interruption |
| `structured_output_grace_iters` | `3` | Extra tolerance iterations when structured output parsing fails |

### 2.5 `ContextConfig`

Controls context-window compression:

| Field | Default | Description |
|-------|---------|-------------|
| `enable` | `false` | Whether compression is enabled |
| `trigger_ratio` | `0.8` | Trigger when tokens exceed `context_size * trigger_ratio` |
| `reserve_ratio` | `0.1` | Context ratio reserved for the model reply |
| `compression_prompt` | `"<STD_CP_PROMPT>"` | System prompt for the compression model call |
| `tool_result_limit` | `4096` | Truncation limit for tool-result content |

When enabled, the Agent estimates the current context token count before each model call; if it exceeds the threshold, it invokes compression to trim context.

### 2.6 `Middleware`

`Middleware` is the Agent extension point. All hooks are no-op by default and are called in FIFO registration order:

| Hook | Timing | Typical Use |
|------|--------|-------------|
| `pre_reply` | Before reply starts | Modify input, start async retrieval, capture model reference |
| `post_reply` | After reply ends | Write audit logs, persist state |
| `on_system_prompt` | Before the first model call | Append memory, policy, or dynamic instructions |
| `pre_reasoning` | Before each model call | Modify context messages or tool schemas |
| `post_reasoning` | After model response | Record model response, collect usage stats |
| `pre_acting` | Before tool execution | Modify or reject a tool call |
| `post_acting` | After tool execution | Record tool result, trigger side effects |
| `pre_observe` | When `observe()` is called | Normalize observed messages |
| `pre_print` | Before output rendering | Modify display content |

The built-in `MemoryMiddleware` appends the `MEMORY.md` index in `on_system_prompt`, and can asynchronously retrieve relevant memories in `pre_reply` / `pre_reasoning`, injecting them into user messages as `HintBlock`s.

### 2.7 Permission System

Before executing a tool, the Agent uses `PermissionEngine` to check `PermissionContext`. Current permission modes include:

| Mode | Default Behavior |
|------|------------------|
| `Default` | Allow when no rule matches |
| `AcceptEdits` | Allow when no rule matches |
| `Explore` | Read-only planning mode; denies uncategorized tool calls unless an allow rule matches |
| `Bypass` | Allow when no rule matches |
| `DontAsk` | Converts ask decisions to deny; allows when no rule matches |

Rule priority is: `deny` → `ask` → `allow` → mode default. Rules support exact matching, `*` wildcard matching, and `prefix*` prefix matching; `rule_content` can match substrings inside serialized tool input.

### 2.8 SubAgent collaboration

`agent_scope_agent` exposes an in-process SubAgent collaboration layer for bounded parent-to-collaborator tasks. A parent creates a `SubAgentRegistry`, registers named `SubAgent` values or validates reusable `SubAgentTemplate` blueprints, and sends an explicit `DelegationRequest` to `delegate_once()` or `delegate_many()`.

Successful results are returned as `CollaborationResult` with `CollaborationStatus::Succeeded` and a result `Msg` whose `name` remains the SubAgent speaker identity. `ContextSharingPolicy` defaults to least privilege, and `CapabilityScope` gates tools, memory, session, workspace, sandbox, model access, and side effects. Deferred Python app-service/message-bus/distributed patterns return `SubAgentErrorCategory::UnsupportedFeature` rather than silent success.

## 3. Quick Example (Quick Example)

This is the standard construction path from the repository examples: create a model, optionally register tools, then construct a `ReActAgent`.

<!-- source: examples/common.rs:L327-L356 -->
```rust
use agent_scope_agent::{AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_tool::ToolKit;

let system_prompt = concat!(
    "You are a helpful AI assistant. ",
    "When the user asks a mathematical question, use the 'calculator' tool."
);

let mut toolkit = ToolKit::new();
toolkit.register(create_calculator_tool());

let config = AgentConfig::builder()
    .name("assistant")
    .system_prompt(system_prompt)
    .model(model)
    .toolkit(toolkit)
    .build()?;

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![],
)?;
```

Send a user message and wait for the final reply:

```rust
use agent_scope_agent::Agent;
use agent_scope_message::factory::user_msg;

let reply = agent
    .reply(Some(vec![user_msg("user", "帮我计算 15 * 27 + 3")?]))
    .await?;

println!("{}", reply.get_text_content("\n").unwrap_or_default());
```

## 4. Usage Patterns (Usage Patterns)

### 4.1 Run the Built-in Terminal Agent

`examples/agent-demo` is the complete Agent showcase. It calls the real DashScope API by default and starts an interactive REPL:

```bash
cargo run --example agent_demo
cargo run --example agent_demo -- --model qwen-plus --show-events
cargo run --example agent_demo -- --prompt "Use calculator to compute 23 * (17 + 5)"
```

It demonstrates:

- constructing a real `DashScopeChatModel` from `.env`/`API_KEY`
- consuming and rendering `AgentEvent` values with `reply_stream()`
- registering tools such as `calculator`, `safe_time`, and `demo_knowledge_lookup`
- denying `dangerous_demo_action` through `PermissionContext`
- redacting API keys and `sk-...`-like secrets from terminal and JSON output

### 4.2 Non-Streaming Reply: `reply()`

`reply()` is suitable for CLI tasks, tests, and backend endpoints: callers only need the final message and do not need token-by-token rendering.

```rust
let input = vec![user_msg("user", "介绍一下 AgentScope Rust")?];
let output = agent.reply(Some(input)).await?;

for block in &output.content {
    println!("{block:?}");
}
```

Note: even if the underlying model defaults to streaming, `reply()` internally accumulates it into a complete `ChatResponse` and then returns the final `Msg`.

### 4.3 Streaming Reply: `reply_stream()`

`reply_stream()` is suitable for terminal UIs, WebSocket/SSE, trace recorders, and other scenarios that need real-time feedback.

```rust
use futures::StreamExt;

let mut stream = agent
    .reply_stream(Some(vec![user_msg("user", "一步步计算 (2+3)*4")?]))
    .await?;

while let Some(event) = stream.next().await {
    match event {
        AgentEvent::TextBlockDelta(e) => print!("{}", e.delta),
        AgentEvent::ToolCallStart(e) => eprintln!("calling tool: {}", e.tool_call_name),
        AgentEvent::ToolResultEnd(e) => eprintln!("tool result: {:?}", e.state),
        AgentEvent::ReplyEnd(e) => eprintln!("finished: {:?}", e.finished_reason),
        _ => {}
    }
}
```

Consumers should handle at least:

- `ReplyStart` / `ReplyEnd`: boundaries of one reply
- `ModelCallStart` / `ModelCallEnd`: model-call boundaries and token usage
- `TextBlock*` / `ThinkingBlock*`: text and reasoning content
- `ToolCall*` / `ToolResult*`: tool calls and tool results
- `UserInterrupt` / `ExceedMaxIters`: control-flow exceptions

For more complete event rendering, see `examples/agent-demo/render.rs`.

### 4.4 Observing Messages: `observe()`

`observe()` only appends messages to Agent context and does not trigger a model call. It is useful for injecting external events, user history, or another Agent's output into the current Agent.

```rust
agent
    .observe(Some(vec![user_msg("user", "我偏好简洁回答")?]))
    .await?;

// 稍后基于已有上下文继续回复
let reply = agent.reply(None).await?;
```

Calling `reply(None)` before any context exists returns `AgentError::NoContentToReply`.

### 4.5 Registering Tools and Letting the Model Call Them

The Agent does not implement tools itself. It exposes tool schemas through `ToolKit`, and executes tools after the model returns a `ToolCallBlock`.

```rust
let mut toolkit = ToolKit::new();
toolkit.register(create_calculator_tool());

let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .toolkit(toolkit)
    .build()?;
```

The tool-call lifecycle is typically:

```text
模型返回 ToolCallBlock
→ ToolCallStart / ToolCallDelta* / ToolCallEnd
→ PermissionEngine 检查
→ ToolKit.call_tool(...)
→ ToolResultStart / ToolResultTextDelta* / ToolResultEnd
→ 工具结果写回上下文，进入下一轮模型调用
```

For tool schemas, `FunctionTool`, and `ToolKit` details, see [Tool System](./tool.md).

### 4.6 Enhancing an Agent with Middleware

The last argument to `ReActAgent::new` is `Vec<Arc<dyn Middleware>>`. For example, injecting memory:

<!-- source: examples/common.rs:L363-L406 -->
```rust
use std::sync::Arc;
use agent_scope_agent::MemoryMiddleware;
use agent_scope_memory::{FileMemory, MemoryConfig};

let memory_config = MemoryConfig {
    memory_dir: "memory_data".into(),
    ..Default::default()
};
let memory = Arc::new(FileMemory::new(workdir, memory_config.clone(), None));
let middleware = Arc::new(MemoryMiddleware::new(memory, memory_config));

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![middleware],
)?;
```

When writing custom middleware, implement only the hooks you care about:

```rust
use agent_scope_agent::{AgentError, Middleware};
use agent_scope_message::Msg;
use agent_scope_model::ChatModel;
use std::sync::Arc;

struct AuditMiddleware;

#[async_trait::async_trait]
impl Middleware for AuditMiddleware {
    async fn pre_reply(
        &self,
        agent_name: &str,
        input: &mut Option<Vec<Msg>>,
        _model: &Arc<dyn ChatModel>,
    ) -> Result<(), AgentError> {
        tracing::info!(agent = agent_name, has_input = input.is_some(), "reply started");
        Ok(())
    }
}
```

### 4.7 Configuring Tool Permissions

In default mode, tool calls are allowed when no rule matches. If you run an Agent in a read-only exploration scenario, switch to `Explore` and explicitly allow safe tools:

```rust
use agent_scope_agent::{PermissionContext, PermissionMode, PermissionRule};

let mut permission = PermissionContext::new(PermissionMode::Explore);
permission.add_rule(PermissionRule::allow("calculator"));
permission.add_rule(PermissionRule::deny("shell*"));

let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .toolkit(toolkit)
    .permission_context(permission)
    .build()?;
```

`deny` rules take precedence over `allow`. `ask` rules are automatically converted to rejection in `DontAsk` mode.

### 4.8 Controlling Context Compression

Context compression is disabled by default. Enable it for long conversations or large tool-result scenarios:

```rust
let context_config = ContextConfig {
    enable: true,
    trigger_ratio: 0.8,
    reserve_ratio: 0.1,
    tool_result_limit: 4096,
    ..ContextConfig::default()
};

let agent = ReActAgent::new(config, ReActConfig::default(), context_config, vec![])?;
```

Compression happens before each model call. Whether it triggers is determined by `model.count_tokens(...)` and `model.context_size()`.

### 4.9 Interrupting an In-Progress Reply

`ReActAgent::interrupt()` can be called from any thread. A common use is when a UI receives Ctrl+C or the user clicks a stop button:

```rust
agent.interrupt();
```

After interruption:

- The active model call or stream consumption stops through `CancellationToken`
- The event stream receives `UserInterrupt`
- `ReplyEnd.finished_reason` is `Interrupted`
- The next `reply()` / `reply_stream()` automatically uses a new cancellation token and can continue normally

## 5. Error Handling (Errors)

`AgentError` is the unified Agent-layer error type:

| Error | Common Cause | Suggested Handling |
|-------|--------------|--------------------|
| `InvalidConfig` | Missing name/model, or invalid config value | Fail fast during construction |
| `NoContentToReply` | `reply(None)` with empty context | Pass user messages first or call `observe()` |
| `AlreadyStreaming` | An active `reply()` / `reply_stream()` already exists | Consume or drop the current stream before starting another reply |
| `ModelError` | Provider call failed | Inspect the source and handle authentication, rate limit, network, etc. by category |
| `ToolError` | Tool missing, invalid input, or execution failure | Check tool registration and model-generated JSON input |
| `PermissionDenied` | Permission rules rejected tool execution | Adjust `PermissionContext` or ask the user to authorize |
| `MaxItersExceeded` | ReAct loop exceeded `max_iters` | Increase the limit, improve the system prompt, or constrain tool loops |
| `CancellationError` | Reply was cancelled | Usually treat as normal control flow |
| `ContextCompressionFailed` | Compression model call failed | Disable compression or check model availability |

## 6. Relationship to Other Modules

```text
用户 / 应用
   │
   ▼
Agent trait ───────────────┐
   │                       │
   ▼                       │
ReActAgent                 │ observe/reply/reply_stream
   │
   ├─ AgentState           → 会话上下文、reply id、迭代状态
   ├─ ChatModel            → 模型调用与 token 估算
   ├─ ToolKit              → 工具 schema 与工具执行
   ├─ PermissionEngine     → 工具执行授权
   ├─ Middleware           → 记忆、RAG、审计、自定义扩展
   └─ AgentEvent           → 流式 UI 与 trace
```

The Agent module is designed to “orchestrate without coupling”: model providers, tool implementations, memory storage, and UI rendering evolve independently in their own crates; Agent depends only on their traits and data protocols.

## 7. Pitfalls (Pitfalls)

1. **`chat.rs` does not automatically read the API key from environment variables**: it only accepts `-k` / `--api-key`; even if `.env` contains `API_KEY`, pass it explicitly.
2. **Do not start two concurrent replies on the same Agent**: the second one gets `AlreadyStreaming`. For concurrency, create an independent Agent per session or wait for the current stream to finish.
3. **`reply(None)` requires existing context**: call `reply(Some(...))` or `observe(Some(...))` first.
4. **Streaming consumers must read to completion or explicitly drop the stream**: otherwise the Agent still considers a reply in progress.
5. **`ReActAgent::state()` is not suitable for direct use**: use `try_state()` to read state.
6. **Tool input is a model-generated JSON string**: the Tool layer parses it; clearer tool schemas reduce invalid model-generated input.
7. **Permissions are not a sandbox by default**: `Default` mode allows tool calls when no rule matches; use `PermissionMode::Explore` and configure allow rules for read-only scenarios.

## 8. See Also

- [Getting Started](../getting-started.md) — run the `chat` example from the command line
- [Model Abstraction](./model.md) — `ChatModel` and streaming/non-streaming model returns
- [DashScope Provider](./dashscope.md) — configuration for the current built-in provider
- [Tool System](./tool.md) — `FunctionTool` / `ToolKit` / tool-call lifecycle
- [Event & Streaming](./event-streaming.md) — `AgentEvent` and real-time UI rendering
- [Memory and Session Management](../getting-started.md#5-understand-the-code-in-ten-minutes) — extend Agent through `MemoryMiddleware`, `AgentState`, and session stores


## Built-in task planning tools

`ReActAgent` registers a set of built-in task planning tools by default, letting the model break down and track complex work during the reasoning-acting loop. This mirrors the Python AgentScope tools `TaskCreate` / `TaskList` / `TaskGet` / `TaskUpdate` — planning is built into the tool system, with no separate Planner component.

### Task model

Tasks live in the agent state (`AgentState.tasks_context`) and persist with the session. Each task has: a sequential numeric `id`, `subject`, `description`, a status (`pending` / `in_progress` / `completed`), an `owner`, bidirectional `blocks` / `blocked_by` references, arbitrary `metadata`, and a `created_at` timestamp. Tasks are not stored in the compressible conversation context, so they survive long conversations and context compression.

### Built-in tools

| Tool | Purpose |
|------|---------|
| `TaskCreate` | Create a task, auto-assigning the next sequential id, starting as `pending` |
| `TaskList` | List all tasks with status, owner and blocking relations |
| `TaskGet` | Fetch the full details of a single task by id |
| `TaskUpdate` | Update subject/description/owner/status/metadata, set up bidirectional dependencies (`add_blocks` / `add_blocked_by`), or permanently remove a task via `deleted` (cleaning up references) |

Input schemas, behavior and output text align verbatim with the Python reference, so tool results are directly diff-testable. Task tools are treated as safe internal state operations: permission checks always pass and no user approval is triggered.

### Task reminder injection

When unfinished tasks exist (`pending` or `in_progress`) and the current context shows neither task-tool activity nor a previous reminder (e.g. the tool calls were compressed away), the agent appends a hint block to the conversation context stating the number of in-progress and pending tasks and pointing the model at `TaskList`. This keeps the agent aware of unfinished work after context compression.

### Configuration

Enabled by default; to disable:

```rust
let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .task_tools_enabled(false)   // disables task tools and the reminder
    .build()?;
```

### Compatibility

- **Level**: L2 (core-behavior compatible — tool lifecycle, output text, reminder injection) + L3 (public-API semantics — `task_tools_enabled` config, task model and tool types).
- **Upstream baseline**: Python AgentScope commit `9d1026fa` (v2.0.5); tool names, input schemas and output text align verbatim.
- **Breaking change**: the standalone `Planner` component (from Feature 021) has been removed entirely; planning is now provided by the built-in task tools.


## Agent State Persistence

`ReActAgent` ships with out-of-the-box state persistence: after every reply, the agent's full state (dialogue context, summary, task list, permission/tool/middleware contexts) is automatically saved to local storage; after a process restart, the full history can be resumed by the same session id. The persistence semantics align with the Python AgentScope reference session storage (`StorageBase` / `SessionRecord`).

### Out of the Box: Default JSON File Storage

No extra configuration is needed to get automatic persistence — when no store is injected, the built-in `JsonFileSessionStore` is used, with one `{session_id}.json` file per session under the `sessions/` directory of the working directory:

```rust
let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .build()?;                       // default: auto_persist = true, JSON file backend
let agent = ReActAgent::build(config, ReActConfig::default(), ContextConfig::default(), vec![]).await?;
```

Three new optional `AgentConfig` fields (all backward compatible; leaving them unset behaves exactly as before):

| Field | Default | Description |
|-------|---------|-------------|
| `session_store` | `None` → default `JsonFileSessionStore` (`sessions/`) | Injected storage backend |
| `session_id` | `None` → fresh session id | Session id used to resume existing state |
| `auto_persist` | `true` | Auto-save after each reply; `false` ⇒ zero writes |

### Resuming a Session by Id

When `session_id` is set and the async constructor `ReActAgent::build` is used, existing state is loaded from storage at construction time:

- Session exists → the agent is built from the restored state and can continue answering on top of the full history;
- Session missing → a new session is created (no error, no crash);
- Load failure (corrupted file / I/O) → a typed `AgentError` is returned, never a silent degradation.

```rust
// After a process restart, rebuild the agent with the same session id
let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .session_id("my-session-1")      // resume this session from storage
    .build()?;
let agent = ReActAgent::build(config, ReActConfig::default(), ContextConfig::default(), vec![]).await?;
// agent already carries the full history — continue the conversation directly
```

> Note: the synchronous `ReActAgent::new` constructor stays backward compatible and does not asynchronously resume an existing session; use `ReActAgent::build` for session resume.

### Auto-Persist Timing and Failure Handling

- **Timing**: after every `reply()` / `reply_stream()` completes normally, and also when interrupted or cancelled, the latest state is saved.
- **Failure handling**: a failed save is reported via tracing (`warn`) but never breaks the already-produced reply result or an in-flight reasoning loop.
- **Concurrency safety**: saves on the same agent are serialized; writes are atomic (temp file + rename), so a crash never leaves a half-written file.
- **Toggle**: with `auto_persist(false)`, no writes occur even when a backend is configured — behavior identical to before persistence was introduced.

### Session Management

Persisted sessions support lightweight management: `list_ids()` lists all session ids; `list_meta()` returns metadata (created-at, last-active, message count, status) sorted by last-active descending, **without deserializing the full state**; `delete(id)` is idempotent (deleting a missing session is not an error).

```rust
let store = JsonFileSessionStore::new("./sessions");
let ids = store.list_ids().await?;
let metas = store.list_meta().await?;
store.delete("old-session").await?;   // idempotent
```

### Implementing a Custom Storage Backend

The `SessionStore` trait (in `agent_scope_state`) is the **single extension point** for custom backends (SQLite, MySQL, Redis, object storage, …). Implement it and agent state transparently goes through your backend, with behavior identical to the built-in JSON file backend — **no framework changes required**:

```rust
use agent_scope_state::{Session, SessionError, SessionImpl, SessionMeta, SessionStore};

#[async_trait::async_trait]
impl SessionStore for MySqliteStore {
    // save    —— upsert: saving to the same id overwrites
    async fn save(&self, session: &dyn Session) -> Result<(), SessionError> { /* ... */ }

    // load    —— missing returns SessionError::NotFound { session_id }
    async fn load(&self, id: &str) -> Result<SessionImpl, SessionError> { /* ... */ }

    // delete  —— idempotent: Ok(()) even if the id does not exist
    async fn delete(&self, id: &str) -> Result<(), SessionError> { /* ... */ }

    async fn list_ids(&self) -> Result<Vec<String>, SessionError> { /* ... */ }
    async fn list_meta(&self) -> Result<Vec<SessionMeta>, SessionError> { /* ... */ }
}

let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .session_store(Arc::new(MySqliteStore::new("app.db")))
    .session_id("s-1")
    .build()?;
```

**Semantic contract** (aligned with Python `StorageBase`):

| Method | Semantics |
|--------|-----------|
| `save` | upsert — saving to the same id overwrites, mirrors `upsert_session` / `update_session_state` |
| `load` | missing returns `SessionError::NotFound`, mirrors `get_session` |
| `delete` | idempotent, mirrors `delete_session` |
| `list_ids` | list all persisted session ids |
| `list_meta` | lightweight metadata sorted by `last_active` descending, no full-state load, mirrors `list_sessions` |

**Error contract**: every failure must return a typed `SessionError` (`NotFound` / `SerializationError` / `StorageError`) — never string matching; implementations must be `Send + Sync`.

**SQLite example** table (mirrors the `SessionRecordFile` fields of the JSON format):

```sql
CREATE TABLE sessions (
    session_id    TEXT PRIMARY KEY,
    status        TEXT NOT NULL,
    message_count INTEGER NOT NULL,
    created_at    TEXT NOT NULL,
    last_active   TEXT NOT NULL,
    state_json    TEXT NOT NULL          -- serde_json-serialized AgentState
);
```

- `save` → `INSERT ... ON CONFLICT(session_id) DO UPDATE SET ...` (upsert)
- `load` → `SELECT * FROM sessions WHERE session_id = ?`; no rows → `NotFound`
- `list_meta` → `SELECT session_id, status, message_count, created_at, last_active FROM sessions ORDER BY last_active DESC`

MySQL semantics are identical; upsert uses `INSERT ... ON DUPLICATE KEY UPDATE ...` and `state_json` uses `LONGTEXT` / `JSON`.

### Compatibility

- **Level**: L2 (core-behavior compatible — session save/resume semantics aligned with Python `StorageBase`) + L3 (public-API semantics — `JsonFileSessionStore`, `SessionStore` extension point, new `AgentConfig` fields). L1 byte-level protocol is out of scope.
- **Data protocol**: the persisted `AgentState` fields map one-to-one to the Python reference; all `AgentState` fields are `#[serde(default)]` and unknown fields are ignored, so older data files load compatibly and new fields never break old data.
- **Upstream baseline**: Python AgentScope v2.0.5 (commit `6698d98`), `app/storage/_base.py`, `app/storage/_model/_session.py`, `state/_state.py`.


