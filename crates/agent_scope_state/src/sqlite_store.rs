//! SQLite-backed [`SessionStore`] implementation.
//!
//! Each session is stored as one row containing lightweight metadata plus the
//! full [`AgentState`](crate::AgentState) JSON payload. The logical record
//! matches [`SessionRecordFile`](crate::SessionRecordFile), but SQLite gives
//! callers a single-file store with indexed lookups and atomic upserts.

use std::path::Path;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::agent_state::AgentState;
use crate::session::{Session, SessionError, SessionImpl, SessionMeta, SessionStatus};
use crate::session_store::{SessionStore, validate_session_id};

/// SQLite-backed [`SessionStore`].
#[derive(Clone)]
pub struct SqliteSessionStore {
    pool: SqlitePool,
}

impl SqliteSessionStore {
    /// Open or create a SQLite database file and initialize the session schema.
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, SessionError> {
        let options = SqliteConnectOptions::new()
            .filename(path.as_ref())
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| SessionError::StorageError {
                session_id: String::new(),
                reason: format!("failed to open sqlite session store: {e}"),
            })?;
        Self::from_pool(pool).await
    }

    /// Create an isolated in-memory SQLite store.
    pub async fn connect_in_memory() -> Result<Self, SessionError> {
        let options = SqliteConnectOptions::new().filename(":memory:");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|e| SessionError::StorageError {
                session_id: String::new(),
                reason: format!("failed to open in-memory sqlite session store: {e}"),
            })?;
        Self::from_pool(pool).await
    }

    /// Wrap an existing SQLite pool and initialize the session schema.
    pub async fn from_pool(pool: SqlitePool) -> Result<Self, SessionError> {
        let store = Self { pool };
        store.ensure_schema().await?;
        Ok(store)
    }

    /// Access the underlying pool for callers that need to share a connection.
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn ensure_schema(&self) -> Result<(), SessionError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY NOT NULL,
                status_json TEXT NOT NULL,
                message_count INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                last_active TEXT NOT NULL,
                state_json TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| SessionError::StorageError {
            session_id: String::new(),
            reason: format!("failed to initialize sqlite session schema: {e}"),
        })?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sessions_last_active
            ON sessions(last_active DESC)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| SessionError::StorageError {
            session_id: String::new(),
            reason: format!("failed to initialize sqlite session index: {e}"),
        })?;

        Ok(())
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn save(&self, session: &dyn Session) -> Result<(), SessionError> {
        let id = session.id().to_string();
        validate_session_id(&id)?;

        let state_json = serde_json::to_string(session.state()).map_err(|e| {
            SessionError::SerializationError {
                session_id: id.clone(),
                reason: e.to_string(),
            }
        })?;
        let status_json = serde_json::to_string(&session.status()).map_err(|e| {
            SessionError::SerializationError {
                session_id: id.clone(),
                reason: e.to_string(),
            }
        })?;
        let message_count = i64::try_from(session.state().context_length()).map_err(|e| {
            SessionError::StorageError {
                session_id: id.clone(),
                reason: format!("message count does not fit sqlite integer: {e}"),
            }
        })?;

        sqlx::query(
            r#"
            INSERT INTO sessions (
                session_id,
                status_json,
                message_count,
                created_at,
                last_active,
                state_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(session_id) DO UPDATE SET
                status_json = excluded.status_json,
                message_count = excluded.message_count,
                created_at = excluded.created_at,
                last_active = excluded.last_active,
                state_json = excluded.state_json
            "#,
        )
        .bind(&id)
        .bind(status_json)
        .bind(message_count)
        .bind(session.created_at().to_rfc3339())
        .bind(session.last_active().to_rfc3339())
        .bind(state_json)
        .execute(&self.pool)
        .await
        .map_err(|e| SessionError::StorageError {
            session_id: id,
            reason: format!("failed to save sqlite session: {e}"),
        })?;

        Ok(())
    }

    async fn load(&self, id: &str) -> Result<SessionImpl, SessionError> {
        validate_session_id(id)?;

        let row = sqlx::query(
            r#"
            SELECT state_json, created_at, last_active
            FROM sessions
            WHERE session_id = ?1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SessionError::StorageError {
            session_id: id.to_string(),
            reason: format!("failed to load sqlite session: {e}"),
        })?
        .ok_or_else(|| SessionError::NotFound {
            session_id: id.to_string(),
        })?;

        let state_json: String = row.get("state_json");
        let created_at = parse_datetime(id, row.get("created_at"))?;
        let last_active = parse_datetime(id, row.get("last_active"))?;
        let mut state: AgentState =
            serde_json::from_str(&state_json).map_err(|e| SessionError::SerializationError {
                session_id: id.to_string(),
                reason: e.to_string(),
            })?;

        // The lookup key is authoritative, matching JsonFileSessionStore.
        state.session_id = id.to_string();
        Ok(SessionImpl::new(state).with_persisted_timestamps(created_at, last_active))
    }

    async fn delete(&self, id: &str) -> Result<(), SessionError> {
        validate_session_id(id)?;

        sqlx::query("DELETE FROM sessions WHERE session_id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| SessionError::StorageError {
                session_id: id.to_string(),
                reason: format!("failed to delete sqlite session: {e}"),
            })?;

        Ok(())
    }

    async fn list_ids(&self) -> Result<Vec<String>, SessionError> {
        let rows = sqlx::query("SELECT session_id FROM sessions ORDER BY session_id ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| SessionError::StorageError {
                session_id: String::new(),
                reason: format!("failed to list sqlite session ids: {e}"),
            })?;

        Ok(rows.into_iter().map(|row| row.get("session_id")).collect())
    }

    async fn list_meta(&self) -> Result<Vec<SessionMeta>, SessionError> {
        let rows = sqlx::query(
            r#"
            SELECT session_id, status_json, message_count, created_at, last_active
            FROM sessions
            ORDER BY last_active DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SessionError::StorageError {
            session_id: String::new(),
            reason: format!("failed to list sqlite session metadata: {e}"),
        })?;

        rows.into_iter()
            .map(|row| {
                let session_id: String = row.get("session_id");
                let message_count: i64 = row.get("message_count");
                Ok(SessionMeta {
                    session_id: session_id.clone(),
                    status: parse_status(&session_id, row.get("status_json"))?,
                    message_count: usize::try_from(message_count).map_err(|e| {
                        SessionError::StorageError {
                            session_id: session_id.clone(),
                            reason: format!("invalid sqlite message count: {e}"),
                        }
                    })?,
                    created_at: parse_datetime(&session_id, row.get("created_at"))?,
                    last_active: parse_datetime(&session_id, row.get("last_active"))?,
                })
            })
            .collect()
    }
}

