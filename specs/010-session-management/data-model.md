# Data Model: Session Management (Feature 010)

**Feature**: 010-session-management  
**Date**: 2026-07-30  
**Source**: [spec.md](./spec.md) | [research.md](./research.md)

## Entity Overview

```
┌──────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Session    │────>│   AgentState     │────>│   ReplyContext  │
│  (new trait) │     │  (existing)      │     │   (existing)    │
└──────┬───────┘     └────────┬─────────┘     └─────────────────┘
       │                      │
       │ stores               │ contains
       ▼                      ▼
┌──────────────┐     ┌──────────────────┐
│ SessionStore │     │  middle_context  │
│  (new trait) │     │  (existing)      │
└──────────────┘     └──────────────────┘
       │
       │ implements
       ▼
┌──────────────────────┐
│ InMemorySessionStore │
│   (new struct)       │
└──────────────────────┘
```

---

## Entity 1: Session (trait)

**Crate**: `agent_scope_state`  
**File**: `agent_scope_state/src/session.rs`

```rust
#[async_trait::async_trait]
pub trait Session: Send + Sync {
    /// Unique session identifier.
    fn id(&self) -> &str;

    /// Current session status.
    fn status(&self) -> SessionStatus;

    /// Immutable reference to the agent state.
    fn state(&self) -> &AgentState;

    /// Mutable reference to the agent state (for middleware/external mutation).
    fn state_mut(&mut self) -> &mut AgentState;

    /// Close this session. Idempotent.
    /// After close, further operations return SessionError::Closed.
    async fn close(&mut self) -> Result<(), SessionError>;

    /// Whether this session has been closed.
    fn is_closed(&self) -> bool;

    /// Creation timestamp.
    fn created_at(&self) -> chrono::DateTime<chrono::Utc>;

    /// Last activity timestamp.
    fn last_active(&self) -> chrono::DateTime<chrono::Utc>;

    /// Touch (update last_active timestamp).
    fn touch(&mut self);
}
```

### SessionStatus (enum)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Closed,
}
```

### SessionImpl (struct — default implementation)

```rust
pub struct SessionImpl {
    agent_state: AgentState,
    status: SessionStatus,
    created_at: chrono::DateTime<chrono::Utc>,
    last_active: chrono::DateTime<chrono::Utc>,
    cancel_token: CancellationToken,
}
```

**State transitions**:

```
  new() / load()
       │
       ▼
  ┌─────────┐   close()    ┌─────────┐
  │ Active  │──────────────>│ Closed  │
  └─────────┘               └─────────┘
       │                         │
       │ close() (idempotent)    │ All operations return
       └─────────────────────────│ SessionError::Closed
                                 │
                                 │ close() is no-op
```

**Validation rules**:
- `id()` MUST return `self.agent_state.session_id`
- `close()` on already-closed session is a no-op (idempotent), NOT an error
- After `close()`, `is_closed()` returns `true` and all mutating operations return `SessionError::Closed`
- `CancellationToken` MUST be cancelled when `close()` is called
- `touch()` updates `last_active` to `Utc::now()`

**Relationships**:
- 1:1 with `AgentState` — each Session wraps exactly one AgentState
- 1:1 with `CancellationToken` — one token per session for task cancellation
- Stored by `SessionStore`

---

## Entity 2: SessionStore (trait)

**Crate**: `agent_scope_state`  
**File**: `agent_scope_state/src/session_store.rs`

```rust
#[async_trait::async_trait]
pub trait SessionStore: Send + Sync {
    /// Persist a session's full state.
    async fn save(&self, session: &dyn Session) -> Result<(), SessionError>;

    /// Load a session from storage by ID.
    async fn load(&self, id: &str) -> Result<SessionImpl, SessionError>;

    /// Delete a persisted session.
    async fn delete(&self, id: &str) -> Result<(), SessionError>;

    /// List all persisted session IDs.
    async fn list_ids(&self) -> Result<Vec<String>, SessionError>;

    /// List session metadata (lightweight, no full state load).
    async fn list_meta(&self) -> Result<Vec<SessionMeta>, SessionError>;
}
```

### InMemorySessionStore (struct)

```rust
pub struct InMemorySessionStore {
    sessions: RwLock<HashMap<String, String>>, // id → JSON string
    meta: RwLock<HashMap<String, SessionMeta>>,
}
```

**Validation rules**:
- `save()` upserts — overwrites existing ID
- `load()` returns `SessionError::NotFound` for unknown IDs
- `delete()` is idempotent — deleting nonexistent ID is a no-op
- `list_meta()` returns metadata sorted by `last_active` descending

---

## Entity 3: SessionMeta

**Crate**: `agent_scope_state`  
**File**: `agent_scope_state/src/session.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: String,
    pub status: SessionStatus,
    pub message_count: usize,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_active: chrono::DateTime<chrono::Utc>,
}
```

**Purpose**: Lightweight summary for session listing — avoids loading full `AgentState` (which includes all message history).

**Validation rules**:
- `message_count` equals `session.state().context_length()` at time of generation
- `status` reflects current `session.status()`

---

## Entity 4: TrimStrategy

**Crate**: `agent_scope_state`  
**File**: `agent_scope_state/src/trim.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrimStrategy {
    /// Maximum number of messages before trimming.
    pub max_messages: Option<usize>,

    /// Maximum estimated token count before trimming.
    pub max_tokens: Option<usize>,

    /// Number of recent messages to always keep.
    pub keep_recent: usize,

    /// Whether to preserve system messages at the start.
    pub keep_system_messages: bool,
}
```

**Default**:

```rust
impl Default for TrimStrategy {
    fn default() -> Self {
        Self {
            max_messages: None,
            max_tokens: None,
            keep_recent: 20,
            keep_system_messages: true,
        }
    }
}
```

**Trimming algorithm contract**:
1. If `max_messages` is set and `context.len() > max_messages`, trim
2. If `max_tokens` is set and `count_tokens(context) > max_tokens`, trim
3. Walk from newest to oldest, keep `keep_recent` messages
4. Ensure tool call/tool result pairs stay together (never split)
5. If `keep_system_messages`, preserve all System-role messages at context start
6. Move trimmed message content to `summary` field as text
7. Emit `SessionTrimmedEvent` with before/after counts

**Validation rules**:
- `keep_recent` MUST be > 0 (at least 1 message kept)
- Trimming MUST NOT orphan tool calls from their results
- After trimming, `context.len() < max_messages` or `count_tokens(context) < max_tokens`

---

## Entity 5: SessionError

**Crate**: `agent_scope_state`  
**File**: `agent_scope_state/src/session.rs`

```rust
#[derive(Debug)]
pub enum SessionError {
    /// Session is closed and cannot accept operations.
    Closed { session_id: String },

