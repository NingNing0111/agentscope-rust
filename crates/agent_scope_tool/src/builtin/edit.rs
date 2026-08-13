//! EditTool — built-in `Edit` tool for performing exact string replacements
//! in workspace files.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_edit.py` (upstream commit `9d1026fa`).

use std::path::Path;

use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use serde_json::Value as JsonValue;

use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::{BuiltInToolContext, ToolErrorCategory};

/// Built-in `Edit` tool.
///
/// Performs exact string replacements in a file. Enforces the
/// read-before-modify guard: the target file must have been read earlier in
/// the session (via the `Read` tool), otherwise the edit is rejected.
pub struct EditTool {
    ctx: BuiltInToolContext,
}

impl EditTool {
    /// Create a new [`EditTool`] bound to a workspace context.
    #[must_use]
    pub fn new(ctx: BuiltInToolContext) -> Self {
        Self { ctx }
    }
}

/// Build a complete one-shot [`ToolResultBlock`].
fn make_result(name: &str, text: String, state: ToolResultState) -> ToolResultBlock {
    ToolResultBlock {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        output: ToolOutput::Text(text),
        state,
        metadata: std::collections::HashMap::new(),
        created_at: chrono::Utc::now().to_rfc3339(),
        finished_at: Some(chrono::Utc::now().to_rfc3339()),
        is_last: true,
    }
}

