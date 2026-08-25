//! SessionStore trait and InMemorySessionStore implementation.
//!
//! Provides an abstraction for session persistence backends.

use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;

use super::session::{Session, SessionError, SessionImpl, SessionMeta};

// ---------------------------------------------------------------------------
// SessionStore trait (T010)
// ---------------------------------------------------------------------------

/// Abstraction for session persistence backends.
///
/// # Guarantees
///
/// - `save()` is an upsert — saving to an existing ID overwrites
/// - `load()` returns `SessionError::NotFound` if the ID doesn't exist
/// - `delete()` is idempotent — no error if the ID doesn't exist
/// - `list_meta()` returns results sorted by `last_active` descending
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Persist a session's full state. Upserts (overwrites existing).
    async fn save(&self, session: &dyn Session) -> Result<(), SessionError>;

    /// Load a session by ID. Returns `NotFound` if missing.
    async fn load(&self, id: &str) -> Result<SessionImpl, SessionError>;

    /// Delete a persisted session. Idempotent.
    async fn delete(&self, id: &str) -> Result<(), SessionError>;

    /// List all persisted session IDs.
    async fn list_ids(&self) -> Result<Vec<String>, SessionError>;

    /// List session metadata (lightweight, no full state load).
    async fn list_meta(&self) -> Result<Vec<SessionMeta>, SessionError>;
}

