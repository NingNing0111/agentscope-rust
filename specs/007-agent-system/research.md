# Research: Agent System

**Feature**: 007-agent-system | **Date**: 2026-07-29

## Research Tasks & Decisions

### 1. Agent trait design — `reply()` input type signature

**Decision**: Accept `Option<Vec<Msg>>` as the input parameter (not `Msg | Vec<Msg> | None`).

**Rationale**: 
- Python AgentScope's `reply()` accepts `Msg | list[Msg] | None` due to Python's dynamic typing.
- In Rust, a single `Option<Vec<Msg>>` captures all three cases: `None` (no input), `Some(vec![msg])` (single message), `Some(vec![msg1, msg2])` (multiple messages).
- Using an enum like `enum ReplyInput { Single(Msg), Many(Vec<Msg>), None }` would be closer to Python but adds unnecessary API complexity since callers can always wrap in `Some(vec![...])`.
- `Option<Vec<Msg>>` is idiomatic Rust and trivially constructed by callers.

**Alternatives considered**:
- `Into<Vec<Msg>>` generic — rejected because it makes the trait non-object-safe and prevents `Arc<dyn Agent>` usage.
- Separate `reply_single`, `reply_many`, `reply_none` methods — rejected as un-idiomatic and diverges from AgentScope's unified `reply()` interface.

---

### 2. Agent trait object safety

**Decision**: The `Agent` trait will NOT be object-safe by default. `reply()` returns `impl Future` which is not object-safe. Instead, a separate `DynAgent` trait (or using `#[async_trait]`) will provide object safety where needed.

**Rationale**:
- `async fn` in traits is stable in Rust 2024 edition but returning `impl Future` from trait methods prevents `dyn Agent`.
- `#[async_trait]` macro (from the `async-trait` crate) boxed futures enable `dyn Agent` while keeping the public API clean.
- The project already uses `#[async_trait::async_trait]` in `ChatModel` trait — consistency with existing patterns.

**Alternatives considered**:
- Manual `Pin<Box<dyn Future<Output = ...>>>` return types — more verbose, same semantics as `#[async_trait]`.
- Two-trait pattern (`Agent` + `DynAgent`) — adds complexity without benefit; `#[async_trait]` is simpler.

---

### 3. `reply_stream()` return type

**Decision**: Return `Pin<Box<dyn Stream<Item = AgentEvent> + Send>>` from `reply_stream()`.

**Rationale**:
- Must be object-safe (for `Arc<dyn Agent>` scenarios).
- `AgentEvent` items represent the full lifecycle trace (events + final message embedded in ReplyEnd).
- Using `Stream` (from `futures`) rather than tokio's `Receiver` keeps the API transport-agnostic.
- Model response content blocks are included in ModelCallEnd/ChatResponse for non-streaming; for streaming, TextBlockDelta events carry the content.

**Alternatives considered**:
- `tokio::sync::mpsc::Receiver<AgentEvent>` — rejected because it ties the API to tokio and adds backpressure complexity.
- `(Stream<AgentEvent>, oneshot::Receiver<Msg>)` pair — rejected as unnecessarily complex; final Msg can be extracted from ReplyEnd event.

---

### 4. Middleware trait design — all 8 hooks vs. builder pattern

**Decision**: Define a single `Middleware` trait with 8 optional hook methods, each defaulting to no-op. ReActAgent iterates registered middlewares in FIFO order per hook point.

**Rationale**:
- Python AgentScope uses a single `Middleware` interface with optionallow hooks.
- Trait with default no-op methods allows middleware authors to implement only the hooks they need.
- FIFO execution order matches Python AgentScope behavior.
- The 8 hooks map to the 10 hook constants defined in `agent_scope_types::hook`:
  - `agent_hooks`: pre_reply, post_reply, pre_print, post_print, pre_observe, post_observe
  - `react_agent_hooks` (inherits agent_hooks +): pre_reasoning, post_reasoning, pre_acting, post_acting

