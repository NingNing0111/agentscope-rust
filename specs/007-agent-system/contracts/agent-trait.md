# Contract: Agent trait

**Feature**: 007-agent-system | **Trait**: `agent_scope_agent::Agent`

## Purpose

The `Agent` trait defines the common interface that all agent types MUST implement. It is the primary extension point for the agent system — new agent types (in future features) implement this trait to participate in the agent ecosystem.

## Interface

### Methods

```rust
#[async_trait]
pub trait Agent: Send + Sync {
    async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError>;
    async fn reply_stream(&self, input: Option<Vec<Msg>>) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>;
    async fn observe(&self, input: Option<Vec<Msg>>) -> Result<(), AgentError>;
    fn name(&self) -> &str;
    fn state(&self) -> &AgentState;
}
```

### `reply(input) → Result<Msg, AgentError>`

**Description**: Process input messages and return the agent's assistant response as a single `Msg`.

**Input contract**:

| Input | Behavior |
|-------|----------|
| `None` and context is empty | Returns `Err(AgentError::NoContentToReply)` |
| `None` and context has messages | Uses existing context, proceeds with reasoning |
| `Some(vec![msg])` | Appends the user message to context, then reasons |
| `Some(vec![msg1, msg2])` | Appends all messages to context, then reasons |

**Output contract**:
- Returns `Ok(Msg)` with `role = Assistant` containing the final text/tool result content blocks.
- Returns `Err(AgentError)` for any failure (model error, tool error, timeout, cancellation, max iterations).
- The returned `Msg` is also appended to `state().context`.

**Event sequence** (emitted via internal channel, observable through `reply_stream()`):
```
ReplyStart → (ModelCallStart → ModelCallEnd → [streaming blocks]) → ... → ReplyEnd
```

**Concurrency**: `reply()` is NOT reentrant. Calling `reply()` while another `reply()` is in progress is undefined behavior (implementation MAY return an error or queue).

### `reply_stream(input) → Result<Stream<AgentEvent>, AgentError>`

**Description**: Same as `reply()` but returns a `Stream` of `AgentEvent` items. This enables real-time monitoring of agent progress.

**Output contract**:
- Stream yields all intermediate events (ModelCallStart, TextBlockDelta, ToolCallStart, etc.).
- The final event is `AgentEvent::ReplyEnd` which carries `finished_reason` and optionally the final `Msg` content.
- Stream completes (`None`) after `ReplyEnd` is yielded.
- On error, the stream MAY yield events up to the point of failure, then the stream ends. The error is NOT propagated through the Stream but is available via the `ReplyEnd` event's `error` field.

### `observe(input) → Result<(), AgentError>`

**Description**: Append messages to the agent's context without triggering a reply.

**Input contract**:

| Input | Behavior |
|-------|----------|
| `None` | No-op, returns `Ok(())` |
| `Some(msgs)` | Appends all messages to `state.context` |

**Post-condition**: `state().context.len()` increases by `input.len()` (if `Some`).
**Hook**: Fires `pre_observe` middleware hook before appending.

### `name() → &str`

Always returns the agent's configured name. Never panics. Constant for the lifetime of the agent.

### `state() → &AgentState`

Returns a reference to the agent's current state. This is a snapshot — the state may change after the reference is obtained. For consistent reads, the caller should clone if needed.

## Implementor Responsibilities

Any type implementing `Agent` MUST:

1. **Event ordering**: Emit events in the order defined above. The exact sequence depends on the agent type (e.g., ReActAgent includes tool events).

2. **State consistency**: The returned `Msg` from `reply()` MUST be appended to `state.context` before returning.

3. **Error safety**: On error, the agent state MUST remain consistent. Partial model responses MUST NOT be appended to context.

4. **Thread safety**: All methods are `&self` (shared reference). Implementations MUST handle internal mutability (e.g., `RwLock<AgentState>`) correctly, ensuring no deadlocks.

5. **Hook dispatch**: Implementations with middleware support MUST invoke hooks in registration order at each defined hook point.

## Consumer Usage

```rust
use agent_scope_agent::{Agent, ReActAgent, AgentConfig, ReActConfig};
use agent_scope_message::factory::user_msg;

// Create agent
let agent = ReActAgent::new(
    AgentConfig::builder()
        .name("assistant")
        .model(my_model)
        .build()?,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![],
)?;

// Simple reply
let reply = agent.reply(Some(vec![user_msg("user", "Hello")?])).await?;
assert_eq!(reply.role, Role::Assistant);

// Observe without reply
agent.observe(Some(vec![system_msg("system", "You are helpful")?])).await?;

// Stream reply
let stream = agent.reply_stream(Some(vec![user_msg("user", "Hi")?])).await?;
// consume stream...
```

## Test Contract

Any mock `Agent` used in tests MUST:

1. Return deterministic results for `reply()`.
2. Not depend on live model API calls.
3. Record the sequence of `observe()` calls for verification.

## Compatibility

This trait maps to Python AgentScope's `AgentBase` class:
- `reply()` → `AgentBase.__call__()` / `AgentBase._reply()`
- `observe()` → `AgentBase.observe()`
- `name` → `AgentBase.name`
- `state` → internal `_state` attribute

Rust-specific adaptations: `Agent` is a trait (not a base class); `reply_stream()` is an explicit API method (Python uses generator/inspect flags).
