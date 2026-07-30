# Research: Session Management (Feature 010)

**Feature**: 010-session-management  
**Date**: 2026-07-30  
**Status**: Complete

## R1: Crate Architecture — Extend vs New Crate

**Decision**: Extend `agent_scope_state` crate with Session trait and SessionStore trait. Do NOT create a new `agent_scope_session` crate.

**Rationale**:
- `agent_scope_state` already owns `AgentState` (the core data type for session state), `session_id` field, `middle_context`, and all serialization logic
- Creating a new crate would introduce a circular dependency: `agent_scope_session` → `agent_scope_state` for `AgentState`, and `agent_scope_agent` → `agent_scope_session` for Session operations
- The crate is already named `agent_scope_state`, which semantically encompasses Session management
- Constitution §11 (分层与依赖方向): adding a trait to the existing state crate maintains the current dependency graph without new edges

**Alternatives considered**:
- New `agent_scope_session` crate: Adds a new crate to the workspace, increases compilation units. Rejected because `Session` is too tightly coupled with `AgentState` to justify a separate crate.
- Put Session in `agent_scope_agent`: Creates unwanted coupling between session management and agent logic. Agent crate should consume sessions, not define them.

## R2: Session Trait Design — Async vs Sync

**Decision**: `Session` trait uses `async_trait` with async methods for `save()` and `load()`. Close and state-access methods are synchronous.

**Rationale**:
- Persistence operations (`save`, `load`) involve I/O → must be async per Constitution §10 (结构化并发)
- State query operations (`id()`, `state()`, `context_length()`) are in-memory reads → sync is fine
- Following the pattern established by `Agent` trait which uses `#[async_trait::async_trait]`
- Send + Sync bounds for trait object safety: `Arc<dyn Session>`

**Alternatives considered**:
- Fully sync with blocking I/O: Violates Constitution §10. Tokio runtime would be blocked.
- All methods async: Unnecessary complexity for simple in-memory reads.

## R3: SessionStore Trait Design

**Decision**: A separate `SessionStore` trait with methods `save(session: &dyn Session)`, `load(id: &str)`, `delete(id: &str)`, `list()`. Storage backend is injected via trait object.

**Rationale**:
- Follows the pattern of `ChatModel`, `Tool`, `Memory` — all use trait abstraction for pluggable backends
- Default `InMemorySessionStore` for testing (no external dependency)
- Future backends: file-based, Redis, database — all implement the same trait
- Separates "what is a session" (Session trait) from "where is it stored" (SessionStore trait)

**Alternatives considered**:
- Session trait with built-in save/load: Violates separation of concerns. Session becomes storage-aware.
- Generic `<S: Storage>` parameter: Introduces monomorphization complexity. Trait objects are simpler for this use case.

## R4: Session Events

**Decision**: Add 5 new `EventType` variants to `agent_scope_event`: `SessionCreated`, `SessionClosed`, `SessionSaved`, `SessionLoaded`, `SessionTrimmed`. Each has a corresponding event struct in a new `session_events.rs` module.

**Rationale**:
- Constitution §14 (可观测性): All critical operations MUST emit events
- Follows the existing pattern in `agent_scope_event` (per-category event modules: `reply_events.rs`, `tool_events.rs`, etc.)
- Session lifecycle is a distinct event domain, justifying its own module

**New event types**:

| Event Type | Struct | Key Fields |
|---|---|---|
| `SESSION_CREATED` | `SessionCreatedEvent` | session_id, timestamp |
| `SESSION_CLOSED` | `SessionClosedEvent` | session_id, reason |
| `SESSION_SAVED` | `SessionSavedEvent` | session_id, message_count |
| `SESSION_LOADED` | `SessionLoadedEvent` | session_id, message_count |
| `SESSION_TRIMMED` | `SessionTrimmedEvent` | session_id, messages_before, messages_after |

**Alternatives considered**:
- Reuse `CustomEvent`: Loses type safety and structured data. Violates FR-020.
- Put events in session crate: Breaks the existing event architecture where all events live in `agent_scope_event`.

## R5: Context Trimming Strategy

**Decision**: Implement `TrimStrategy` as a configuration struct with `max_messages` (count-based) and `max_tokens` (token-based) thresholds. Trimming is triggered when either threshold is exceeded. Strategy: keep system messages + last N user/assistant pairs with tool call chains intact.

**Rationale**:
- `AgentState` already has `max_context_messages` field and `AppendContextError::ContextFull` — trimming extends this pattern
- `context_compression` module in `agent_scope_agent` already provides `compress_context()` with token counting via `model.count_tokens()`
- Session-level trimming complements the existing per-reply compression
- Tool call chain integrity is critical — never orphan a tool result from its call