#[async_trait::async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn description(&self) -> &str {
        "Performs exact string replacements in files.\n\
         \n\
         Usage:\n\
         - You must use your `Read` tool at least once in the conversation\n\
           before editing. This tool will error if you attempt an edit without\n\
           reading the file.\n\
         - When editing text from Read tool output, ensure you preserve the\n\
           exact indentation (tabs/spaces) as it appears AFTER the line number\n\
           prefix. The line number prefix format is: line number + tab.\n\
           Everything after that is the actual file content to match. Never\n\
           include any part of the line number prefix in the old_string or\n\
           new_string.\n\
         - ALWAYS prefer editing existing files in the codebase. NEVER write\n\
           new files unless explicitly required.\n\
         - Only use emojis if the user explicitly requests it. Avoid adding\n\
           emojis to files unless asked.\n\
         - The edit will FAIL if `old_string` is not unique in the file."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to edit."
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to replace. Must match exactly including whitespace and indentation."
                },
                "new_string": {
                    "type": "string",
                    "description": "The string to replace old_string with."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "If true, replace all occurrences. If false (default), only replace if there is exactly one occurrence.",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        false
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        // Extract required parameters.
        let file_path = match input.get("file_path").and_then(JsonValue::as_str) {
            Some(s) => s.to_string(),
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Edit",
                    format!(
                        "Error: {}: invalid_arguments: missing required 'file_path' parameter",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };
        let old_string = match input.get("old_string").and_then(JsonValue::as_str) {
            Some(s) => s.to_string(),
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Edit",
                    format!(
                        "Error: {}: invalid_arguments: missing required 'old_string' parameter",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };
        let new_string = match input.get("new_string").and_then(JsonValue::as_str) {
            Some(s) => s.to_string(),
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Edit",
                    format!(
                        "Error: {}: invalid_arguments: missing required 'new_string' parameter",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };
        let replace_all = input
            .get("replace_all")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);

        // Resolve the path inside the workspace.
        let path = match self.ctx.resolve_in_workspace(&file_path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Edit",
                    format!(
                        "Error: {}: path_outside_workspace: {e}",
                        ToolErrorCategory::PermissionDenied.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // The file must exist.
        let exists = match self.ctx.backend.file_exists(&path).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Edit",
                    format!(
                        "Error: {}: file_not_found: {e}",
                        ToolErrorCategory::ExecutionFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };
        if !exists {
            return Ok(ToolExecOutput::Complete(make_result(
                "Edit",
                format!(
                    "Error: {}: file_not_found: File not found: {path}",
                    ToolErrorCategory::ExecutionFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Read-before-modify guard (FR-008): the file must have been read
        // earlier in the session.
        let is_read = match self.ctx.session.read() {
            Ok(guard) => guard.is_read(Path::new(&path)),
            Err(_) => false,
        };
        if !is_read {
            return Ok(ToolExecOutput::Complete(make_result(
                "Edit",
                format!(
                    "Error: {}: read_before_modify_required: To edit a file, you must first read it using the Read tool.",
                    ToolErrorCategory::PermissionDenied.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Validation: old_string must be non-empty.
        if old_string.is_empty() {
            return Ok(ToolExecOutput::Complete(make_result(
                "Edit",
                format!(
                    "Error: {}: invalid_arguments: old_string must not be empty",
                    ToolErrorCategory::ValidationFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Validation: old_string and new_string must differ.
        if old_string == new_string {
            return Ok(ToolExecOutput::Complete(make_result(
                "Edit",
                format!(
                    "Error: {}: invalid_arguments: old_string and new_string are identical. No changes to make.",
                    ToolErrorCategory::ValidationFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Read the current file contents.
        let bytes = match self.ctx.backend.read_file(&path).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Edit",
                    format!(
                        "Error: {}: file_not_found: Error reading file: {e}",
                        ToolErrorCategory::ExecutionFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };
        let content = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Edit",
                    format!(
                        "Error: {}: unsupported_file_type: file is not valid UTF-8: {path}",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // Count occurrences.
        let occurrences = content.matches(&old_string).count();

        // Zero occurrences → pattern not found.
        if occurrences == 0 {
            return Ok(ToolExecOutput::Complete(make_result(
                "Edit",
                format!(
                    "Error: {}: pattern_not_found: old_string not found in {path}",
                    ToolErrorCategory::ValidationFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Multiple occurrences without replace_all → ambiguous edit.
        if occurrences > 1 && !replace_all {
            return Ok(ToolExecOutput::Complete(make_result(
                "Edit",
                format!(
                    "Error: {}: ambiguous_edit: old_string appears {occurrences} times in {path}. Set replace_all=true to replace all occurrences, or make old_string more specific.",
                    ToolErrorCategory::ValidationFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Perform the replacement.
        let updated = if replace_all {
            content.replace(&old_string, &new_string)
        } else {
            content.replacen(&old_string, &new_string, 1)
        };

        // Write the updated content back.
        if let Err(e) = self.ctx.backend.write_file(&path, updated.as_bytes()).await {
            return Ok(ToolExecOutput::Complete(make_result(
                "Edit",
                format!(
                    "Error: {}: execution: Error writing file: {e}",
                    ToolErrorCategory::ExecutionFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Success message, mirroring Python.
        let replacement_msg = if replace_all {
            format!("all {occurrences} occurrences")
        } else {
            "1 occurrence".to_string()
        };
        Ok(ToolExecOutput::Complete(make_result(
            "Edit",
            format!("Successfully replaced {replacement_msg} in {path}"),
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

    /// Build a context rooted at a temp directory. Returns the context and
    /// the shared session.
    fn ctx_in(dir: &tempfile::TempDir) -> (BuiltInToolContext, Arc<RwLock<WorkspaceToolSession>>) {
        let workdir = dir.path().to_string_lossy().to_string();
        let backend: Arc<dyn WorkspaceBackend> = Arc::new(LocalBackend::new());
        let session = Arc::new(RwLock::new(WorkspaceToolSession::new("ws-1")));
        let ctx = BuiltInToolContext::new(backend, workdir, session.clone());
        (ctx, session)
    }

    /// Mark the given file as read in the session so the guard passes.
    fn mark_read(session: &Arc<RwLock<WorkspaceToolSession>>, path: &std::path::Path) {
        session.write().unwrap().record_read(path);
    }

    fn text_of(block: &ToolResultBlock) -> String {
        match &block.output {
            ToolOutput::Text(t) => t.clone(),
            _ => String::new(),
        }
    }

    #[tokio::test]
    async fn edit_single_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "hello world\n").unwrap();
        let (ctx, session) = ctx_in(&dir);
        let path = file.to_string_lossy().to_string();
        mark_read(&session, &file);
        let tool = EditTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": path,
                "old_string": "hello",
                "new_string": "goodbye"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        assert!(text_of(&block).contains("Successfully replaced 1 occurrence"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "goodbye world\n");
    }

    #[tokio::test]
    async fn edit_requires_read_first() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "hello\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let path = file.to_string_lossy().to_string();
        let tool = EditTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": path,
                "old_string": "hello",
                "new_string": "goodbye"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("read_before_modify_required"));
    }

    #[tokio::test]
    async fn edit_replace_all() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x y x y x\n").unwrap();
        let (ctx, session) = ctx_in(&dir);
        let path = file.to_string_lossy().to_string();
        mark_read(&session, &file);
        let tool = EditTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": path,
                "old_string": "x",
                "new_string": "z",
                "replace_all": true
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        assert!(text_of(&block).contains("Successfully replaced all 3 occurrences"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "z y z y z\n");
    }

    #[tokio::test]
    async fn edit_ambiguous_without_replace_all() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x y x\n").unwrap();
        let (ctx, session) = ctx_in(&dir);
        let path = file.to_string_lossy().to_string();
        mark_read(&session, &file);
        let tool = EditTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": path,
                "old_string": "x",
                "new_string": "z"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("ambiguous_edit"));
    }

    #[tokio::test]
    async fn edit_pattern_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "hello\n").unwrap();
        let (ctx, session) = ctx_in(&dir);
        let path = file.to_string_lossy().to_string();
        mark_read(&session, &file);
        let tool = EditTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": path,
                "old_string": "missing",
                "new_string": "x"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("pattern_not_found"));
    }

    #[tokio::test]
    async fn edit_identical_strings_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "hello\n").unwrap();
        let (ctx, session) = ctx_in(&dir);
        let path = file.to_string_lossy().to_string();
        mark_read(&session, &file);
        let tool = EditTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": path,
                "old_string": "hello",
                "new_string": "hello"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("invalid_arguments"));
    }

    #[tokio::test]
    async fn edit_file_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, session) = ctx_in(&dir);
        let missing = dir.path().join("missing.txt").to_string_lossy().to_string();
        // A missing file can never have been read — guard returns file_not_found.
        let _ = session;
        let tool = EditTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": missing,
                "old_string": "x",
                "new_string": "y"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("file_not_found"));
    }

    #[tokio::test]
    async fn edit_path_escape_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = EditTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": "../a.txt",
                "old_string": "x",
                "new_string": "y"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("path_outside_workspace"));
    }
}
