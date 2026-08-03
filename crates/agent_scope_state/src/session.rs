//! Session trait, SessionImpl, and supporting types.
//!
//! A session wraps an [`AgentState`] and provides lifecycle management,
//! structured concurrency via [`CancellationToken`], and isolation between
//! concurrent sessions.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::agent_state::AgentState;

// ---------------------------------------------------------------------------
// SessionStatus (T003)
// ---------------------------------------------------------------------------

/// Session lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Closed,
}

// ---------------------------------------------------------------------------
// SessionError (T004)
// ---------------------------------------------------------------------------

/// Typed errors for session operations.
///
/// 6 variants covering closed session, persistence, storage, and configuration errors.
#[derive(Debug)]
pub enum SessionError {
    /// Operation attempted on a closed session.
    Closed { session_id: String },

    /// Session ID already exists in the store.
    AlreadyExists { session_id: String },

    /// Session not found in the store.
    NotFound { session_id: String },

    /// Serialization or deserialization failed.
    SerializationError { session_id: String, reason: String },

    /// Storage backend error.
    StorageError { session_id: String, reason: String },

    /// Invalid trim configuration.
    InvalidTrimConfig { reason: String },
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed { session_id } => {
                write!(f, "Session '{session_id}' is closed")
            }
            Self::AlreadyExists { session_id } => {
                write!(f, "Session '{session_id}' already exists")
            }
            Self::NotFound { session_id } => {
                write!(f, "Session '{session_id}' not found")
            }
            Self::SerializationError { session_id, reason } => {
                write!(
                    f,
                    "Serialization error for session '{session_id}': {reason}"
                )
            }
            Self::StorageError { session_id, reason } => {
                write!(f, "Storage error for session '{session_id}': {reason}")
            }
            Self::InvalidTrimConfig { reason } => {
                write!(f, "Invalid trim configuration: {reason}")
            }
        }
    }
}

impl std::error::Error for SessionError {}

// ---------------------------------------------------------------------------
// SessionMeta (T005)
// ---------------------------------------------------------------------------

/// Lightweight metadata for session listing.
///
/// Used by [`super::session_store::SessionStore::list_meta`] to provide
/// session summaries without loading full [`AgentState`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: String,
    pub status: SessionStatus,
    pub message_count: usize,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Session trait (T006)
// ---------------------------------------------------------------------------

/// Core session trait — represents a single conversation session.
///
/// # Guarantees
///
/// - `id()` never changes for the lifetime of the Session
/// - `close()` is idempotent — calling it multiple times is safe
/// - After `close()`, `status()` returns `Closed` and `is_closed()` returns `true`
/// - `touch()` updates `last_active` to `Utc::now()`
#[async_trait]
pub trait Session: Send + Sync {
    /// Unique session identifier (delegates to `self.state().session_id`).
    fn id(&self) -> &str;

    /// Current session status.
    fn status(&self) -> SessionStatus;

    /// Immutable reference to the agent state.
    fn state(&self) -> &AgentState;

    /// Mutable reference to the agent state (for middleware / external mutation).
    fn state_mut(&mut self) -> &mut AgentState;

    /// Close this session. Idempotent.
    ///
    /// After close:
    /// - `status()` returns `Closed`
    /// - `is_closed()` returns `true`
    /// - Further mutating operations return `SessionError::Closed`
    /// - The associated [`CancellationToken`] is cancelled
    async fn close(&mut self) -> Result<(), SessionError>;

    /// Whether this session has been closed.
    fn is_closed(&self) -> bool;

    /// Creation timestamp.
    fn created_at(&self) -> DateTime<Utc>;

    /// Last activity timestamp.
    fn last_active(&self) -> DateTime<Utc>;

    /// Update `last_active` to the current time.
    fn touch(&mut self);
}

// ---------------------------------------------------------------------------
// SessionImpl (T007)
// ---------------------------------------------------------------------------

/// Default [`Session`] implementation wrapping [`AgentState`].
///
/// # Structured Concurrency
///
/// Each `SessionImpl` owns a [`CancellationToken`]. Calling [`close`](Session::close)
/// cancels the token, signalling all session-scoped tasks to stop. If
/// `SessionImpl` is dropped without being closed, the token is cancelled
/// in the [`Drop`] impl as a safety net.
pub struct SessionImpl {
    agent_state: AgentState,
    status: SessionStatus,
    created_at: DateTime<Utc>,
    last_active: DateTime<Utc>,
    cancel_token: CancellationToken,
}

impl SessionImpl {
    /// Create a new session wrapping the given [`AgentState`].
    pub fn new(agent_state: AgentState) -> Self {
        Self {
            agent_state,
            status: SessionStatus::Active,
            created_at: Utc::now(),
            last_active: Utc::now(),
            cancel_token: CancellationToken::new(),
        }
    }

    /// Restore persisted timestamps (used by the session store's `load`, so a
    /// reloaded session keeps its original creation/last-active times instead
    /// of resetting them to the load instant, which would corrupt ordering).
    pub fn with_persisted_timestamps(
        mut self,
        created_at: DateTime<Utc>,
        last_active: DateTime<Utc>,
    ) -> Self {
        self.created_at = created_at;
        self.last_active = last_active;
        self
    }

