//! JSON-file backed [`SessionStore`] implementation.
//!
//! This is the built-in, out-of-the-box persistence backend for agent state
//! (Feature 025). Each session is persisted as one `{session_id}.json` file in
//! a configurable directory (default `sessions/`), containing the lightweight
//! [`SessionMeta`] fields plus the full [`AgentState`] — mirroring the logical
//! shape of the Python reference implementation's `SessionRecord`.
//!
//! # Guarantees
//!
//! - **Atomic writes**: a temp file is written, fsynced, then renamed over the
//!   target. A crash at any point leaves either the old or the new complete
//!   file, never a partially-written one.
//! - **Path-traversal safety**: session ids are validated before any filesystem
//!   access; `/`, `\`, `.` and empty ids are rejected.
//! - **Stable data protocol**: all `SessionRecordFile` fields are
//!   `#[serde(default)]`, and `AgentState` ignores unknown fields, so files from
//!   older or newer versions load compatibly.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::agent_state::AgentState;
use crate::session::{Session, SessionError, SessionImpl, SessionMeta, SessionStatus};
use crate::session_store::SessionStore;

// ---------------------------------------------------------------------------
// SessionRecordFile — on-disk record (T004)
// ---------------------------------------------------------------------------

/// On-disk session record: one `{session_id}.json` file per session.
///
/// The top-level fields are the lightweight session metadata; `state` carries
/// the full [`AgentState`]. Mirrors the Python reference `SessionRecord`
/// logical structure (`id / created_at / updated_at / status / message_count /
/// state`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecordFile {
    pub session_id: String,
    #[serde(default = "default_status")]
    pub status: SessionStatus,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default = "default_utc_now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "default_utc_now")]
    pub last_active: DateTime<Utc>,
    pub state: AgentState,
}

// ---------------------------------------------------------------------------
// Session-id validation (T005)
// ---------------------------------------------------------------------------

/// Validate a session id as a safe file-name component.
///
/// Rejects empty ids and ids containing path separators (`/`, `\`) or `.`,
/// which could otherwise enable path traversal or overwriting unrelated files.
fn validate_session_id(id: &str) -> Result<(), SessionError> {
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
// Atomic write helper (T006)
// ---------------------------------------------------------------------------

/// Atomically write `contents` to `{dir}/{id}.json`.
///
/// Writes to a temp file, fsyncs it, then renames over the target. A crash at
/// any point leaves either the old or the new complete file — never a
/// partially-written one (spec FR-004).
async fn atomic_write(dir: &Path, id: &str, contents: &[u8]) -> Result<(), SessionError> {
    let final_path = dir.join(format!("{id}.json"));
    // Unique temp name so concurrent saves of the same session don't clobber
    // each other's temp file mid-write (a fixed `{id}.json.tmp` let one writer
    // truncate the other's temp file, causing lost updates or spurious errors).
    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let unique = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = dir.join(format!("{id}.{}.{unique}.json.tmp", std::process::id()));

    fs::write(&tmp_path, contents)
        .await
        .map_err(|e| SessionError::StorageError {
            session_id: id.to_string(),
            reason: format!("failed to write temp file: {e}"),
        })?;

    // fsync the temp file so its contents reach disk before the rename.
    let file = fs::File::open(&tmp_path)
        .await
        .map_err(|e| SessionError::StorageError {
            session_id: id.to_string(),
            reason: format!("failed to open temp file for fsync: {e}"),
        })?;
    file.sync_all()
        .await
        .map_err(|e| SessionError::StorageError {
            session_id: id.to_string(),
            reason: format!("failed to fsync temp file: {e}"),
        })?;

    fs::rename(&tmp_path, &final_path)
        .await
        .map_err(|e| SessionError::StorageError {
            session_id: id.to_string(),
            reason: format!("failed to rename temp file into place: {e}"),
        })?;

    // Best-effort cleanup of the temp file (normally already moved by rename).
    let _ = fs::remove_file(&tmp_path).await;

    Ok(())
}

// ---------------------------------------------------------------------------
// JsonFileSessionStore (T013)
// ---------------------------------------------------------------------------

/// JSON-file backed [`SessionStore`].
///
/// Persists each session as one `{session_id}.json` file in `dir`. This is the
/// default backend used when no store is injected into an agent.
#[derive(Clone)]
pub struct JsonFileSessionStore {
    dir: PathBuf,
}

impl JsonFileSessionStore {
    /// Create a store rooted at `dir`. The directory is created lazily on the
    /// first save.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Create a store rooted at the default `sessions/` directory (relative to
    /// the current working directory).
    pub fn with_default_dir() -> Self {
        Self::new("sessions")
    }

    /// The directory this store reads and writes session files in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Create the store directory if it does not exist.
    async fn ensure_dir(&self) -> Result<(), SessionError> {
        fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| SessionError::StorageError {
                session_id: String::new(),
                reason: format!(
                    "failed to create store directory {}: {e}",
                    self.dir.display()
                ),
            })
    }

    /// Read the raw bytes of a session file, mapping a missing file to
    /// `SessionError::NotFound`.
    async fn read_file(&self, id: &str) -> Result<Vec<u8>, SessionError> {
        let path = self.dir.join(format!("{id}.json"));
        match fs::read(&path).await {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(SessionError::NotFound {
                session_id: id.to_string(),
            }),
            Err(e) => Err(SessionError::StorageError {
                session_id: id.to_string(),
                reason: format!("failed to read session file: {e}"),
            }),
        }
    }
}

impl Default for JsonFileSessionStore {
    fn default() -> Self {
        Self::with_default_dir()
    }
}

