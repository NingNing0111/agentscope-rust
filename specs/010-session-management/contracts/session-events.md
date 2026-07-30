# Contract: Session Events

**Feature**: 010-session-management  
**Crate**: `agent_scope_event`  
**New file**: `src/session_events.rs`

## EventType Additions

Add to `agent_scope_event/src/event_type.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    // ... existing 28 variants ...
    SessionCreated,   // NEW — "SESSION_CREATED"
    SessionClosed,    // NEW — "SESSION_CLOSED"
    SessionSaved,     // NEW — "SESSION_SAVED"
    SessionLoaded,    // NEW — "SESSION_LOADED"
    SessionTrimmed,   // NEW — "SESSION_TRIMMED"
}
```

## Event Structs

New module `agent_scope_event/src/session_events.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::base::EventBase;

/// Emitted when a new session is created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreatedEvent {
    pub base: EventBase,
    pub session_id: String,
}

/// Emitted when a session is explicitly closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionClosedEvent {
    pub base: EventBase,
    pub session_id: String,
    /// Reason: "explicit_close" | "drop" | "error"
    pub reason: String,
}

/// Emitted when a session is persisted to a store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSavedEvent {
    pub base: EventBase,
    pub session_id: String,
    pub message_count: usize,
}

/// Emitted when a session is loaded from a store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLoadedEvent {
    pub base: EventBase,
    pub session_id: String,
    pub message_count: usize,
}

/// Emitted after context trimming is performed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTrimmedEvent {
    pub base: EventBase,
    pub session_id: String,
    pub messages_before: usize,
    pub messages_after: usize,
    pub tokens_before: Option<usize>,
    pub tokens_after: Option<usize>,
}
```

## AgentEvent Additions

Add to `agent_scope_event/src/lib.rs`:

```rust
pub use session_events::{
    SessionCreatedEvent, SessionClosedEvent, SessionSavedEvent,
    SessionLoadedEvent, SessionTrimmedEvent,
};

pub enum AgentEvent {
    // ... existing 27 variants ...
    #[serde(rename = "SESSION_CREATED")]
    SessionCreated(SessionCreatedEvent),
    #[serde(rename = "SESSION_CLOSED")]
    SessionClosed(SessionClosedEvent),
    #[serde(rename = "SESSION_SAVED")]
    SessionSaved(SessionSavedEvent),
    #[serde(rename = "SESSION_LOADED")]
    SessionLoaded(SessionLoadedEvent),
    #[serde(rename = "SESSION_TRIMMED")]
    SessionTrimmed(SessionTrimmedEvent),
}
```

## Serialization Format

All events use tagged JSON:

```json
{"type": "SESSION_CREATED", "base": {...}, "session_id": "s-001"}
{"type": "SESSION_CLOSED", "base": {...}, "session_id": "s-001", "reason": "explicit_close"}
{"type": "SESSION_SAVED", "base": {...}, "session_id": "s-001", "message_count": 42}
{"type": "SESSION_LOADED", "base": {...}, "session_id": "s-001", "message_count": 42}
{"type": "SESSION_TRIMMED", "base": {...}, "session_id": "s-001", "messages_before": 100, "messages_after": 50, "tokens_before": 5000, "tokens_after": 2400}
```

## Guarantees

- **G1**: All session state changes emit corresponding events (FR-020)
- **G2**: Events are tagged with `"type"` field for discrimination
- **G3**: Events carry `EventBase` with timestamp and sequence metadata
- **G4**: `session_id` is present in every session event
- **G5**: JSON round-trip is guaranteed (serialize → deserialize preserves equality)
