//! AgentState — agent runtime state.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use agent_scope_message::{ContentBlock, Msg, Role, ToolCallState};

use crate::permission::PermissionContext;
use crate::task::TaskContext;

fn default_session_id() -> String {
    uuid::Uuid::new_v4().as_simple().to_string()
}

// ---------------------------------------------------------------------------
// ReplyContext
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyContext {
    #[serde(default = "default_session_id")]
    pub reply_id: String,
    #[serde(default)]
    pub cur_iter: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<serde_json::Value>,
}

impl Default for ReplyContext {
    fn default() -> Self {
        Self {
            reply_id: default_session_id(),
            cur_iter: 0,
            structured_schema: None,
            structured_output: None,
        }
    }
}

// ---------------------------------------------------------------------------
// SummaryContent
// ---------------------------------------------------------------------------

/// Summary content — either a plain string or a list of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SummaryContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl Default for SummaryContent {
    fn default() -> Self {
        SummaryContent::Text(String::new())
    }
}

// ---------------------------------------------------------------------------
// ReadCacheEntry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadCacheEntry {
    pub lines: Vec<String>,
    pub updated_at: f64,
    pub bytes: f64,
    pub file_path: String,
}

// ---------------------------------------------------------------------------
// ToolContext
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContext {
    #[serde(default = "default_max_cache_files")]
    pub max_cache_files: usize,
    #[serde(default = "default_max_cache_bytes")]
    pub max_cache_bytes: f64,
    #[serde(default)]
    pub read_file_cache: Vec<ReadCacheEntry>,
    #[serde(default)]
    pub activated_groups: Vec<String>,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            max_cache_files: default_max_cache_files(),
            max_cache_bytes: default_max_cache_bytes(),
            read_file_cache: Vec::new(),
            activated_groups: Vec::new(),
        }
    }
}

fn default_max_cache_files() -> usize {
    100
}

fn default_max_cache_bytes() -> f64 {
    25000.0
}

impl ToolContext {
    /// Check if a file cache entry exists and is valid (stale detection API placeholder).
    /// Full mtime-based validation requires filesystem access (tokio::fs).
    pub fn get_cache(&self, file_path: &str) -> Option<&ReadCacheEntry> {
        self.read_file_cache
            .iter()
            .find(|entry| entry.file_path == file_path)
    }

    /// Cache file content with LRU eviction.
    pub fn cache_file(&mut self, file_path: &str, lines: Vec<String>, bytes: f64) {
        // Remove any existing entry for the same file
        self.read_file_cache
            .retain(|entry| entry.file_path != file_path);

        // Evict oldest entries for file count limit
        while !self.read_file_cache.is_empty() && self.read_file_cache.len() >= self.max_cache_files
        {
            self.read_file_cache.remove(0);
        }

        // Evict oldest entries for byte limit
        let mut total_bytes: f64 = self.read_file_cache.iter().map(|e| e.bytes).sum();
        while !self.read_file_cache.is_empty() && total_bytes + bytes > self.max_cache_bytes {
            let removed = self.read_file_cache.remove(0);
            total_bytes -= removed.bytes;
        }

        self.read_file_cache.push(ReadCacheEntry {
            lines,
            updated_at: 0.0,
            bytes,
            file_path: file_path.to_string(),
        });
    }

    /// Clean cache entries not in the reserved set.
    pub fn clean_file_cache(&mut self, reserved_file_paths: &std::collections::HashSet<String>) {
        self.read_file_cache
            .retain(|entry| reserved_file_paths.contains(&entry.file_path));
    }
}

// ---------------------------------------------------------------------------
// AppendContextError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum AppendContextError {
    /// Context has reached the max_context_messages limit.
    ContextFull {
        max_messages: usize,
        current_count: usize,
    },
}

// ---------------------------------------------------------------------------
// AgentState
// ---------------------------------------------------------------------------

/// The agent state that should be saved and loaded from storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    #[serde(default = "default_session_id")]
    pub session_id: String,
    #[serde(default)]
    pub summary: SummaryContent,
    #[serde(default)]
    pub context: Vec<Msg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_context_messages: Option<usize>,
    #[serde(default)]
    pub reply_context: ReplyContext,
    #[serde(default)]
    pub permission_context: PermissionContext,
    #[serde(default)]
    pub tool_context: ToolContext,
    #[serde(default)]
    pub tasks_context: TaskContext,
    #[serde(default)]
    pub middle_context: HashMap<String, serde_json::Value>,
}