#[async_trait]
impl SessionStore for JsonFileSessionStore {
    async fn save(&self, session: &dyn Session) -> Result<(), SessionError> {
        let id = session.id().to_string();
        validate_session_id(&id)?;
        self.ensure_dir().await?;

        let state = session.state();
        let record = SessionRecordFile {
            session_id: id.clone(),
            status: session.status(),
            message_count: state.context_length(),
            created_at: session.created_at(),
            last_active: session.last_active(),
            state: state.clone(),
        };

        let json =
            serde_json::to_vec_pretty(&record).map_err(|e| SessionError::SerializationError {
                session_id: id.clone(),
                reason: e.to_string(),
            })?;

        atomic_write(&self.dir, &id, &json).await
    }

    async fn load(&self, id: &str) -> Result<SessionImpl, SessionError> {
        validate_session_id(id)?;

        let bytes = self.read_file(id).await?;
        let record: SessionRecordFile =
            serde_json::from_slice(&bytes).map_err(|e| SessionError::SerializationError {
                session_id: id.to_string(),
                reason: e.to_string(),
            })?;

        // The file name is authoritative for the session id.
        let mut state = record.state;
        state.session_id = id.to_string();
        Ok(
            SessionImpl::new(state)
                .with_persisted_timestamps(record.created_at, record.last_active),
        )
    }

    async fn delete(&self, id: &str) -> Result<(), SessionError> {
        validate_session_id(id)?;

        let path = self.dir.join(format!("{id}.json"));
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            // Idempotent — deleting a non-existent session is not an error.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(SessionError::StorageError {
                session_id: id.to_string(),
                reason: format!("failed to delete session file: {e}"),
            }),
        }
    }

    async fn list_ids(&self) -> Result<Vec<String>, SessionError> {
        let mut ids = Vec::new();

        let mut entries = match fs::read_dir(&self.dir).await {
            Ok(entries) => entries,
            // A missing directory simply means no sessions yet.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ids),
            Err(e) => {
                return Err(SessionError::StorageError {
                    session_id: String::new(),
                    reason: format!("failed to read store directory: {e}"),
                });
            }
        };

        while let Some(entry) =
            entries
                .next_entry()
                .await
                .map_err(|e| SessionError::StorageError {
                    session_id: String::new(),
                    reason: format!("failed to read store directory entry: {e}"),
                })?
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(".json") {
                // Ignore temp files and anything that is not a valid session id.
                if validate_session_id(stem).is_ok() {
                    ids.push(stem.to_string());
                }
            }
        }

        Ok(ids)
    }

    async fn list_meta(&self) -> Result<Vec<SessionMeta>, SessionError> {
        let mut metas = Vec::new();

        for id in self.list_ids().await? {
            let path = self.dir.join(format!("{id}.json"));
            let bytes = match fs::read(&path).await {
                Ok(bytes) => bytes,
                // A file can disappear between listing and reading; skip it.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(SessionError::StorageError {
                        session_id: id.clone(),
                        reason: format!("failed to read session file: {e}"),
                    });
                }
            };

            // Parse only the lightweight outer metadata, skipping the full
            // AgentState payload (spec FR-010). A single corrupted/truncated
            // session file must not make the whole list fail — skip it with a
            // warning so the remaining sessions stay enumerable (audit M5).
            let meta: SessionMetaOnly = match serde_json::from_slice(&bytes) {
                Ok(meta) => meta,
                Err(e) => {
                    eprintln!("json_file_store: skipping corrupted session file '{id}': {e}");
                    continue;
                }
            };

            metas.push(SessionMeta {
                session_id: meta.session_id,
                status: meta.status,
                message_count: meta.message_count,
                created_at: meta.created_at,
                last_active: meta.last_active,
            });
        }

        // Sort by last_active descending (session-store.md §4).
        metas.sort_by_key(|m| std::cmp::Reverse(m.last_active));
        Ok(metas)
    }
}

/// Lightweight subset of [`SessionRecordFile`] used by `list_meta` to avoid
/// deserializing the full [`AgentState`] payload.
#[derive(Debug, Deserialize)]
struct SessionMetaOnly {
    #[serde(default)]
    session_id: String,
    #[serde(default = "default_status")]
    status: SessionStatus,
    #[serde(default)]
    message_count: usize,
    #[serde(default = "default_utc_now")]
    created_at: DateTime<Utc>,
    #[serde(default = "default_utc_now")]
    last_active: DateTime<Utc>,
}

fn default_status() -> SessionStatus {
    SessionStatus::Active
}

fn default_utc_now() -> DateTime<Utc> {
    Utc::now()
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// T005: Session id validation.
    #[test]
    fn test_validate_session_id_rejects_invalid() {
        assert!(validate_session_id("").is_err());
        assert!(validate_session_id("a/b").is_err());
        assert!(validate_session_id("a\\b").is_err());
        assert!(validate_session_id("a.b").is_err());
        assert!(validate_session_id("..").is_err());

        assert!(validate_session_id("s-1").is_ok());
        assert!(validate_session_id("a1b2c3d4").is_ok());
    }

    /// T013: Default store uses the `sessions/` directory.
    #[test]
    fn test_default_dir() {
        let store = JsonFileSessionStore::with_default_dir();
        assert_eq!(store.dir(), Path::new("sessions"));
    }

    /// Sanity: JsonFileSessionStore is usable as a `dyn SessionStore`.
    #[test]
    fn test_store_is_usable_as_trait_object() {
        let store = JsonFileSessionStore::new(std::env::temp_dir());
        let _boxed: Box<dyn SessionStore + Send + Sync> = Box::new(store);
    }
}