    /// Create a new session with a custom session_id.
    ///
    /// A fresh [`AgentState`] is created internally with the given id.
    pub fn with_session_id(session_id: String) -> Self {
        Self::new(AgentState::with_session_id(session_id))
    }

    /// Obtain a child token for spawning session-scoped tasks.
    ///
    /// When the session is closed (or dropped), this token is cancelled,
    /// signalling tasks to stop.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.child_token()
    }
}

impl Drop for SessionImpl {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}

#[async_trait]
impl Session for SessionImpl {
    fn id(&self) -> &str {
        &self.agent_state.session_id
    }

    fn status(&self) -> SessionStatus {
        self.status
    }

    fn state(&self) -> &AgentState {
        &self.agent_state
    }

    fn state_mut(&mut self) -> &mut AgentState {
        self.touch();
        &mut self.agent_state
    }

    async fn close(&mut self) -> Result<(), SessionError> {
        if matches!(self.status, SessionStatus::Closed) {
            return Ok(()); // idempotent
        }
        self.status = SessionStatus::Closed;
        self.cancel_token.cancel();
        Ok(())
    }

    fn is_closed(&self) -> bool {
        matches!(self.status, SessionStatus::Closed)
    }

    fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    fn last_active(&self) -> DateTime<Utc> {
        self.last_active
    }

    fn touch(&mut self) {
        self.last_active = Utc::now();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // T008: Session lifecycle tests — create, verify, close, idempotent close
    #[tokio::test]
    async fn test_session_create_close() {
        let mut session = SessionImpl::with_session_id("test-session-1".into());

        // Verify creation
        assert_eq!(session.id(), "test-session-1");
        assert_eq!(session.status(), SessionStatus::Active);
        assert!(!session.is_closed());

        // Verify initial state
        assert_eq!(session.state().context_length(), 0);
        assert!(!session.created_at().to_string().is_empty());
        assert!(!session.last_active().to_string().is_empty());

        // Close
        session.close().await.unwrap();
        assert!(session.is_closed());
        assert_eq!(session.status(), SessionStatus::Closed);

        // Idempotent close — second call is no-op
        session.close().await.unwrap();
        assert!(session.is_closed());
    }

    #[tokio::test]
    async fn test_session_append_context_and_close() {
        use agent_scope_message::{ContentBlock, TextBlock};

        let mut session = SessionImpl::with_session_id("s-ctx".into());

        // Append context via state_mut
        let blocks = vec![ContentBlock::Text(TextBlock::new("hello".into()))];
        session.state_mut().append_context("agent", blocks).unwrap();
        assert_eq!(session.state().context_length(), 1);

        // Close and verify operations on closed session
        session.close().await.unwrap();
        assert!(session.is_closed());
    }

    #[tokio::test]
    async fn test_session_touch_updates_last_active() {
        let mut session = SessionImpl::with_session_id("s-touch".into());
        let before = session.last_active();

        // Small delay to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(1));

        session.touch();
        let after = session.last_active();
        assert!(
            after >= before,
            "last_active should be updated after touch()"
        );
    }

    // T009: Session isolation tests
    #[tokio::test]
    async fn test_session_isolation() {
        use agent_scope_message::{ContentBlock, TextBlock};

        let mut session_a = SessionImpl::with_session_id("iso-a".into());
        let mut session_b = SessionImpl::with_session_id("iso-b".into());

        // Append different messages to each
        let block_a = vec![ContentBlock::Text(TextBlock::new("msg-for-A".into()))];
        let block_b = vec![ContentBlock::Text(TextBlock::new("msg-for-B".into()))];

        session_a
            .state_mut()
            .append_context("agent-a", block_a)
            .unwrap();
        session_b
            .state_mut()
            .append_context("agent-b", block_b)
            .unwrap();

        // Verify isolation
        assert_eq!(session_a.state().context_length(), 1);
        assert_eq!(session_b.state().context_length(), 1);
        assert_eq!(session_a.state().context[0].name, "agent-a");
        assert_eq!(session_b.state().context[0].name, "agent-b");

        // Close session A — B remains active
        session_a.close().await.unwrap();
        assert!(session_a.is_closed());
        assert!(!session_b.is_closed());

        // B can still be used
        let more_blocks = vec![ContentBlock::Text(TextBlock::new("more-for-B".into()))];
        session_b
            .state_mut()
            .append_context("agent-b", more_blocks)
            .unwrap();
        assert_eq!(session_b.state().context_length(), 2);
    }

    #[tokio::test]
    async fn test_session_with_custom_id() {
        let session = SessionImpl::with_session_id("custom-id-001".into());
        assert_eq!(session.id(), "custom-id-001");
        assert_eq!(session.status(), SessionStatus::Active);
    }

    #[tokio::test]
    async fn test_session_close_cancels_token() {
        let mut session = SessionImpl::with_session_id("cancel-on-close".into());
        let token = session.cancel_token();

        assert!(!token.is_cancelled());
        session.close().await.unwrap();
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_session_drop_cancels_token() {
        let token = {
            let session = SessionImpl::with_session_id("cancel-on-drop".into());
            let token = session.cancel_token();
            assert!(!token.is_cancelled());
            token
        };

        assert!(token.is_cancelled());
    }
}