impl AgentState {
    /// Create a new AgentState with auto-generated session_id.
    pub fn new() -> Self {
        Self {
            session_id: default_session_id(),
            summary: SummaryContent::default(),
            context: Vec::new(),
            max_context_messages: None,
            reply_context: ReplyContext::default(),
            permission_context: PermissionContext::new(),
            tool_context: ToolContext::default(),
            tasks_context: TaskContext::default(),
            middle_context: HashMap::new(),
        }
    }

    /// Create with a custom session_id.
    pub fn with_session_id(session_id: String) -> Self {
        Self {
            session_id,
            ..Self::new()
        }
    }

    /// Append content blocks to the context.
    ///
    /// If the tail message is an assistant message with matching name and reply_id,
    /// blocks are appended to it. Otherwise a new assistant message is created.
    ///
    /// Returns `Err(AppendContextError::ContextFull)` if max_context_messages is reached.
    pub fn append_context(
        &mut self,
        name: &str,
        blocks: Vec<ContentBlock>,
    ) -> Result<(), AppendContextError> {
        // Check limit
        if let Some(max) = self.max_context_messages
            && self.context.len() >= max
        {
            return Err(AppendContextError::ContextFull {
                max_messages: max,
                current_count: self.context.len(),
            });
        }

        let reply_id = self.reply_context.reply_id.clone();

        // Try to append to tail assistant message with matching name+reply_id
        let should_append = self
            .context
            .last()
            .map(|msg| msg.role == Role::Assistant && msg.name == name && msg.id == reply_id)
            .unwrap_or(false);

        if should_append {
            if let Some(last_msg) = self.context.last_mut() {
                last_msg.content.extend(blocks);
            }
        } else {
            let new_msg = Msg::new(name.to_string(), blocks, Role::Assistant)
                .expect("assistant messages accept all content types");
            self.context.push(new_msg);
        }

        Ok(())
    }

    /// Check if the tail assistant message has awaiting tool calls.
    pub fn has_awaiting_tool_calls(&self, name: &str) -> bool {
        self.context
            .last()
            .filter(|msg| msg.role == Role::Assistant && msg.name == name)
            .map(|msg| {
                msg.content.iter().any(|block| {
                    if let ContentBlock::ToolCall(tc) = block {
                        matches!(tc.state, ToolCallState::Asking)
                            || (matches!(tc.state, ToolCallState::Submitted))
                    } else {
                        false
                    }
                })
            })
            .unwrap_or(false)
    }

