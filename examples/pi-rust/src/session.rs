use std::fs;
use std::path::PathBuf;

use agent_scope_event::AgentEvent;
use serde::{Deserialize, Serialize};

use crate::config::RuntimeConfig;
use crate::error::{PiError, PiResult};

/// Monotonic counter for unique temp-file names in [`SessionStore::save`].
static SAVE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub cwd: String,
    pub model: String,
    pub turns: Vec<ConversationTurn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub index: usize,
    pub user_input: String,
    pub events: Vec<AgentEvent>,
    pub assistant_text: String,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub code: String,
    pub message: String,
    pub category: String,
    pub retryable: bool,
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub updated_at: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    dir: PathBuf,
}

impl SessionRecord {
    pub fn new(config: &RuntimeConfig) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().as_simple().to_string(),
            created_at: now.clone(),
            updated_at: now,
            cwd: config.cwd.display().to_string(),
            model: config.model.clone(),
            turns: Vec::new(),
            summary: None,
        }
    }

    pub fn add_turn(
        &mut self,
        user_input: String,
        events: Vec<AgentEvent>,
        assistant_text: String,
        error: Option<ErrorRecord>,
    ) {
        let now = chrono::Utc::now().to_rfc3339();
        let index = self.turns.len();
        self.turns.push(ConversationTurn {
            index,
            user_input,
            events,
            assistant_text,
            started_at: now.clone(),
            completed_at: Some(now.clone()),
            error,
        });
        self.updated_at = now;
        self.summary = self.turns.last().map(|turn| summarize(&turn.user_input));
    }
}

impl SessionStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn save(&self, session: &SessionRecord) -> PiResult<()> {
        fs::create_dir_all(&self.dir).map_err(|err| PiError::io("create sessions dir", err))?;
        let safe_id = safe_component(&session.id);
        let path = self.dir.join(format!("{safe_id}.json"));
        let json = serde_json::to_string_pretty(session)?;
        // Atomic write: rename is atomic on POSIX, so a crash mid-save cannot
        // leave a truncated session file behind. The tmp name uses the same
        // sanitized id so it cannot escape the sessions directory either.
        let unique = SAVE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = self
            .dir
            .join(format!(".{safe_id}.tmp-{}-{unique}", std::process::id()));
        fs::write(&tmp, json).map_err(|err| PiError::io("write session tmp", err))?;
        fs::rename(&tmp, path).map_err(|err| PiError::io("commit session", err))
    }

    pub fn load(&self, id: &str) -> PiResult<SessionRecord> {
        let path = self.path_for(id);
        if !path.exists() {
            return Err(PiError::session(format!("session '{id}' does not exist")));
        }
        let json = fs::read_to_string(path).map_err(|err| PiError::io("read session", err))?;
        serde_json::from_str(&json).map_err(PiError::from)
    }

    pub fn load_latest(&self) -> PiResult<Option<SessionRecord>> {
        let summaries = self.list()?;
        let Some(latest) = summaries.first() else {
            return Ok(None);
        };
        self.load(&latest.id).map(Some)
    }

    pub fn list(&self) -> PiResult<Vec<SessionSummary>> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(|err| PiError::io("read sessions dir", err))? {
            let entry = entry.map_err(|err| PiError::io("read session entry", err))?;
            if entry.path().extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let json = fs::read_to_string(entry.path())
                .map_err(|err| PiError::io("read session file", err))?;
            if let Ok(record) = serde_json::from_str::<SessionRecord>(&json) {
                records.push(SessionSummary {
                    id: record.id,
                    updated_at: record.updated_at,
                    summary: record.summary.unwrap_or_else(|| "new session".to_string()),
                });
            }
        }
        records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(records)
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{}.json", safe_component(id)))
    }
}

/// Reduce a caller-supplied id to a single safe path component so it cannot
/// traverse out of the sessions directory via `..` or a path separator.
///
/// An id made entirely of safe characters is returned unchanged (so existing
/// `{id}.json` files stay addressable). Only when a character must be replaced
/// is a short hash of the original appended, so two distinct ids that sanitize
/// to the same component (e.g. `"a/b"` and `"a_b"`) still map to distinct files
/// instead of silently overwriting each other's session (audit S7).
fn safe_component(id: &str) -> String {
    let mut needs_hash = false;
    let mut out = String::with_capacity(id.len() + 9);
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
            needs_hash = true;
        }
    }
    if out.is_empty() {
        out.push('_');
        needs_hash = true;
    }
    if needs_hash {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut hasher);
        let hash = hasher.finish();
        out.push('-');
        out.push_str(&format!("{hash:x}"));
    }
    out
}

fn summarize(input: &str) -> String {
    let mut text: String = input.chars().take(80).collect();
    if input.chars().count() > 80 {
        text.push('…');
    }
    text
}
