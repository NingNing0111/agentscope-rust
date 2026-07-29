# Data Model: Agent System

**Feature**: 007-agent-system | **Date**: 2026-07-29

## Entity Overview

```text
AgentConfig ──► ReActAgent ──► AgentState
                    │
            ┌───────┼───────┐
            │       │       │
        ChatModel  ToolKit  Middleware[]
                    │
              ReActConfig
              ContextConfig
```

## 1. AgentConfig

Constructor configuration for creating agents. Non-reactive (set once at construction).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | `String` | **required** | Agent identifier, used in messages and events |
| `system_prompt` | `String` | `""` | System prompt prepended to model context |
| `model` | `Arc<dyn ChatModel>` | **required** | Model for reasoning calls |
| `toolkit` | `Option<ToolKit>` | `None` | Registered tools for tool-calling |

**Validation rules**:
- `name` MUST NOT be empty
- `model` MUST NOT be null
- `system_prompt` MAY be empty (no-op)

**Serialization**: `#[derive(Deserialize)]` with `#[serde(default)]` on optional fields for forward compatibility.

## 2. ReActConfig

Loop behavior configuration for ReActAgent.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_iters` | `u32` | `20` | Maximum reasoning-acting iterations per reply |
| `stop_on_reject` | `bool` | `false` | Stop loop on permission denial (vs. waiting for confirmation) |
| `interruption_message` | `String` | `"The execution was interrupted."` | Message returned when interrupted |
| `structured_output_grace_iters` | `u32` | `3` | Extra iterations allowed when structured_output fails parse |

**Validation rules**:
- `max_iters` MUST be > 0
- `structured_output_grace_iters` MUST be > 0

**Serialization**: `#[derive(Deserialize)]`. Field names match Python AgentScope for config portability.

## 3. ContextConfig

Context window management configuration.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `trigger_ratio` | `f64` | `0.8` | Fraction of context_size that triggers compression |
| `reserve_ratio` | `f64` | `0.1` | Fraction of context_size reserved for model response |
| `compression_prompt` | `String` | `"<STD_CP_PROMPT>"` | System prompt for compression model call |
| `tool_result_limit` | `usize` | `4096` | Truncation limit for tool result content (characters) |

**Validation rules**:
- `0.0 < trigger_ratio < 1.0`
- `0.0 <= reserve_ratio < trigger_ratio`

**Serialization**: `#[derive(Deserialize)]`.

## 4. AgentError

Typed error enum for all agent operations.

| Variant | Fields | Description |
|---------|--------|-------------|
| `ValidationError` | `{ message: String }` | Invalid input or configuration |
| `ModelError` | `{ source: agent_scope_model::ModelError }` | Model call failure (wraps ModelError) |
| `ToolError` | `{ source: agent_scope_tool::ToolError }` | Tool execution failure |
| `TimeoutError` | `{ operation: String, duration: Duration }` | Operation timed out |
| `CancellationError` | `{ reply_id: String }` | Reply was cancelled/interrupted |
| `PermissionDenied` | `{ tool_name: String, reason: String }` | Tool execution rejected by permission engine |
| `ContextCompressionFailed` | `{ reason: String }` | Context compression model call failed |
| `NoContentToReply` | — | `reply(None)` called with empty state context |
| `MaxItersExceeded` | `{ max_iters: u32 }` | ReAct loop exceeded iteration limit |
| `InvalidConfig` | `{ field: String, message: String }` | Config validation failed at build time |

**Serialization**: `#[derive(Debug, thiserror::Error)]`. Implements `Display` for user-facing messages and source chain for debugging.

## 5. Agent trait

The core abstraction — common interface for all agent types.

```rust
#[async_trait]
pub trait Agent: Send + Sync {
    /// Send a reply to the given input and return the assistant's final Msg.
    async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError>;

    /// Stream reply events (including intermediate events and final Msg via ReplyEnd).
    async fn reply_stream(
        &self,
        input: Option<Vec<Msg>>,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>;

    /// Observe messages without triggering a reply.
    async fn observe(&self, input: Option<Vec<Msg>>) -> Result<(), AgentError>;

    /// The agent's name.
    fn name(&self) -> &str;

    /// Immutable reference to the agent's state.
    fn state(&self) -> &AgentState;
}
```

**Contract guarantees**:
- `reply(None)` with empty context returns `Err(NoContentToReply)`.
- `reply(Some(msgs))` appends msgs to context before reasoning.
- `observe(Some(msgs))` appends msgs to context, returns `Ok(())`.
- `observe(None)` is a no-op, returns `Ok(())`.
- `state()` always returns a valid reference (agent owns its state).

## 6. ReActAgent

The primary agent implementation.

```rust
pub struct ReActAgent {
    config: AgentConfig,
    react_config: ReActConfig,
    context_config: ContextConfig,
    state: RwLock<AgentState>,
    middlewares: Vec<Arc<dyn Middleware>>,
    event_emitter: EventEmitter,
    cancel_token: CancellationToken,
}
```

| Field | Type | Purpose |
|-------|------|---------|
| `config` | `AgentConfig` | Immutable construction parameters |
| `react_config` | `ReActConfig` | Loop behavior |
| `context_config` | `ContextConfig` | Context window management |
| `state` | `RwLock<AgentState>` | Mutable runtime state (message context, reply context, tools context) |
| `middlewares` | `Vec<Arc<dyn Middleware>>` | Registered hook interceptors |
| `event_emitter` | `EventEmitter` | Broadcast channel for event publishing |
| `cancel_token` | `CancellationToken` | External cancellation signal for interruption |

