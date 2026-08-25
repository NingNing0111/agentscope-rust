//! ResetToolsTool — built-in `ResetTools` meta-tool for activating/deactivating
//! tool groups.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_meta.py` (upstream commit `9d1026fa`).
//!
//! The input schema is dynamic: one boolean field per non-`basic` authorized
//! tool group. The boolean values are the groups' **final activation state**
//! (not an increment) — any group not explicitly set to `true` is deactivated.
//! Activation never expands beyond the workspace authorization boundary
//! (FR-019).

use std::collections::BTreeSet;

use agent_scope_message::ToolResultState;
#[cfg(test)]
use agent_scope_message::{ToolOutput, ToolResultBlock};
use serde_json::Value as JsonValue;

use crate::make_text_result as make_result;
use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::{BuiltInToolContext, ToolErrorCategory};

/// Built-in `ResetTools` meta-tool.
///
/// Reads the authorized tool groups from the shared [`WorkspaceToolSession`]
/// to build a dynamic input schema, then applies the final-state activation
/// semantics through the session's `record_groups`.
pub struct ResetToolsTool {
    ctx: BuiltInToolContext,
}

impl ResetToolsTool {
    /// Create a new [`ResetToolsTool`] bound to a workspace context.
    #[must_use]
    pub fn new(ctx: BuiltInToolContext) -> Self {
        Self { ctx }
    }
}

#[async_trait::async_trait]
impl Tool for ResetToolsTool {
    fn name(&self) -> &str {
        "ResetTools"
    }

    fn description(&self) -> &str {
        "Reset or change the set of tools available to the agent for the \
         current task. Tools are organized into groups; each group can be \
         activated or deactivated by providing a boolean value. \
         \n\
         IMPORTANT: The boolean value you provide for each group is the \
         group's FINAL activation state, not an incremental change — any \
         group you do not explicitly set to true is deactivated, regardless \
         of its previous state. The `basic` group is always active. \
         Best practice: activate groups on demand and deactivate them when \
         no longer needed to save context. \
         \n\
         The response lists the instructions of the currently active tool \
         groups — you MUST read and follow them."
    }

