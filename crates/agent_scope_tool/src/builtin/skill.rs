//! SkillTool — built-in `Skill` tool for viewing skill content by exact name.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_skill.py` (upstream commit `9d1026fa`) and the
//! existing `agent_scope_tool::skill_viewer::SkillViewer`.
//!
//! Unlike the auto-registered `SkillViewer`, this tool is bound to the shared
//! [`WorkspaceToolSession`] so the activated tool groups (managed by
//! `ResetTools`) filter which skills are visible, aligning with Python
//! `_skill.py:112` (`get_skills_method(activated_groups)`).

use std::collections::HashMap;

use agent_scope_message::ToolResultState;
#[cfg(test)]
use agent_scope_message::{ToolOutput, ToolResultBlock};
use serde_json::Value as JsonValue;

use crate::make_skill_text_result as make_result;
use crate::skill_viewer::ListSkillsCallback;
use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::BuiltInToolContext;
use super::ToolErrorCategory;

/// Built-in `Skill` tool (session-aware).
///
/// Resolves the skill name through a caller-provided callback that is invoked
/// with the *currently active tool groups* from the shared session.
pub struct SkillTool {
    ctx: BuiltInToolContext,
    _get_skills_method: ListSkillsCallback,
}

impl SkillTool {
    /// Create a new [`SkillTool`] bound to a workspace context.
    ///
    /// `get_skills_method` maps the activated group names to the available
    /// skills (name → [`Skill`](agent_scope_workspace::Skill)).
    #[must_use]
    pub fn new(ctx: BuiltInToolContext, get_skills_method: ListSkillsCallback) -> Self {
        Self {
            ctx,
            _get_skills_method: get_skills_method,
        }
    }
}

#[async_trait::async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    fn description(&self) -> &str {
        "Retrieve a skill within the conversation. When users ask you to \
         perform tasks, check if any of the available skills match. \
         Skills provide specialized capabilities and domain knowledge."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "The exact name of the skill to view."
                }
            },
            "required": ["skill"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let skill_name = match input.get("skill").and_then(JsonValue::as_str) {
            Some(s) => s.to_string(),
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "SkillNotFoundError: missing required 'skill' parameter".to_string(),
                    ToolResultState::Error,
                )));
            }
        };

        // Activated groups from the shared session (ResetTools-managed).
        // Falls back to an empty list (all groups) when the session is
        // unavailable, matching the unconstrained SkillViewer behaviour.
        let activated_groups: Vec<String> = self.session_groups();

        let skills_map: HashMap<String, agent_scope_workspace::Skill> =
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (self._get_skills_method)(&activated_groups)
            })) {
                Ok(map) => map,
                Err(_) => {
                    tracing::warn!("SkillTool callback panicked for skill '{skill_name}'");
                    return Ok(ToolExecOutput::Complete(make_result(
                        format!(
                            "SkillNotFoundError: internal error retrieving skill '{skill_name}'"
                        ),
                        ToolResultState::Error,
                    )));
                }
            };

        match skills_map.get(&skill_name) {
            Some(skill) => Ok(ToolExecOutput::Complete(make_result(
                skill.markdown.clone(),
                ToolResultState::Success,
            ))),
            None => {
                tracing::warn!("SkillTool: skill '{}' not found", skill_name);
                Ok(ToolExecOutput::Complete(make_result(
                    format!(
                        "Error: {}: skill_not_found: SkillNotFoundError: Skill '{skill_name}' not found.",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )))
            }
        }
    }
}

impl SkillTool {
    /// Read the currently active tool groups from the shared session.
    fn session_groups(&self) -> Vec<String> {
        match self.ctx.session.read() {
            Ok(guard) => guard.all_active_groups(),
            Err(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::WorkspaceToolSession;
    use agent_scope_workspace::Skill;
    use agent_scope_workspace::backend::{LocalBackend, WorkspaceBackend};
    use std::sync::{Arc, RwLock};

    /// A callback that returns skills regardless of the active groups.
    fn make_skills() -> HashMap<String, Skill> {
        let mut map = HashMap::new();
        map.insert(
            "test-skill".to_string(),
            Skill {
                name: "test-skill".into(),
                description: "A test skill".into(),
                dir: "/tmp/test".into(),
                markdown: "# Hello World".into(),
                updated_at: 0.0,
            },
        );
        map
    }

    fn ctx_in(dir: &tempfile::TempDir) -> (BuiltInToolContext, Arc<RwLock<WorkspaceToolSession>>) {
        let workdir = dir.path().to_string_lossy().to_string();
        let backend: Arc<dyn WorkspaceBackend> = Arc::new(LocalBackend::new());
        let session = Arc::new(RwLock::new(WorkspaceToolSession::new("ws-1")));
        let ctx = BuiltInToolContext::new(backend, workdir, session.clone());
        (ctx, session)
    }

    fn text_of(block: &ToolResultBlock) -> String {
        match &block.output {
            ToolOutput::Text(t) => t.clone(),
            _ => String::new(),
        }
    }

    #[tokio::test]
    async fn skill_missing_parameter() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = SkillTool::new(ctx, Box::new(|_| make_skills()));
        let out = tool.call(serde_json::json!({})).await.unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("SkillNotFoundError"));
    }

    #[tokio::test]
    async fn skill_known_skill_success() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = SkillTool::new(ctx, Box::new(|_| make_skills()));
        let out = tool
            .call(serde_json::json!({ "skill": "test-skill" }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        assert_eq!(text_of(&block), "# Hello World");
    }

    #[tokio::test]
    async fn skill_unknown_skill_error() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = SkillTool::new(ctx, Box::new(|_| make_skills()));
        let out = tool
            .call(serde_json::json!({ "skill": "unknown" }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        let text = text_of(&block);
        assert!(text.contains("skill_not_found"), "got: {text}");
        assert!(text.contains("unknown"), "got: {text}");
    }

    #[tokio::test]
    async fn skill_callback_receives_activated_groups() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, session) = ctx_in(&dir);
        // Record a group so `all_active_groups()` is `[basic, coding]`.
        session.write().unwrap().record_groups(["coding"]);
        let seen: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen2 = Arc::clone(&seen);
        let tool = SkillTool::new(
            ctx,
            Box::new(move |groups: &[String]| {
                *seen2.lock().unwrap() = groups.to_vec();
                make_skills()
            }),
        );
        let _ = tool
            .call(serde_json::json!({ "skill": "test-skill" }))
            .await
            .unwrap();
        let groups = seen.lock().unwrap();
        assert!(groups.contains(&"coding".to_string()), "got: {groups:?}");
        assert!(groups.contains(&"basic".to_string()), "got: {groups:?}");
    }
}
