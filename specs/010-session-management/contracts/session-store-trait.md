# Contract: SessionStore Trait

**Feature**: 010-session-management  
**Crate**: `agent_scope_state`  
**File**: `src/session_store.rs`

## Interface

```rust
use async_trait::async_trait;

use super::session::{SessionError, SessionImpl, SessionMeta};

/// Abstraction for session persistence backends.
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Persist a session. Upserts (overwrites existing).
    async fn save(&self, session: &dyn super::session::Session) -> Result<(), SessionError>;

    /// Load a session by ID. Returns NotFound if missing.
    async fn load(&self, id: &str) -> Result<SessionImpl, SessionError>;

    /// Delete a persisted session. Idempotent.
    async fn delete(&self, id: &str) -> Result<(), SessionError>;

    /// List all persisted session IDs.
    async fn list_ids(&self) -> Result<Vec<String>, SessionError>;

    /// List session metadata (no full state load).
    async fn list_meta(&self) -> Result<Vec<SessionMeta>, SessionError>;
}
```

## Default Implementation

```rust
use std::collections::HashMap;
use tokio::sync::RwLock;

/// In-memory session store for testing and single-process use.
pub struct InMemorySessionStore {
    sessions: RwLock<HashMap<String, String>>,  // id → JSON
    meta: RwLock<HashMap<String, SessionMeta>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self;
}

impl SessionStore for InMemorySessionStore { /* ... */ }
```

## Usage Contract

### Save and Load round-trip
```rust
let store = InMemorySessionStore::new();
let session = SessionImpl::with_session_id("s1".into());

// Add some state
session.state_mut().append_context("agent", vec![block]).unwrap();

// Save
store.save(&session).await?;

// Load
let restored = store.load("s1").await?;
assert_eq!(restored.state().context_length(), 1);
assert_eq!(restored.state().context[0].name, "agent");
```

### Delete is idempotent
```rust
store.delete("s1").await?;
store.delete("s1").await?; // no-op, no error
assert!(store.load("s1").await.is_err()); // NotFound
```

### List metadata
```rust
let metas = store.list_meta().await?;
for m in metas {
    println!("{}: {} messages, last active: {}", m.session_id, m.message_count, m.last_active);
}
```

## Guarantees

- **G1**: `save()` is an upsert — saves to an existing ID overwrite
- **G2**: `load()` returns `SessionError::NotFound` if ID doesn't exist
- **G3**: `delete()` is idempotent — no error if ID doesn't exist
- **G4**: `list_meta()` returns results sorted by `last_active` descending
- **G5**: `InMemorySessionStore` is `Send + Sync` — safe for concurrent access
