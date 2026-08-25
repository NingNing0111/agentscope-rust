//! WriteTool — built-in `Write` tool for writing files to the workspace.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_write.py` (upstream commit `9d1026fa`).

use std::path::Path;

use agent_scope_message::ToolResultState;
#[cfg(test)]
use agent_scope_message::{ToolOutput, ToolResultBlock};
use serde_json::Value as JsonValue;

use crate::make_text_result as make_result;
use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::{BuiltInToolContext, ToolErrorCategory};

/// Built-in `Write` tool.
///
/// Writes a file to the workspace filesystem. If the target already exists,
/// the read-before-overwrite guard requires that it was read earlier in the
/// session (via the `Read` tool), otherwise the write is rejected.
pub struct WriteTool {
    ctx: BuiltInToolContext,
    mode: WriteToolMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteToolMode {
    Legacy,
    Pi,
}

impl WriteTool {
    /// Create a new [`WriteTool`] bound to a workspace context.
    #[must_use]
    pub fn new(ctx: BuiltInToolContext) -> Self {
        Self {
            ctx,
            mode: WriteToolMode::Legacy,
        }
    }

    /// Create a pi-compatible lowercase `write` tool.
    #[must_use]
    pub fn new_pi(ctx: BuiltInToolContext) -> Self {
        Self {
            ctx,
            mode: WriteToolMode::Pi,
        }
    }
}

#[async_trait::async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        match self.mode {
            WriteToolMode::Legacy => "Write",
            WriteToolMode::Pi => "write",
        }
    }

    fn description(&self) -> &str {
        match self.mode {
            WriteToolMode::Legacy => {
                "Writes a file to the local filesystem.\n\
                 \n\
                 Usage:\n\
                 - This tool will overwrite the existing file if there is one at the provided path.\n\
                 - If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first.\n\
                 - ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.\n\
                 - NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.\n\
                 - Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked."
            }
            WriteToolMode::Pi => {
                "Writes UTF-8 text content to a file in the workspace. Use `path` and `content`. If the file already exists, it must have been read first with `read`; prefer `edit` for changes to existing files."
            }
        }
    }

    fn input_schema(&self) -> JsonValue {
        match self.mode {
            WriteToolMode::Legacy => serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to write (must be absolute, not relative)"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["file_path", "content"]
            }),
            WriteToolMode::Pi => serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to write."
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Compatibility alias for path."
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file."
                    }
                },
                "required": ["content"],
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
        // Extract required parameters.
        let path_key = if self.mode == WriteToolMode::Pi {
            "path"
        } else {
            "file_path"
        };
        let file_path = match input
            .get(path_key)
            .or_else(|| input.get("file_path"))
            .and_then(JsonValue::as_str)
        {
            Some(s) => s.to_string(),
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: invalid_arguments: missing required '{path_key}' parameter",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };
        let content = match input.get("content").and_then(JsonValue::as_str) {
            Some(s) => s.to_string(),
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: invalid_arguments: missing required 'content' parameter",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // Resolve the path inside the workspace.
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

        // Read-before-overwrite guard (FR-012): if the file exists, it must
        // have been read earlier in the session.
        let exists = match self.ctx.backend.file_exists(&path).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: execution: failed to check existence: {e}",
                        ToolErrorCategory::ExecutionFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };
        if exists {
            let is_read = match self.ctx.session.read() {
                Ok(guard) => guard.is_read(Path::new(&path)),
                Err(_) => false,
            };
            if !is_read {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: read_before_modify_required: File {file_path} exists but has not been read yet. You must read the file first before writing to it.",
                        ToolErrorCategory::PermissionDenied.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        }

        // Write the content (the backend creates parent directories).
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

        // Count lines in the content (mirrors Python `len(content.split("\n"))`).
        let line_count = content.split('\n').count();

        Ok(ToolExecOutput::Complete(make_result(
            self.name(),
            format!("The file {file_path} has been written successfully ({line_count} lines)."),
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
    async fn write_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let path = dir.path().join("new.txt").to_string_lossy().to_string();
        let tool = WriteTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": path,
                "content": "line1\nline2\n"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        assert!(text_of(&block).contains("has been written successfully"));
        // "line1\nline2\n" splits into 3 elements on '\n'.
        assert!(text_of(&block).contains("3 lines"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("new.txt")).unwrap(),
            "line1\nline2\n"
        );
    }

    #[tokio::test]
    async fn write_new_file_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let path = dir.path().join("a/b/c.txt").to_string_lossy().to_string();
        let tool = WriteTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": path,
                "content": "hi"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a/b/c.txt")).unwrap(),
            "hi"
        );
    }

    #[tokio::test]
    async fn write_overwrite_requires_read() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "old").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let path = file.to_string_lossy().to_string();
        let tool = WriteTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": path,
                "content": "new"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("read_before_modify_required"));
        // File unchanged.
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "old");
    }

    #[tokio::test]
    async fn write_overwrite_after_read() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "old").unwrap();
        let (ctx, session) = ctx_in(&dir);
        let path = file.to_string_lossy().to_string();
        mark_read(&session, &file);
        let tool = WriteTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": path,
                "content": "new content"
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new content");
    }

    #[tokio::test]
    async fn write_path_escape_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = WriteTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": "../a.txt",
                "content": "x"
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