/// Validate a session id as a safe storage key.
///
/// File-backed stores must be able to use ids as file-name components, so the
/// shared rule rejects empty ids plus path separators and `.`. Database-backed
/// stores reuse the same rule to keep session ids portable across backends.
pub(crate) fn validate_session_id(id: &str) -> Result<(), SessionError> {
    if id.is_empty() {
        return Err(SessionError::StorageError {
            session_id: id.to_string(),
            reason: "session id must not be empty".to_string(),
        });
    }
    if id.contains('/') || id.contains('\\') || id.contains('.') {
        return Err(SessionError::StorageError {
            session_id: id.to_string(),
            reason: "session id contains an invalid character (path separator or '.')".to_string(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// InMemorySessionStore (T011)
// ---------------------------------------------------------------------------

/// In-memory session store for testing and single-process use.
///
/// Stores serialized [`AgentState`] JSON strings and [`SessionMeta`] records.
pub struct InMemorySessionStore {
    sessions: RwLock<HashMap<String, String>>,
    meta: RwLock<HashMap<String, SessionMeta>>,
}

impl InMemorySessionStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            meta: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn save(&self, session: &dyn Session) -> Result<(), SessionError> {
        let id = session.id().to_string();

        // Serialize AgentState to JSON
        let json = serde_json::to_string(session.state()).map_err(|e| {
            SessionError::SerializationError {
                session_id: id.clone(),
                reason: e.to_string(),
            }
        })?;

        // Build metadata
        let meta_entry = SessionMeta {
            session_id: id.clone(),
            status: session.status(),
            message_count: session.state().context_length(),
            created_at: session.created_at(),
            last_active: session.last_active(),
        };

        self.sessions.write().await.insert(id.clone(), json);
        self.meta.write().await.insert(id, meta_entry);

        Ok(())
    }

    async fn load(&self, id: &str) -> Result<SessionImpl, SessionError> {
        let sessions = self.sessions.read().await;
        let json = sessions.get(id).ok_or_else(|| SessionError::NotFound {
            session_id: id.to_string(),
        })?;

        let agent_state: crate::AgentState =
            serde_json::from_str(json).map_err(|e| SessionError::SerializationError {
                session_id: id.to_string(),
                reason: e.to_string(),
            })?;

        Ok(SessionImpl::new(agent_state))
    }

    async fn delete(&self, id: &str) -> Result<(), SessionError> {
        self.sessions.write().await.remove(id);
        self.meta.write().await.remove(id);
        Ok(())
    }

    async fn list_ids(&self) -> Result<Vec<String>, SessionError> {
        let ids: Vec<String> = self.sessions.read().await.keys().cloned().collect();
        Ok(ids)
    }

    async fn list_meta(&self) -> Result<Vec<SessionMeta>, SessionError> {
        let meta_map = self.meta.read().await;
        let mut metas: Vec<SessionMeta> = meta_map.values().cloned().collect();
        // Sort by last_active descending
        metas.sort_by_key(|b| std::cmp::Reverse(b.last_active));
        Ok(metas)
    }
}

// ---------------------------------------------------------------------------
// Tests (T012, T013, T018)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_message::{ContentBlock, Role, TextBlock};

    /// T012: Save/load/delete round-trip tests
    #[tokio::test]
    async fn test_session_save_load_delete() {
        let store = InMemorySessionStore::new();

        // Create session with messages
        let mut session = SessionImpl::with_session_id("s1".into());
        for i in 0..5 {
            let blocks = vec![ContentBlock::Text(TextBlock::new(format!("msg-{i}")))];
            session.state_mut().append_context("agent", blocks).unwrap();
        }
        assert_eq!(session.state().context_length(), 5);

        // Save
        store.save(&session).await.unwrap();

        // Load
        let restored = store.load("s1").await.unwrap();
        assert_eq!(restored.state().context_length(), 5);
        assert_eq!(restored.id(), "s1");

        // Verify message content
        for msg in restored.state().context.iter() {
            assert_eq!(msg.name, "agent");
            assert_eq!(msg.role, Role::Assistant);
            assert!(!msg.content.is_empty());
        }

        // Delete
        store.delete("s1").await.unwrap();

        // Delete is idempotent
        store.delete("s1").await.unwrap();

        // Load after delete → NotFound
        let result = store.load("s1").await;
        assert!(result.is_err());
        if let Err(SessionError::NotFound { session_id }) = result {
            assert_eq!(session_id, "s1");
        } else {
            panic!("expected NotFound error");
        }
    }

    /// T012: Save/load preserves full state including reply context
    #[tokio::test]
    async fn test_save_load_preserves_reply_context() {
        let store = InMemorySessionStore::new();

        let mut session = SessionImpl::with_session_id("s-reply".into());
        session.state_mut().reply_context.cur_iter = 5;
        session.state_mut().reply_context.reply_id = "reply-001".into();

        store.save(&session).await.unwrap();

        let restored = store.load("s-reply").await.unwrap();
        assert_eq!(restored.state().reply_context.cur_iter, 5);
        assert_eq!(restored.state().reply_context.reply_id, "reply-001");
    }

    /// T013: List IDs and metadata
    #[tokio::test]
    async fn test_list_ids_and_meta() {
        let store = InMemorySessionStore::new();

        // Save 3 sessions
        for id in &["s-a", "s-b", "s-c"] {
            let mut session = SessionImpl::with_session_id(id.to_string());
            if *id == "s-b" {
                // s-b has more messages
                for _ in 0..3 {
                    let blocks = vec![ContentBlock::Text(TextBlock::new("msg".into()))];
                    session.state_mut().append_context("agent", blocks).unwrap();
                }
            }
            store.save(&session).await.unwrap();
        }

        // List IDs
        let ids = store.list_ids().await.unwrap();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"s-a".to_string()));
        assert!(ids.contains(&"s-b".to_string()));
        assert!(ids.contains(&"s-c".to_string()));

        // List meta — should be sorted by last_active descending
        let metas = store.list_meta().await.unwrap();
        assert_eq!(metas.len(), 3);
        // Verify sorting
        for i in 1..metas.len() {
            assert!(
                metas[i - 1].last_active >= metas[i].last_active,
                "Metadata should be sorted by last_active descending"
            );
        }

        // s-b has 3 messages
        let meta_b: Vec<_> = metas.iter().filter(|m| m.session_id == "s-b").collect();
        assert_eq!(meta_b.len(), 1);
        assert_eq!(meta_b[0].message_count, 3);
    }

    /// T018: Middleware context persistence test (US4)
    #[tokio::test]
    async fn test_middle_context_persistence() {
        let store = InMemorySessionStore::new();

        let mut session = SessionImpl::with_session_id("s-mc".into());

        // Write to middle_context
        let mut mc = std::collections::HashMap::new();
        mc.insert(
            "memory_query_result".to_string(),
            serde_json::Value::String("found relevant memories".to_string()),
        );
        mc.insert("last_index_tokens".to_string(), serde_json::json!(1500));
        session.state_mut().middle_context = mc;

        // Save and reload
        store.save(&session).await.unwrap();
        let restored = store.load("s-mc").await.unwrap();

        // Verify middle_context is preserved
        let mc = &restored.state().middle_context;
        assert_eq!(
            mc.get("memory_query_result").and_then(|v| v.as_str()),
            Some("found relevant memories")
        );
        assert_eq!(
            mc.get("last_index_tokens").and_then(|v| v.as_i64()),
            Some(1500)
        );
    }

    #[tokio::test]
    async fn test_session_full_lifecycle() {
        let store = InMemorySessionStore::new();
        let mut session = SessionImpl::with_session_id("s-full".into());
        let token = session.cancel_token();

        for i in 0..3 {
            let blocks = vec![ContentBlock::Text(TextBlock::new(format!("msg-{i}")))];
            session.state_mut().append_context("agent", blocks).unwrap();
        }
        session.state_mut().middle_context.insert(
            "phase".to_string(),
            serde_json::Value::String("full-lifecycle".to_string()),
        );

        store.save(&session).await.unwrap();
        let restored = store.load("s-full").await.unwrap();
        assert_eq!(restored.id(), "s-full");
        assert_eq!(restored.state().context_length(), 3);
        assert_eq!(
            restored
                .state()
                .middle_context
                .get("phase")
                .and_then(|v| v.as_str()),
            Some("full-lifecycle")
        );

        session.close().await.unwrap();
        assert!(session.is_closed());
        assert!(token.is_cancelled());

        store.delete("s-full").await.unwrap();
        assert!(matches!(
            store.load("s-full").await,
            Err(SessionError::NotFound { .. })
        ));
    }
}