    /// Get awaiting tool calls from the tail assistant message.
    pub fn get_awaiting_tool_calls(&self, name: &str) -> Vec<&agent_scope_message::ToolCallBlock> {
        self.context
            .last()
            .filter(|msg| msg.role == Role::Assistant && msg.name == name)
            .map(|msg| {
                msg.content
                    .iter()
                    .filter_map(|block| {
                        if let ContentBlock::ToolCall(tc) = block {
                            if matches!(tc.state, ToolCallState::Asking | ToolCallState::Submitted)
                            {
                                Some(tc)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Set the maximum number of context messages. `None` = no limit.
    pub fn set_max_context_messages(&mut self, max: Option<usize>) {
        self.max_context_messages = max;
    }

    /// Get the current number of context messages.
    pub fn context_length(&self) -> usize {
        self.context.len()
    }

    /// Auto-migrate from legacy JSON format (top-level reply_id/cur_iter).
    pub fn from_legacy_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut value: serde_json::Value = serde_json::from_str(json)?;

        // If top-level reply_id or cur_iter exist, merge into reply_context
        if let Some(obj) = value.as_object_mut() {
            let has_legacy = obj.contains_key("reply_id") || obj.contains_key("cur_iter");

            if has_legacy {
                // Extract legacy fields first (before entering reply_context borrow)
                let legacy_reply_id = obj.remove("reply_id");
                let legacy_cur_iter = obj.remove("cur_iter");

                let reply_context = obj
                    .entry("reply_context")
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

                if let Some(reply_ctx) = reply_context.as_object_mut() {
                    if let Some(reply_id) = legacy_reply_id
                        && !reply_ctx.contains_key("reply_id")
                    {
                        reply_ctx.insert("reply_id".to_string(), reply_id);
                    }
                    if let Some(cur_iter) = legacy_cur_iter
                        && !reply_ctx.contains_key("cur_iter")
                    {
                        reply_ctx.insert("cur_iter".to_string(), cur_iter);
                    }
                }
            }
        }

        serde_json::from_value(value)
    }
}

impl Default for AgentState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_message::TextBlock;

    #[test]
    fn test_agent_state_new() {
        let state = AgentState::new();
        assert!(!state.session_id.is_empty());
        assert_eq!(state.context_length(), 0);
        assert_eq!(state.reply_context.cur_iter, 0);
    }

    #[test]
    fn test_append_context_creates_new_message() {
        let mut state = AgentState::new();
        state.reply_context.reply_id = "reply-1".into();

        let blocks = vec![ContentBlock::Text(TextBlock::new("hello".into()))];
        state.append_context("agent", blocks).unwrap();

        assert_eq!(state.context_length(), 1);
        assert_eq!(state.context[0].name, "agent");
    }

    #[test]
    fn test_append_context_with_context_full() {
        let mut state = AgentState::new();
        state.set_max_context_messages(Some(1));

        // First append fills the context
        state
            .append_context(
                "agent",
                vec![ContentBlock::Text(TextBlock::new("msg1".into()))],
            )
            .unwrap();
        assert_eq!(state.context_length(), 1);

        // Second append should fail
        let result = state.append_context(
            "agent",
            vec![ContentBlock::Text(TextBlock::new("msg2".into()))],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_has_awaiting_tool_calls_false_for_empty() {
        let state = AgentState::new();
        assert!(!state.has_awaiting_tool_calls("agent"));
    }

    #[test]
    fn test_has_awaiting_tool_calls_detects_asking() {
        use agent_scope_message::{ToolCallBlock, ToolCallState};

        let mut state = AgentState::new();
        state.reply_context.reply_id = "reply-001".into();

        // Create an assistant message with ASKING tool call
        let mut tc = ToolCallBlock::new("tc-1".into(), "search".into(), "{}".into());
        tc.state = ToolCallState::Asking;

        let msg = Msg::new(
            "agent".into(),
            vec![ContentBlock::ToolCall(tc)],
            Role::Assistant,
        )
        .unwrap();
        state.context.push(msg);

        assert!(state.has_awaiting_tool_calls("agent"));
        let awaiting = state.get_awaiting_tool_calls("agent");
        assert_eq!(awaiting.len(), 1);
    }

    #[test]
    fn test_reply_context_default() {
        let rc = ReplyContext::default();
        assert!(!rc.reply_id.is_empty());
        assert_eq!(rc.cur_iter, 0);
        assert!(rc.structured_schema.is_none());
    }

    #[test]
    fn test_tool_context_lru_eviction() {
        let mut tc = ToolContext {
            max_cache_files: 2,
            max_cache_bytes: 10000.0,
            ..Default::default()
        };

        tc.cache_file("file1.txt", vec!["line1".into()], 100.0);
        tc.cache_file("file2.txt", vec!["line2".into()], 100.0);
        tc.cache_file("file3.txt", vec!["line3".into()], 100.0);

        // Should have evicted file1.txt
        assert_eq!(tc.read_file_cache.len(), 2);
        assert!(tc.get_cache("file1.txt").is_none());
        assert!(tc.get_cache("file2.txt").is_some());
        assert!(tc.get_cache("file3.txt").is_some());
    }

    #[test]
    fn test_clean_file_cache() {
        let mut tc = ToolContext::default();
        tc.cache_file("a.txt", vec!["a".into()], 50.0);
        tc.cache_file("b.txt", vec!["b".into()], 50.0);
        tc.cache_file("c.txt", vec!["c".into()], 50.0);

        let mut reserved = std::collections::HashSet::new();
        reserved.insert("b.txt".to_string());
        tc.clean_file_cache(&reserved);

        assert_eq!(tc.read_file_cache.len(), 1);
        assert!(tc.get_cache("b.txt").is_some());
    }

    #[test]
    fn test_agent_state_json_roundtrip() {
        let state = AgentState::new();
        let json = serde_json::to_string(&state).unwrap();
        let restored: AgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.session_id, state.session_id);
        assert_eq!(
            restored.reply_context.cur_iter,
            state.reply_context.cur_iter
        );
    }

    #[test]
    fn test_legacy_migration() {
        let legacy = r#"{
            "session_id": "abc123",
            "reply_id": "old-reply-id",
            "cur_iter": 5,
            "summary": "",
            "context": [],
            "reply_context": {},
            "permission_context": {},
            "tool_context": {"max_cache_files": 100, "max_cache_bytes": 25000.0, "read_file_cache": [], "activated_groups": []},
            "tasks_context": {"tasks": []},
            "middle_context": {}
        }"#;

        let state = AgentState::from_legacy_json(legacy).unwrap();
        assert_eq!(state.session_id, "abc123");
        assert_eq!(state.reply_context.reply_id, "old-reply-id");
        assert_eq!(state.reply_context.cur_iter, 5);
    }

    #[test]
    fn test_summary_content_default() {
        let sc = SummaryContent::default();
        if let SummaryContent::Text(t) = sc {
            assert!(t.is_empty());
        } else {
            panic!("expected Text variant");
        }
    }
}
