//! Integration tests for ReActAgent persistence (Feature 025).
//!
//! Covers the agent-side integration of `SessionStore`:
//!
//! - Cross-process resume by session_id (quickstart 场景 2)
//! - Automatic save after reply / interruption (quickstart 场景 3 / 4)
//! - auto_persist = false ⇒ zero writes (quickstart 场景 5)
//! - Custom SessionStore backend injection (quickstart 场景 7)
//!
//! Uses `tests/mocks.rs` (MockModel / ScriptedModel) — no real LLM required
//! (constitution article 6).

mod mocks;

use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, RwLock};

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_message::Role;
use agent_scope_message::factory::user_msg;
use agent_scope_state::{
    AgentState, JsonFileSessionStore, Session, SessionError, SessionImpl, SessionMeta,
    SessionStatus, SessionStore,
};
use mocks::MockModel;
use tempfile::TempDir;

/// Build an agent with a `JsonFileSessionStore` rooted at `dir` and a
/// `session_id`, resuming any existing state.
async fn build_persisted_agent(
    model: Arc<MockModel>,
    dir: &std::path::Path,
    session_id: &str,
) -> ReActAgent {
    let store = JsonFileSessionStore::new(dir);
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .session_store(Arc::new(store))
        .session_id(session_id)
        .build()
        .unwrap();
    ReActAgent::build(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// T010 — cross-process resume by session_id (quickstart 场景 2)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_resume_after_rebuild() {
    let tmp = TempDir::new().unwrap();
    let model = Arc::new(MockModel::new("mock", "resumed reply"));

    // First process: reply once, state is auto-persisted under "s-1".
    let agent = build_persisted_agent(model.clone(), tmp.path(), "s-1").await;
    agent
        .reply(Some(vec![user_msg("user", "first message").unwrap()]))
        .await
        .unwrap();
    let first_len = agent.try_state().context.len();
    assert!(
        first_len >= 2,
        "reply should append user + assistant to context"
    );
    drop(agent); // simulate process restart

    // Second process: resume by the same session id.
    let agent2 = build_persisted_agent(model, tmp.path(), "s-1").await;
    let resumed_len = agent2.try_state().context.len();
    assert!(
        resumed_len >= first_len,
        "resumed session must carry the full history (resumed={resumed_len}, first={first_len})"
    );

    // Continue answering based on the restored history.
    let reply = agent2
        .reply(Some(vec![user_msg("user", "second message").unwrap()]))
        .await
        .unwrap();
    assert_eq!(reply.role, Role::Assistant);
    assert!(
        agent2.try_state().context.len() > resumed_len,
        "second reply must extend the resumed context"
    );
}

/// Loading a non-existent session id creates a fresh session, not an error.
#[tokio::test]
async fn test_unknown_session_id_creates_new_session() {
    let tmp = TempDir::new().unwrap();
    let model = Arc::new(MockModel::new("mock", "hello"));

    let agent = build_persisted_agent(model, tmp.path(), "never-existed").await;
    assert_eq!(agent.try_state().session_id, "never-existed");
    assert_eq!(agent.try_state().context.len(), 0);
}

// ---------------------------------------------------------------------------
// T011 — automatic save after reply (quickstart 场景 3)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_auto_persist_after_reply() {
    let tmp = TempDir::new().unwrap();
    let store = JsonFileSessionStore::new(tmp.path());
    let model = Arc::new(MockModel::new("mock", "persisted reply"));

    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .session_store(Arc::new(store.clone()))
        .session_id("s-auto")
        .build()
        .unwrap();
    let agent = ReActAgent::build(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .await
    .unwrap();

    agent
        .reply(Some(vec![user_msg("user", "hello").unwrap()]))
        .await
        .unwrap();

    // Batch replies persist synchronously — the state is on disk already.
    let loaded = store.load("s-auto").await;
    assert!(
        matches!(loaded, Ok(session) if session.state().context_length() >= 2),
        "state after reply must be persisted and contain the round's context"
    );
}

// ---------------------------------------------------------------------------
// T012 — save on interruption / cancellation (quickstart 场景 4)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_persist_on_interruption() {
    let tmp = TempDir::new().unwrap();
    let store = JsonFileSessionStore::new(tmp.path());
    let model = Arc::new(MockModel::new("mock", "should not appear"));

    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .session_store(Arc::new(store.clone()))
        .session_id("s-intr")
        .build()
        .unwrap();
    let agent = ReActAgent::build(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .await
    .unwrap();

    // First, produce some history with a normal reply.
    agent
        .reply(Some(vec![user_msg("user", "first round").unwrap()]))
        .await
        .unwrap();

    // Interrupt the next reply — it ends immediately, but the latest state
    // (including the first round's history) is still persisted.
    agent.interrupt();
    let reply = agent
        .reply(Some(vec![user_msg("user", "interrupt me").unwrap()]))
        .await
        .unwrap();
    assert!(
        reply.get_text_content("").unwrap().contains("interrupted"),
        "interrupted reply should return the interruption message"
    );

    let loaded = store.load("s-intr").await;
    assert!(
        matches!(loaded, Ok(session) if session.state().context_length() >= 2),
        "state at the interruption moment (with prior history) must be persisted"
    );
}

// ---------------------------------------------------------------------------
// T022 — auto_persist = false ⇒ zero writes (quickstart 场景 5)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_auto_persist_disabled_zero_writes() {
    let tmp = TempDir::new().unwrap();
    let store = JsonFileSessionStore::new(tmp.path());
    let model = Arc::new(MockModel::new("mock", "no write"));

    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .session_store(Arc::new(store))
        .session_id("s-noauto")
        .auto_persist(false)
        .build()
        .unwrap();
    let agent = ReActAgent::build(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .await
    .unwrap();

    for i in 0..3 {
        agent
            .reply(Some(vec![user_msg("user", &format!("msg {i}")).unwrap()]))
            .await
            .unwrap();
    }

    let entries: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        entries,
        Vec::<String>::new(),
        "no session files may be created when auto_persist is disabled"
    );
}