fn parse_status(session_id: &str, value: String) -> Result<SessionStatus, SessionError> {
    serde_json::from_str(&value).map_err(|e| SessionError::SerializationError {
        session_id: session_id.to_string(),
        reason: format!("invalid session status: {e}"),
    })
}

fn parse_datetime(session_id: &str, value: String) -> Result<DateTime<Utc>, SessionError> {
    DateTime::parse_from_rfc3339(&value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| SessionError::SerializationError {
            session_id: session_id.to_string(),
            reason: format!("invalid session timestamp: {e}"),
        })
}

#[cfg(test)]
mod tests {
    use agent_scope_message::{ContentBlock, Role, TextBlock};

    use super::*;

    fn add_text(session: &mut SessionImpl, text: &str) {
        let msg = agent_scope_message::Msg::new(
            "user".into(),
            vec![ContentBlock::Text(TextBlock::new(text.to_string()))],
            Role::User,
        )
        .unwrap();
        session.state_mut().context.push(msg);
    }

    #[tokio::test]
    async fn round_trips_session_state_and_meta() {
        let store = SqliteSessionStore::connect_in_memory().await.unwrap();
        let mut session = SessionImpl::with_session_id("s1".into());
        add_text(&mut session, "hello");

        store.save(&session).await.unwrap();

        let loaded = store.load("s1").await.unwrap();
        assert_eq!(loaded.id(), "s1");
        assert_eq!(loaded.state().context_length(), 1);
        assert_eq!(loaded.created_at(), session.created_at());
        assert_eq!(loaded.last_active(), session.last_active());

        let meta = store.list_meta().await.unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].session_id, "s1");
        assert_eq!(meta[0].message_count, 1);
    }

    #[tokio::test]
    async fn save_upserts_existing_session() {
        let store = SqliteSessionStore::connect_in_memory().await.unwrap();
        let mut session = SessionImpl::with_session_id("s1".into());
        store.save(&session).await.unwrap();

        add_text(&mut session, "second");
        store.save(&session).await.unwrap();

        assert_eq!(store.list_ids().await.unwrap(), vec!["s1"]);
        assert_eq!(store.load("s1").await.unwrap().state().context_length(), 1);
    }

    #[tokio::test]
    async fn delete_is_idempotent_and_load_reports_missing() {
        let store = SqliteSessionStore::connect_in_memory().await.unwrap();
        let session = SessionImpl::with_session_id("s1".into());
        store.save(&session).await.unwrap();

        store.delete("s1").await.unwrap();
        store.delete("s1").await.unwrap();

        assert!(matches!(
            store.load("s1").await,
            Err(SessionError::NotFound { .. })
        ));
        assert!(store.list_ids().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn file_database_persists_across_store_instances() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("sessions.sqlite");
        let store = SqliteSessionStore::connect(&db_path).await.unwrap();
        let session = SessionImpl::with_session_id("s1".into());
        store.save(&session).await.unwrap();
        drop(store);

        let reopened = SqliteSessionStore::connect(&db_path).await.unwrap();
        assert_eq!(reopened.list_ids().await.unwrap(), vec!["s1"]);
        assert_eq!(reopened.load("s1").await.unwrap().id(), "s1");
    }

    #[tokio::test]
    async fn invalid_session_ids_are_rejected() {
        let store = SqliteSessionStore::connect_in_memory().await.unwrap();
        assert!(matches!(
            store.load("../evil").await,
            Err(SessionError::StorageError { .. })
        ));
    }
}