**Lifecycle**:
1. `ReActAgent::new(config, react_config, context_config, middlewares)` — validates config, initializes state, creates event channel.
2. `agent.reply(input)` → `pre_reply hook` → reasoning-acting loop → `post_reply hook` → return `Msg`.
3. Interruption: external code calls `agent.cancel()` → sets cancel_token → loop exits at next check point.

**State transitions (per reply)**:
```
Idle → ReplyStart → Reasoning → (Acting → Reasoning)* → ReplyEnd → Idle
```

## 7. Middleware trait

Extension hook interface.

```rust
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Called before reply starts. Can modify the input messages.
    async fn pre_reply(&self, _agent: &ReActAgent, _input: &mut Option<Vec<Msg>>) -> Result<(), AgentError> { Ok(()) }
    
    /// Called after reply completes (success or error).
    async fn post_reply(&self, _agent: &ReActAgent, _result: &Result<Msg, AgentError>) -> Result<(), AgentError> { Ok(()) }
    
    /// Called before reasoning (model call). Can modify messages/tools.
    async fn pre_reasoning(&self, _agent: &ReActAgent, _messages: &mut Vec<Msg>, _tools: &mut Option<Vec<JsonValue>>) -> Result<(), AgentError> { Ok(()) }
    
    /// Called after model returns response.
    async fn post_reasoning(&self, _agent: &ReActAgent, _response: &ChatResponse) -> Result<(), AgentError> { Ok(()) }
    
    /// Called before tool execution. Can modify or reject tool call.
    async fn pre_acting(&self, _agent: &ReActAgent, _tool_call: &mut ToolCallBlock) -> Result<(), AgentError> { Ok(()) }
    
    /// Called after tool execution completes.
    async fn post_acting(&self, _agent: &ReActAgent, _result: &ToolExecOutput) -> Result<(), AgentError> { Ok(()) }
    
    /// Called when observe() is invoked.
    async fn pre_observe(&self, _agent: &ReActAgent, _input: &mut Option<Vec<Msg>>) -> Result<(), AgentError> { Ok(()) }
    
    /// Called before print/output rendering.
    async fn pre_print(&self, _agent: &ReActAgent, _content: &mut String) -> Result<(), AgentError> { Ok(()) }
}
```

**Note**: The spec FR-016 defines 8 hook methods. The hook constants in `agent_scope_types::hook` define 10 constants (6 agent_hooks + 4 react_agent_hooks). The 8 middleware methods map to them as:
- `pre_reply` / `post_reply` → `PRE_REPLY` / `POST_REPLY`
- `pre_reasoning` / `post_reasoning` → `PRE_REASONING` / `POST_REASONING`
- `pre_acting` / `post_acting` → `PRE_ACTING` / `POST_ACTING`
- `pre_observe` → `PRE_OBSERVE` (post_observe → not in middleware trait, handled internally)
- `pre_print` → `PRE_PRINT` (post_print → not in middleware trait, handled internally)

The `post_observe` and `post_print` hooks are internal to the agent (no middleware interception needed — they're used for internal state management).

## 8. EventEmitter

Internal helper for event publishing.

```rust
pub(crate) struct EventEmitter {
    tx: tokio::sync::broadcast::Sender<AgentEvent>,
}
```

| Method | Description |
|--------|-------------|
| `new(capacity: usize)` | Create with bounded channel capacity |
| `emit(&self, event: impl Into<AgentEvent>)` | Publish event (non-blocking; drops if no receivers with lagged warning) |
| `subscribe(&self) -> broadcast::Receiver<AgentEvent>` | Create new subscriber |

## 9. PermissionEngine

Tool execution authorization.

```rust
pub struct PermissionEngine {
    rules: Vec<PermissionRule>,
}

pub struct PermissionRule {
    pub tool_pattern: String,     // glob or exact tool name
    pub allow: bool,
    pub require_confirm: bool,
}
```

**Methods**:
- `check(&self, tool_name: &str, input: &JsonValue) -> PermissionResult`
- `PermissionResult::Allow` — execute immediately
- `PermissionResult::Deny { reason }` — reject (if `stop_on_reject`, stop loop)
- `PermissionResult::RequireConfirm` — emit `RequireUserConfirmEvent`, wait for response

## Entity Relationship Summary

```
ReActAgent *──1 AgentConfig
ReActAgent *──1 ReActConfig
ReActAgent *──1 ContextConfig
ReActAgent 1──1 AgentState
ReActAgent 1──1 EventEmitter
ReActAgent *──* Middleware (ordered Vec)
ReActAgent 1──1 ChatModel (via AgentConfig)
ReActAgent 1──0..1 ToolKit (via AgentConfig)
ReActAgent 1──1 CancellationToken
AgentState  1──* Msg (context)
AgentState  1──1 ReplyContext
AgentState  1──1 PermissionContext
AgentState  1──1 ToolContext
PermissionEngine 1──* PermissionRule
```

## State Validation Rules

1. `AgentState::context` MUST only contain messages with valid `Role` values.
2. `ReplyContext::cur_iter` MUST be reset to 0 at the start of each `reply()` call.
3. `ReplyContext::reply_id` MUST be regenerated for each `reply()` call.
4. Tool calls in `AgentState::context` MUST transition `Asking → Submitted → (Success | Error)`.
5. `max_iters` check MUST happen at the top of each loop iteration before model call.