**Alternatives considered**:
- Separate traits per hook (e.g., `PreReplyHook`, `PostReplyHook`) — rejected; too many traits, registration complexity, harder to write middleware that intercepts multiple hooks.
- `HashMap<String, Box<dyn Fn>>` dynamic registration — rejected; loses type safety, harder to pass structured contexts.
- Builder/combinator pattern — rejected; adds API complexity for marginal ergonomics gain.

---

### 5. Context compression — when and how

**Decision**: 
- Check `context_size` from `ChatModel` before each `model.call()`.
- When estimated token count exceeds `context_size * trigger_ratio`, invoke compression.
- Compression: call model with a compression prompt to summarize the oldest messages, replace them with a `summary` content block in state.
- Preserve at least `reserve_ratio * context_size` tokens for new messages.

**Rationale**:
- The `ChatModel::count_tokens()` default (bytes/4 heuristic) provides a token estimate without requiring a full tokenizer.
- `trigger_ratio` (default 0.8) prevents waiting until the exact limit before compressing, giving headroom.
- `reserve_ratio` (default 0.1) ensures there's always room for the model's own response.
- Model-based compression (rather than rule-based truncation) preserves task-relevant information.

**Alternatives considered**:
- Sliding window truncation (keep last N messages) — rejected; loses important context from early conversation.
- Vector-based semantic compression — rejected; requires embedding infrastructure not yet available.
- Provider-specific tokenizer (tiktoken, etc.) — deferred to future feature; byte heuristic is sufficient for MVP.

---

### 6. PermissionEngine placement and implementation

**Decision**: Implement `PermissionEngine` in `agent_scope_agent/src/permission.rs`, replacing the placeholder in `agent_scope_state::permission`.

**Rationale**:
- The spec states: "`PermissionEngine` already exists in `agent_scope_state::permission` (defined as a stub) and will be fully implemented in this feature."
- Current `PermissionContext` is `HashMap<String, JsonValue>` — a true placeholder.
- Full `PermissionEngine` needs tool execution context (tool name, arguments, agent state) to make decisions.
- Implementing it in `agent_scope_agent` avoids a circular dependency (state crate shouldn't depend on tool crate).
- The `PermissionContext` type in `agent_scope_state` will be updated to the real implementation.

**Alternatives considered**:
- Implement in `agent_scope_state` — rejected; would need to import tool types or define generic interfaces, adding complexity.
- Separate permission crate — rejected; overkill for a single checking function used only by ReActAgent.

---

### 7. AgentEvent emission — channel vs. callback

**Decision**: Use `tokio::sync::broadcast` channel internally, wrapped in an `EventEmitter` helper. `reply_stream()` subscribes and yields events. `reply()` collects all events into a `Vec<AgentEvent>` for trace comparison before returning the final Msg.

**Rationale**:
- `broadcast` allows multiple subscribers (future: observability, logging). For now, only `reply_stream()` subscribes.
- `EventEmitter` provides a clean API (`emit(event)`) and handles channel lifecycle (closed when agent is dropped).
- Bounded channel with configurable capacity prevents memory issues from slow consumers.
- Collecting events for `reply()` enables trace verification (per Constitution Article 7).

**Alternatives considered**:
- `mpsc` channel — rejected; single-consumer prevents future multi-subscriber use cases.
- Callback/observer pattern — rejected; harder to integrate with async Stream return type.
- In-band return (events as part of return value) — rejected; cannot yield events progressively during streaming reply.

---

## Technology Choices Summary

| Choice | Decision | Rationale |
|--------|----------|-----------|
| Async trait support | `#[async_trait]` macro | Object safety for `Arc<dyn Agent>`; consistent with existing `ChatModel` |
| Stream type | `futures::Stream` | Transport-agnostic; works with all async runtimes |
| Error type | Enum `AgentError` with 10 variants | Typed errors per Constitution Article 13 |
| Config validation | Builder pattern with `build()` → `Result` | Catches invalid config at construction time, not runtime |
| Token counting | Default `bytes/4` heuristic + `ChatModel::count_tokens()` override | Simple, no external deps, provider-overridable |
| Event channel | `tokio::sync::broadcast` | Multi-subscriber; bounded capacity prevents OOM |
| Middleware storage | `Vec<Arc<dyn Middleware>>` on agent | Simple FIFO iteration; matches Python AgentScope |