    /// Session ID already exists in store.
    AlreadyExists { session_id: String },

    /// Session not found in store.
    NotFound { session_id: String },

    /// Serialization or deserialization failure.
    SerializationError { session_id: String, reason: String },

    /// IO/storage backend error.
    StorageError { session_id: String, reason: String },

    /// Trim operation attempted with invalid configuration.
    InvalidTrimConfig { reason: String },
}
```

**Error types per Constitutional §13**:
- `Closed` → maps to `SessionError` category
- `AlreadyExists`/`NotFound` → maps to `SessionError` category
- `SerializationError` → maps to `SerializationError` category
- `StorageError` → wraps backend failures
- `InvalidTrimConfig` → maps to `ValidationError` category

---

## Entity 6: Session Events

**Crate**: `agent_scope_event`  
**New file**: `agent_scope_event/src/session_events.rs`

### Event structs

```rust
pub struct SessionCreatedEvent {
    pub base: EventBase,
    pub session_id: String,
}

pub struct SessionClosedEvent {
    pub base: EventBase,
    pub session_id: String,
    pub reason: String,  // "explicit_close" | "drop" | "error"
}

pub struct SessionSavedEvent {
    pub base: EventBase,
    pub session_id: String,
    pub message_count: usize,
}

pub struct SessionLoadedEvent {
    pub base: EventBase,
    pub session_id: String,
    pub message_count: usize,
}

pub struct SessionTrimmedEvent {
    pub base: EventBase,
    pub session_id: String,
    pub messages_before: usize,
    pub messages_after: usize,
    pub tokens_before: Option<usize>,
    pub tokens_after: Option<usize>,
}
```

### EventType additions

```rust
pub enum EventType {
    // ... existing 28 variants ...
    SessionCreated,    // NEW
    SessionClosed,     // NEW
    SessionSaved,      // NEW
    SessionLoaded,     // NEW
    SessionTrimmed,    // NEW
}
```

### AgentEvent additions

```rust
pub enum AgentEvent {
    // ... existing variants ...
    SessionCreated(SessionCreatedEvent),    // NEW
    SessionClosed(SessionClosedEvent),      // NEW
    SessionSaved(SessionSavedEvent),        // NEW
    SessionLoaded(SessionLoadedEvent),      // NEW
    SessionTrimmed(SessionTrimmedEvent),    // NEW
}
```

---

## File Structure Changes

### `agent_scope_state/` (extended)

```
src/
├── lib.rs                # +pub mod session; +pub mod session_store; +pub mod trim;
├── agent_state.rs        # (unchanged)
├── permission.rs         # (unchanged)
├── task.rs               # (unchanged)
├── session.rs            # NEW: Session trait, SessionImpl, SessionMeta, SessionStatus, SessionError
├── session_store.rs      # NEW: SessionStore trait, InMemorySessionStore
└── trim.rs               # NEW: TrimStrategy, trim_context()
```

### `agent_scope_event/` (extended)

```
src/
├── lib.rs                # +pub mod session_events; +5 AgentEvent variants; +5 EventType variants
├── session_events.rs     # NEW: 5 session event structs
├── event_type.rs         # +5 EventType variants
└── ... (unchanged)
```

---

## Relationships Summary

| Entity | Crate | Status | Dependencies |
|--------|-------|--------|-------------|
| `Session` (trait) | `agent_scope_state` | NEW | `AgentState`, `SessionStatus`, `SessionError` |
| `SessionImpl` | `agent_scope_state` | NEW | `AgentState`, `CancellationToken` |
| `SessionStore` (trait) | `agent_scope_state` | NEW | `Session`, `SessionMeta`, `SessionError` |
| `InMemorySessionStore` | `agent_scope_state` | NEW | `SessionStore` |
| `SessionMeta` | `agent_scope_state` | NEW | `SessionStatus` |
| `SessionStatus` | `agent_scope_state` | NEW | (enum, no deps) |
| `SessionError` | `agent_scope_state` | NEW | (enum, no deps) |
| `TrimStrategy` | `agent_scope_state` | NEW | (config struct, no deps) |
| `trim_context()` | `agent_scope_state` | NEW | `AgentState`, `TrimStrategy`, `ChatModel` (optional) |
| `SessionCreatedEvent` | `agent_scope_event` | NEW | `EventBase` |
| `SessionClosedEvent` | `agent_scope_event` | NEW | `EventBase` |
| `SessionSavedEvent` | `agent_scope_event` | NEW | `EventBase` |
| `SessionLoadedEvent` | `agent_scope_event` | NEW | `EventBase` |
| `SessionTrimmedEvent` | `agent_scope_event` | NEW | `EventBase` |
