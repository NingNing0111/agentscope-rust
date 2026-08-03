use std::fs;
use std::path::PathBuf;

use agent_scope_event::AgentEvent;
use serde::{Deserialize, Serialize};

use crate::config::RuntimeConfig;
use crate::error::{PiError, PiResult};

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
        let path = self.path_for(&session.id);
        let json = serde_json::to_string_pretty(session)?;
        fs::write(path, json).map_err(|err| PiError::io("write session", err))
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
        self.dir.join(format!("{id}.json"))
    }
}

fn summarize(input: &str) -> String {
    let mut text: String = input.chars().take(80).collect();
    if input.chars().count() > 80 {
        text.push('…');
    }
    text
}
