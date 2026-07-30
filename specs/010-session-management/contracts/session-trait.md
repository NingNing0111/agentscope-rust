# Contract: Session Trait

**Feature**: 010-session-management  
**Crate**: `agent_scope_state`  
**File**: `src/session.rs`

## Interface

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::agent_state::AgentState;

/// Session status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Closed,
}

/// Typed errors for session operations.
#[derive(Debug)]
pub enum SessionError {
    Closed { session_id: String },
    AlreadyExists { session_id: String },
    NotFound { session_id: String },
    SerializationError { session_id: String, reason: String },
    StorageError { session_id: String, reason: String },
    InvalidTrimConfig { reason: String },
}

impl std::fmt::Display for SessionError { /* ... */ }
impl std::error::Error for SessionError {}

/// Lightweight metadata for session listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: String,
    pub status: SessionStatus,
    pub message_count: usize,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

/// Core session trait — represents a single conversation session.
#[async_trait]
pub trait Session: Send + Sync {
    fn id(&self) -> &str;
    fn status(&self) -> SessionStatus;
    fn state(&self) -> &AgentState;
    fn state_mut(&mut self) -> &mut AgentState;
    async fn close(&mut self) -> Result<(), SessionError>;
    fn is_closed(&self) -> bool;
    fn created_at(&self) -> DateTime<Utc>;
    fn last_active(&self) -> DateTime<Utc>;
    fn touch(&mut self);
}
```

## Default Implementation

```rust
/// Default Session implementation wrapping AgentState.
pub struct SessionImpl {
    agent_state: AgentState,
    status: SessionStatus,
    created_at: DateTime<Utc>,
    last_active: DateTime<Utc>,
    cancel_token: CancellationToken,
}

impl SessionImpl {
    pub fn new(agent_state: AgentState) -> Self;
    pub fn with_session_id(session_id: String) -> Self;
    pub fn cancel_token(&self) -> CancellationToken;
}

impl Session for SessionImpl { /* ... */ }
```

## Usage Contract

### Creation
```rust
let session = SessionImpl::with_session_id("user-123-session-1".into());
assert_eq!(session.id(), "user-123-session-1");
assert_eq!(session.status(), SessionStatus::Active);
assert!(!session.is_closed());
```

### Close (idempotent)
```rust
session.close().await?;
assert!(session.is_closed());
session.close().await?; // no-op, no error
```

### Closed session — operations rejected
```rust
// After close:
session.state_mut().append_context("agent", blocks)
// → panics or returns error (SessionError::Closed)
```

### Resource cleanup
```rust
// When SessionImpl is dropped, cancel_token is cancelled automatically.
// All spawned tasks monitoring this token will stop.
```

## Guarantees

- **G1**: `id()` never changes for the lifetime of the Session
- **G2**: `close()` is idempotent — calling it multiple times is safe
- **G3**: After `close()`, `status()` returns `Closed` and `is_closed()` returns `true`
- **G4**: `touch()` updates `last_active` to `Utc::now()`
- **G5**: `cancel_token()` is cancelled on `close()` and on `Drop`