// ---------------------------------------------------------------------------
// T019 — custom SessionStore backend (quickstart 场景 7)
// ---------------------------------------------------------------------------

/// Minimal custom `SessionStore` backed by an in-memory map, simulating a
/// developer-provided backend (e.g. SQLite semantics) without touching the
/// framework.
struct HashMapSessionStore {
    sessions: RwLock<HashMap<String, String>>,
}

impl HashMapSessionStore {
    fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl SessionStore for HashMapSessionStore {
    async fn save(&self, session: &dyn Session) -> Result<(), SessionError> {
        let json = serde_json::to_string(session.state()).map_err(|e| {
            SessionError::SerializationError {
                session_id: session.id().to_string(),
                reason: e.to_string(),
            }
        })?;
        self.sessions
            .write()
            .unwrap()
            .insert(session.id().to_string(), json);
        Ok(())
    }

    async fn load(&self, id: &str) -> Result<SessionImpl, SessionError> {
        let sessions = self.sessions.read().unwrap();
        let json = sessions.get(id).ok_or_else(|| SessionError::NotFound {
            session_id: id.to_string(),
        })?;
        let state: AgentState =
            serde_json::from_str(json).map_err(|e| SessionError::SerializationError {
                session_id: id.to_string(),
                reason: e.to_string(),
            })?;
        Ok(SessionImpl::new(state))
    }

    async fn delete(&self, id: &str) -> Result<(), SessionError> {
        self.sessions.write().unwrap().remove(id);
        Ok(())
    }

    async fn list_ids(&self) -> Result<Vec<String>, SessionError> {
        Ok(self.sessions.read().unwrap().keys().cloned().collect())
    }

    async fn list_meta(&self) -> Result<Vec<SessionMeta>, SessionError> {
        let sessions = self.sessions.read().unwrap();
        let mut metas = Vec::new();
        for id in sessions.keys() {
            metas.push(SessionMeta {
                session_id: id.clone(),
                status: SessionStatus::Active,
                message_count: 0,
                created_at: chrono::Utc::now(),
                last_active: chrono::Utc::now(),
            });
        }
        Ok(metas)
    }
}

#[tokio::test]
async fn test_custom_backend_round_trip() {
    let store = Arc::new(HashMapSessionStore::new());
    let model = Arc::new(MockModel::new("mock", "custom backend reply"));

    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .session_store(Arc::clone(&store) as Arc<dyn SessionStore>)
        .session_id("c-1")
        .build()
        .unwrap();
    let agent = ReActAgent::build(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .await
    .unwrap();

    agent
        .reply(Some(vec![user_msg("user", "with custom backend").unwrap()]))
        .await
        .unwrap();

    // The custom backend persisted the session — read it back through the
    // same store instance (identical semantics to the built-in backend).
    let loaded = store.load("c-1").await;
    assert!(
        matches!(loaded, Ok(session) if session.state().context_length() >= 2),
        "custom backend must preserve the round-trip history"
    );

    // Loading a non-existent session returns a clear "not found" result.
    assert!(
        matches!(
            store.load("does-not-exist").await,
            Err(SessionError::NotFound { .. })
        ),
        "custom backend must return a clear NotFound for missing sessions"
    );

    // Delete is idempotent.
    store.delete("c-1").await.unwrap();
    store.delete("c-1").await.unwrap();
}