    fn input_schema(&self) -> JsonValue {
        // Dynamic schema: one boolean field per authorized non-basic group.
        let mut properties = serde_json::Map::new();
        let session = self.ctx.session.read().unwrap_or_else(|e| e.into_inner());
        for group in session.authorized_groups() {
            properties.insert(
                group.clone(),
                serde_json::json!({
                    "type": "boolean",
                    "description": format!(
                        "Whether the '{group}' tool group is active (final state)."
                    ),
                    "default": false,
                }),
            );
        }
        serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": []
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let name = "ResetTools";

        // Snapshot the authorization boundary for validation.
        let authorized: BTreeSet<String> = match self.ctx.session.read() {
            Ok(guard) => guard.authorized_groups().cloned().collect(),
            Err(_) => BTreeSet::new(),
        };

        // Validate: every provided argument must be a boolean (FR-019
        // / contracts/reset-tools.md). Non-boolean → invalid_arguments.
        if let JsonValue::Object(map) = &input {
            for (key, value) in map {
                if !value.is_boolean() {
                    return Ok(ToolExecOutput::Complete(make_result(
                        name,
                        format!(
                            "Error: {}: invalid_arguments: the argument {key} should be a bool value",
                            ToolErrorCategory::ValidationFailure.as_str()
                        ),
                        ToolResultState::Error,
                    )));
                }
            }
        } else if !input.is_null() && !input.is_object() {
            return Ok(ToolExecOutput::Complete(make_result(
                name,
                format!(
                    "Error: {}: invalid_arguments: input must be a JSON object of group booleans",
                    ToolErrorCategory::ValidationFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Collect the requested active groups (final-state semantics).
        let requested: Vec<String> = if let JsonValue::Object(map) = &input {
            map.iter()
                .filter(|(_, v)| v.as_bool().unwrap_or(false))
                .map(|(k, _)| k.clone())
                .collect()
        } else {
            Vec::new()
        };

        // Authorization boundary (FR-019): never activate a group outside the
        // authorized set. An empty authorized set means no boundary is imposed.
        if !authorized.is_empty() {
            for group in &requested {
                if !authorized.contains(group) {
                    return Ok(ToolExecOutput::Complete(make_result(
                        name,
                        format!(
                            "Error: {}: permission_denied: tool group '{group}' is not authorized in this workspace",
                            ToolErrorCategory::PermissionDenied.as_str()
                        ),
                        ToolResultState::Error,
                    )));
                }
            }
        }

        // Apply the final-state activation through the session (intersects
        // with the authorized boundary again as defense in depth).
        let activated = match self.ctx.session.write() {
            Ok(mut guard) => guard.record_groups(requested),
            Err(_) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    name,
                    format!(
                        "Error: {}: internal_error: tool session is unavailable",
                        ToolErrorCategory::InternalFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // Render the activation result (contract: final activation state).
        let mut lines = Vec::new();
        lines.push("Tool groups reset. Final activation state:".to_string());
        if activated.is_empty() {
            lines.push("  - basic (always active)".to_string());
        } else {
            for group in &activated {
                lines.push(format!("  - {group}"));
            }
        }
        lines.push(
            "Groups not listed above are deactivated. Activate only what the \
             current task needs."
                .to_string(),
        );

        Ok(ToolExecOutput::Complete(make_result(
            name,
            lines.join("\n"),
            ToolResultState::Success,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::WorkspaceToolSession;
    use agent_scope_workspace::backend::{LocalBackend, WorkspaceBackend};
    use std::sync::{Arc, RwLock};

    /// Build a context rooted at a temp directory with authorized groups.
    fn ctx_with_groups(
        dir: &tempfile::TempDir,
        groups: &[&str],
    ) -> (BuiltInToolContext, Arc<RwLock<WorkspaceToolSession>>) {
        let workdir = dir.path().to_string_lossy().to_string();
        let backend: Arc<dyn WorkspaceBackend> = Arc::new(LocalBackend::new());
        let session = Arc::new(RwLock::new(WorkspaceToolSession::with_authorized_groups(
            "ws-1",
            groups.iter().map(|s| s.to_string()),
        )));
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
    async fn reset_tools_schema_has_authorized_groups() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_with_groups(&dir, &["coding", "docs"]);
        let tool = ResetToolsTool::new(ctx);
        let schema = tool.input_schema();
        assert!(schema["properties"]["coding"]["type"] == "boolean");
        assert!(schema["properties"]["docs"]["type"] == "boolean");
        // `basic` is never a dynamic field.
        assert!(schema["properties"].get("basic").is_none());
    }

    #[tokio::test]
    async fn reset_tools_activates_requested_groups_final_state() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, session) = ctx_with_groups(&dir, &["coding", "docs"]);
        let tool = ResetToolsTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "coding": true, "docs": false }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        assert!(text_of(&block).contains("coding"));

        // Final-state semantics: docs is deactivated, coding active.
        let guard = session.read().unwrap();
        assert!(guard.is_group_active("coding"));
        assert!(!guard.is_group_active("docs"));
    }

    #[tokio::test]
    async fn reset_tools_final_state_deactivates_previous() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, session) = ctx_with_groups(&dir, &["coding", "docs"]);
        let tool = ResetToolsTool::new(ctx);

        // First activate coding + docs.
        tool.call(serde_json::json!({ "coding": true, "docs": true }))
            .await
            .unwrap();
        // Then re-record with only docs → coding deactivated.
        tool.call(serde_json::json!({ "docs": true }))
            .await
            .unwrap();

        let guard = session.read().unwrap();
        assert!(!guard.is_group_active("coding"));
        assert!(guard.is_group_active("docs"));
    }

    #[tokio::test]
    async fn reset_tools_unauthorized_group_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, session) = ctx_with_groups(&dir, &["coding"]);
        let tool = ResetToolsTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "admin": true }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(
            text_of(&block).contains("permission_denied"),
            "got: {}",
            text_of(&block)
        );
        // Nothing was activated.
        assert!(!session.read().unwrap().is_group_active("admin"));
    }

    #[tokio::test]
    async fn reset_tools_non_bool_argument_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_with_groups(&dir, &["coding"]);
        let tool = ResetToolsTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "coding": "yes" }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(
            text_of(&block).contains("invalid_arguments"),
            "got: {}",
            text_of(&block)
        );
    }

    #[tokio::test]
    async fn reset_tools_empty_input_deactivates_all() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, session) = ctx_with_groups(&dir, &["coding", "docs"]);
        let tool = ResetToolsTool::new(ctx);

        tool.call(serde_json::json!({ "coding": true }))
            .await
            .unwrap();
        // Empty input → final state is nothing active.
        let out = tool.call(serde_json::json!({})).await.unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        assert!(text_of(&block).contains("basic"));

        let guard = session.read().unwrap();
        assert!(!guard.is_group_active("coding"));
        assert!(!guard.is_group_active("docs"));
        assert!(guard.is_group_active("basic"));
    }
}