**Trimming algorithm**:
1. If `context.len() > max_messages` OR `count_tokens(context) > max_tokens`, trigger trim
2. Walk from the end (newest) backwards, accumulating messages
3. Stop when accumulated count reaches `keep_recent` threshold
4. Ensure tool_call/tool_result pairs stay together (atomic units)
5. Preserve system messages (role=System) at the start
6. Removed messages contribute to `summary` field

**Alternatives considered**:
- LLM-based summarization for trimmed messages: Deferred to future feature. Constitution §15 says performance must not sacrifice correctness — count-based trimming is simpler and more predictable.
- Ring-buffer approach: Violates tool call chain integrity requirement.

## R6: Session Shutdown — Structured Concurrency

**Decision**: Session implements `Drop` + explicit `close()` to cancel associated tasks. A `CancellationToken` per session propagates to spawned agent tasks. `close()` is idempotent.

**Rationale**:
- Constitution §10 (结构化并发): "Session 结束后仍持续运行的 session-scoped 任务" is forbidden
- Existing `CancellationToken` pattern from Feature 008 streaming infrastructure
- `Drop` as safety net, `close()` as explicit API — follows Rust idioms

**Alternatives considered**:
- Reference counting with `Arc<Session>`: Gets complicated with async tasks holding references. Explicit close + token is cleaner.
- No explicit close (GC-like): Non-deterministic shutdown timing violates FR-004 and SC-005.

## R7: Serialization Format

**Decision**: Use `serde_json` for `AgentState` serialization (already implemented). Session persistence serializes the full `AgentState` to JSON. Version field `"format_version": "1.0"` in the serialized output for future-compatibility.

**Rationale**:
- `AgentState` already derives `Serialize + Deserialize` with round-trip tests
- JSON is human-readable, debuggable, and the established format in this project
- Version field enables forward compatibility (Constitution §12)
- `AgentState::from_legacy_json()` already demonstrates migration capability

**Alternatives considered**:
- MessagePack/bincode: Faster but not human-readable. Debugging session state is important.
- Protocol Buffers: Over-engineering for single-process state. Schema maintenance overhead.

## R8: Middleware Context Lifecycle

**Decision**: `middle_context: HashMap<String, serde_json::Value>` is already on `AgentState`. No structural changes needed. Session ensures `middle_context` participates in save/load round-trips transparently through `AgentState` serialization.

**Rationale**:
- Feature 009 Memory System already uses `middle_context` via `MemoryMiddleware`
- The field is already `Serialize + Deserialize` — save/load works out of the box
- Session only needs to document that middleware data is automatically persisted with session state
- No new API surface needed for middleware context management

## R9: Python AgentScope Reference Compatibility

**Decision**: The Python `AgentState` class has `session_id`, `context`, `summary`, and serialization via `model_dump()`/`model_validate()`. Our Rust `AgentState` is already compatible at the data level. Session management (create/close/save/load) in Python is handled by the `ReActAgent` and `Agent` classes directly — there is no standalone `Session` class in Python.

**Implications**:
- `Session` trait is a Rust-native abstraction (per Constitution §8: Rust 原生设计) — it doesn't need to replicate Python's exact class hierarchy
- Session data format (AgentState JSON) MUST remain compatible with Python serialization format
- Session behavior (semantics of close, save, load) should match Python's observable behavior

**Compatibility target**: L2 (核心行为兼容) — Session data protocol is compatible, but the Session management API is Rust-native.

## Summary

| # | Topic | Decision | Impact |
|---|-------|----------|--------|
| R1 | Crate | Extend `agent_scope_state` | No new crate, 1 existing crate gets 2 new modules |
| R2 | Session trait | `async_trait` with mixed sync/async | Save/Load async, queries sync |
| R3 | SessionStore | Separate trait, pluggable backends | Default InMemory, extensible |
| R4 | Events | 5 new EventType variants + session_events.rs | New module in `agent_scope_event` |
| R5 | Trimming | Count+Token thresholds, tool chain integrity | Config-driven, complements compression |
| R6 | Shutdown | CancellationToken per session + Drop | FR-004, SC-005 compliance |
| R7 | Serialization | serde_json with version field | Already supported by AgentState |
| R8 | Middleware Ctx | No changes needed | Auto-included in AgentState serde |
| R9 | Python compat | L2 target, data format compatible | Session trait is Rust-native |
