//! EditTool — built-in `Edit` tool for performing exact string replacements
//! in workspace files.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_edit.py` (upstream commit `9d1026fa`).

use std::path::Path;

#[cfg(test)]
use agent_scope_message::ToolOutput;
use agent_scope_message::{ToolResultBlock, ToolResultState};
use serde_json::Value as JsonValue;

use crate::make_text_result as make_result;
use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::{BuiltInToolContext, ToolErrorCategory};

const MAX_PI_EDITS: usize = 100;

/// Built-in `Edit` tool.
///
/// Performs exact string replacements in a file. Enforces the
/// read-before-modify guard: the target file must have been read earlier in
/// the session (via the `Read` tool), otherwise the edit is rejected.
pub struct EditTool {
    ctx: BuiltInToolContext,
    mode: EditToolMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditToolMode {
    Legacy,
    Pi,
}

impl EditTool {
    /// Create a new [`EditTool`] bound to a workspace context.
    #[must_use]
    pub fn new(ctx: BuiltInToolContext) -> Self {
        Self {
            ctx,
            mode: EditToolMode::Legacy,
        }
    }

    /// Create a pi-compatible lowercase `edit` tool.
    #[must_use]
    pub fn new_pi(ctx: BuiltInToolContext) -> Self {
        Self {
            ctx,
            mode: EditToolMode::Pi,
        }
    }
}

impl EditTool {
    fn apply_replacement(
        name: &str,
        content: String,
        path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> Result<(String, usize, bool), Box<ToolResultBlock>> {
        if old_string.is_empty() {
            return Err(Box::new(make_result(
                name,
                format!(
                    "Error: {}: invalid_arguments: old_string must not be empty",
                    ToolErrorCategory::ValidationFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        if old_string == new_string {
            return Err(Box::new(make_result(
                name,
                format!(
                    "Error: {}: invalid_arguments: old_string and new_string are identical. No changes to make.",
                    ToolErrorCategory::ValidationFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        let occurrences = content.matches(old_string).count();
        if occurrences == 0 {
            return Err(Box::new(make_result(
                name,
                format!(
                    "Error: {}: pattern_not_found: old_string not found in {path}",
                    ToolErrorCategory::ValidationFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }
        if occurrences > 1 && !replace_all {
            return Err(Box::new(make_result(
                name,
                format!(
                    "Error: {}: ambiguous_edit: old_string appears {occurrences} times in {path}. Set replace_all=true to replace all occurrences, or make old_string more specific.",
                    ToolErrorCategory::ValidationFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        let updated = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };
        Ok((updated, occurrences, replace_all))
    }

    async fn call_impl(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let (file_path, edits) = match self.mode {
            EditToolMode::Legacy => {
                let file_path = match input.get("file_path").and_then(JsonValue::as_str) {
                    Some(s) => s.to_string(),
                    None => {
                        return Ok(ToolExecOutput::Complete(make_result(
                            self.name(),
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
                            self.name(),
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
                            self.name(),
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
                (file_path, vec![(old_string, new_string, replace_all)])
            }
            EditToolMode::Pi => {
                let file_path = match input
                    .get("path")
                    .or_else(|| input.get("file_path"))
                    .and_then(JsonValue::as_str)
                {
                    Some(s) => s.to_string(),
                    None => {
                        return Ok(ToolExecOutput::Complete(make_result(
                            self.name(),
                            format!(
                                "Error: {}: invalid_arguments: missing required 'path' parameter",
                                ToolErrorCategory::ValidationFailure.as_str()
                            ),
                            ToolResultState::Error,
                        )));
                    }
                };
                let raw_edits = match input.get("edits").and_then(JsonValue::as_array) {
                    Some(edits) if !edits.is_empty() => edits,
                    Some(_) => {
                        return Ok(ToolExecOutput::Complete(make_result(
                            self.name(),
                            format!(
                                "Error: {}: invalid_arguments: edits must not be empty",
                                ToolErrorCategory::ValidationFailure.as_str()
                            ),
                            ToolResultState::Error,
                        )));
                    }
                    None => {
                        return Ok(ToolExecOutput::Complete(make_result(
                            self.name(),
                            format!(
                                "Error: {}: invalid_arguments: missing required 'edits' parameter",
                                ToolErrorCategory::ValidationFailure.as_str()
                            ),
                            ToolResultState::Error,
                        )));
                    }
                };
                if raw_edits.len() > MAX_PI_EDITS {
                    return Ok(ToolExecOutput::Complete(make_result(
                        self.name(),
                        format!(
                            "Error: {}: invalid_arguments: edits must contain at most {MAX_PI_EDITS} items",
                            ToolErrorCategory::ValidationFailure.as_str()
                        ),
                        ToolResultState::Error,
                    )));
                }
                let mut parsed = Vec::with_capacity(raw_edits.len());
                for (idx, edit) in raw_edits.iter().enumerate() {
                    let old_text = match edit.get("oldText").and_then(JsonValue::as_str) {
                        Some(s) => s.to_string(),
                        None => {
                            return Ok(ToolExecOutput::Complete(make_result(
                                self.name(),
                                format!(
                                    "Error: {}: invalid_arguments: edits[{idx}].oldText is required",
                                    ToolErrorCategory::ValidationFailure.as_str()
                                ),
                                ToolResultState::Error,
                            )));
                        }
                    };
                    let new_text = match edit.get("newText").and_then(JsonValue::as_str) {
                        Some(s) => s.to_string(),
                        None => {
                            return Ok(ToolExecOutput::Complete(make_result(
                                self.name(),
                                format!(
                                    "Error: {}: invalid_arguments: edits[{idx}].newText is required",
                                    ToolErrorCategory::ValidationFailure.as_str()
                                ),
                                ToolResultState::Error,
                            )));
                        }
                    };
                    parsed.push((old_text, new_text, false));
                }
                (file_path, parsed)
            }
        };

        let path = match self.ctx.resolve_in_workspace(&file_path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: path_outside_workspace: {e}",
                        ToolErrorCategory::PermissionDenied.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        let exists = match self.ctx.backend.file_exists(&path).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
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
                self.name(),
                format!(
                    "Error: {}: file_not_found: File not found: {path}",
                    ToolErrorCategory::ExecutionFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        let is_read = match self.ctx.session.read() {
            Ok(guard) => guard.is_read(Path::new(&path)),
            Err(_) => false,
        };
        if !is_read {
            return Ok(ToolExecOutput::Complete(make_result(
                self.name(),
                format!(
                    "Error: {}: read_before_modify_required: To edit a file, you must first read it using the Read tool.",
                    ToolErrorCategory::PermissionDenied.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        let bytes = match self.ctx.backend.read_file(&path).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: file_not_found: Error reading file: {e}",
                        ToolErrorCategory::ExecutionFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };
        let mut content = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: unsupported_file_type: file is not valid UTF-8: {path}",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        let mut total_replacements = 0usize;
        let mut legacy_replace_all = false;
        for (old_text, new_text, replace_all) in edits {
            match Self::apply_replacement(
                self.name(),
                content,
                &path,
                &old_text,
                &new_text,
                replace_all,
            ) {
                Ok((updated, occurrences, did_replace_all)) => {
                    content = updated;
                    total_replacements += if did_replace_all { occurrences } else { 1 };
                    legacy_replace_all = did_replace_all;
                }
                Err(block) => return Ok(ToolExecOutput::Complete(*block)),
            }
        }

        if let Err(e) = self.ctx.backend.write_file(&path, content.as_bytes()).await {
            return Ok(ToolExecOutput::Complete(make_result(
                self.name(),
                format!(
                    "Error: {}: execution: Error writing file: {e}",
                    ToolErrorCategory::ExecutionFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        let replacement_msg = if self.mode == EditToolMode::Legacy && legacy_replace_all {
            format!("all {total_replacements} occurrences")
        } else if total_replacements == 1 {
            "1 occurrence".to_string()
        } else {
            format!("{total_replacements} occurrences")
        };
        Ok(ToolExecOutput::Complete(make_result(
            self.name(),
            format!("Successfully replaced {replacement_msg} in {path}"),
            ToolResultState::Success,
        )))
    }
}

#[async_trait::async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        match self.mode {
            EditToolMode::Legacy => "Edit",
            EditToolMode::Pi => "edit",
        }
    }

    fn description(&self) -> &str {
        match self.mode {
            EditToolMode::Legacy => {
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
            EditToolMode::Pi => {
                "Performs exact string replacements in a workspace file. Use `path` and `edits`, where each edit has `oldText` and `newText`. Edits are validated and applied in memory first, then written once; the file must have been read first with `read`."
            }
        }
    }

    fn input_schema(&self) -> JsonValue {
        match self.mode {
            EditToolMode::Legacy => serde_json::json!({
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
            }),
            EditToolMode::Pi => serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to edit."
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Compatibility alias for path."
                    },
                    "edits": {
                        "type": "array",
                        "description": "Ordered exact replacements to apply. Each oldText must match exactly and uniquely in the file content at the time that edit is applied.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "oldText": {
                                    "type": "string",
                                    "description": "The exact text to replace."
                                },
                                "newText": {
                                    "type": "string",
                                    "description": "The replacement text."
                                }
                            },
                            "required": ["oldText", "newText"]
                        },
                        "minItems": 1,
                        "maxItems": MAX_PI_EDITS
                    }
                },
                "required": ["edits"],
                "anyOf": [
                    { "required": ["path"] },
                    { "required": ["file_path"] }
                ]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        false
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        self.call_impl(input).await
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
