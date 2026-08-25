//! ReadTool — built-in `Read` tool for reading files from the workspace.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_read.py` (upstream commit `9d1026fa`).

use std::path::Path;

use agent_scope_message::ToolResultState;
#[cfg(test)]
use agent_scope_message::{ToolOutput, ToolResultBlock};
use serde_json::Value as JsonValue;

use crate::make_text_result as make_result;
use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::{BuiltInToolContext, ToolErrorCategory};

/// Maximum number of characters shown per line before truncation (mirrors
/// Python `max_line_characters=2000`).
const MAX_LINE_CHARACTERS: usize = 2000;
/// Default maximum number of lines read in one call (Python `limit=2000`).
const DEFAULT_LIMIT: usize = 2000;

/// Built-in `Read` tool.
///
/// Reads a file from the local filesystem and returns its contents in
/// `cat -n` format (6-char padded line number + tab + content), applying
/// optional `offset`/`limit` slicing. Successful reads are recorded in the
/// session's read-state so `Edit`/`Write` can enforce their read-before-
/// modify guard.
pub struct ReadTool {
    ctx: BuiltInToolContext,
    mode: ReadToolMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadToolMode {
    Legacy,
    Pi,
}

impl ReadTool {
    /// Create a new [`ReadTool`] bound to a workspace context.
    #[must_use]
    pub fn new(ctx: BuiltInToolContext) -> Self {
        Self {
            ctx,
            mode: ReadToolMode::Legacy,
        }
    }

    /// Create a pi-compatible lowercase `read` tool.
    #[must_use]
    pub fn new_pi(ctx: BuiltInToolContext) -> Self {
        Self {
            ctx,
            mode: ReadToolMode::Pi,
        }
    }
}

/// Truncate a line longer than `max_chars`, appending a `[truncated]` suffix
/// (mirrors Python `max_line_characters` behaviour).
fn truncate_line(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        line.to_string()
    } else {
        let mut truncated: String = line.chars().take(max_chars).collect();
        truncated.push_str("[truncated]");
        truncated
    }
}

#[async_trait::async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        match self.mode {
            ReadToolMode::Legacy => "Read",
            ReadToolMode::Pi => "read",
        }
    }

    fn description(&self) -> &str {
        match self.mode {
            ReadToolMode::Legacy => {
                "Reads a UTF-8 text file from the workspace filesystem.\n\
                 \n\
                 Usage:\n\
                 - The file_path parameter may be absolute or relative to the workspace root.\n\
                 - By default, it reads up to 2000 lines starting from the beginning of the file.\n\
                 - You can optionally specify a line offset and limit for long files.\n\
                 - Results are returned using cat -n format, with line numbers starting at 1.\n\
                 - Binary files, images, and PDFs are not decoded by this implementation; non-UTF-8 content returns an explicit error."
            }
            ReadToolMode::Pi => {
                "Reads a UTF-8 text file from the workspace. Use `path` for the file path (relative to the workspace root or absolute inside it). Results are returned with line numbers. Successful reads authorize subsequent `edit`/`write` changes to existing files."
            }
        }
    }

    fn input_schema(&self) -> JsonValue {
        match self.mode {
            ReadToolMode::Legacy => serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the UTF-8 text file to read."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Optional 1-based line number to start reading from (default: 1)",
                        "default": 1,
                        "minimum": 1
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Optional maximum number of lines to read (default: 2000, max: 2000)",
                        "default": 2000,
                        "maximum": 2000,
                        "minimum": 1
                    }
                },
                "required": ["file_path"]
            }),
            ReadToolMode::Pi => serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the UTF-8 text file to read."
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Compatibility alias for path."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Optional 1-based line number to start reading from (default: 1)",
                        "default": 1,
                        "minimum": 1
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Optional maximum number of lines to read (default: 2000, max: 2000)",
                        "default": 2000,
                        "maximum": 2000,
                        "minimum": 1
                    }
                },
                "anyOf": [
                    { "required": ["path"] },
                    { "required": ["file_path"] }
                ]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        // Extract the required file path parameter.
        let path_key = if self.mode == ReadToolMode::Pi {
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

        // Resolve the path inside the workspace (lexical containment).
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

        // Extract optional offset/limit with sane defaults and clamping.
        let offset = input
            .get("offset")
            .and_then(JsonValue::as_i64)
            .unwrap_or(1)
            .max(1) as usize;
        let limit = input
            .get("limit")
            .and_then(JsonValue::as_i64)
            .unwrap_or(DEFAULT_LIMIT as i64)
            .clamp(1, DEFAULT_LIMIT as i64) as usize;

        // The file must exist.
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
                    "Error: {}: file_not_found: File does not exist: {path}",
                    ToolErrorCategory::ExecutionFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // It must be a regular file, not a directory.
        let is_dir = match self.ctx.backend.is_dir(&path).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: unsupported_file_type: {e}",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };
        if is_dir {
            return Ok(ToolExecOutput::Complete(make_result(
                self.name(),
                format!(
                    "Error: {}: unsupported_file_type: Path is a directory, not a file: {path}",
                    ToolErrorCategory::ValidationFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Read raw bytes through the backend.
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

        // Decode as UTF-8.
        let content = match String::from_utf8(bytes) {
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

        // Record the successful read in the session read-state (the
        // read-before-modify guard premise for Edit/Write).
        if let Ok(mut session) = self.ctx.session.write() {
            session.record_read(Path::new(&path));
        }

        // Split into logical lines, dropping the empty tail after a trailing
        // newline (mirrors Python `splitlines(keepends=True)`).
        let lines: Vec<&str> = content.split('\n').collect();
        let line_count = if content.is_empty() {
            0
        } else if content.ends_with('\n') {
            lines.len() - 1
        } else {
            lines.len()
        };

        // Slice by offset/limit and format in `cat -n` style.
        let start = offset.saturating_sub(1);
        let end = (start + limit).min(line_count);
        let mut formatted = Vec::with_capacity(end.saturating_sub(start));
        for (idx, line) in lines[start..end].iter().enumerate() {
            let raw = line.trim_end_matches('\r');
            let rendered = truncate_line(raw, MAX_LINE_CHARACTERS);
            formatted.push(format!("{:6}\t{rendered}", start + idx + 1));
        }
        let result = formatted.join("\n");

        Ok(ToolExecOutput::Complete(make_result(
            self.name(),
            result,
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

    /// Build a context rooted at a temp directory. Returns the context, the
    /// shared session, and the workdir string.
    fn ctx_in(
        dir: &tempfile::TempDir,
    ) -> (
        BuiltInToolContext,
        Arc<RwLock<WorkspaceToolSession>>,
        String,
    ) {
        let workdir = dir.path().to_string_lossy().to_string();
        let backend: Arc<dyn WorkspaceBackend> = Arc::new(LocalBackend::new());
        let session = Arc::new(RwLock::new(WorkspaceToolSession::new("ws-1")));
        let ctx = BuiltInToolContext::new(backend, workdir.clone(), session.clone());
        (ctx, session, workdir)
    }

    fn text_of(block: &ToolResultBlock) -> String {
        match &block.output {
            ToolOutput::Text(t) => t.clone(),
            _ => String::new(),
        }
    }

    #[tokio::test]
    async fn read_success_records_session() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello\nworld\n").unwrap();
        let (ctx, session, workdir) = ctx_in(&dir);
        let tool = ReadTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "file_path": "a.txt" }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(text.contains("     1\thello"), "got: {text}");
        assert!(text.contains("     2\tworld"), "got: {text}");

        // The successful read is recorded in the session read-state.
        let expected = std::path::Path::new(&workdir).join("a.txt");
        assert!(session.read().unwrap().is_read(&expected));
    }

    #[tokio::test]
    async fn read_offset_limit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "l1\nl2\nl3\nl4\nl5\n").unwrap();
        let (ctx, _, _) = ctx_in(&dir);
        let tool = ReadTool::new(ctx);

        let out = tool
            .call(serde_json::json!({
                "file_path": "a.txt",
                "offset": 2,
                "limit": 2
            }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(text.contains("     2\tl2"), "got: {text}");
        assert!(text.contains("     3\tl3"), "got: {text}");
        assert!(!text.contains("l1"), "got: {text}");
        assert!(!text.contains("l4"), "got: {text}");
    }

    #[tokio::test]
    async fn read_long_line_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let long = "x".repeat(3000);
        std::fs::write(dir.path().join("a.txt"), format!("{long}\n")).unwrap();
        let (ctx, _, _) = ctx_in(&dir);
        let tool = ReadTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "file_path": "a.txt" }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(
            text.contains("[truncated]"),
            "got first 100: {}",
            &text[..100]
        );
        assert!(!text.contains(&"x".repeat(3000)));
    }

    #[tokio::test]
    async fn read_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _, _) = ctx_in(&dir);
        let tool = ReadTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "file_path": "missing.txt" }))
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
    async fn read_directory_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let (ctx, _, _) = ctx_in(&dir);
        let tool = ReadTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "file_path": "sub" }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("unsupported_file_type"));
    }

    #[tokio::test]
    async fn read_path_escape_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _, _) = ctx_in(&dir);
        let tool = ReadTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "file_path": "../a.txt" }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("path_outside_workspace"));
    }

    #[tokio::test]
    async fn read_non_utf8_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bin.dat"), [0xffu8, 0xfe, 0x00, 0x01]).unwrap();
        let (ctx, _, _) = ctx_in(&dir);
        let tool = ReadTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "file_path": "bin.dat" }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("unsupported_file_type"));
    }

    #[tokio::test]
    async fn read_missing_parameter() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _, _) = ctx_in(&dir);
        let tool = ReadTool::new(ctx);

        let out = tool.call(serde_json::json!({})).await.unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(text_of(&block).contains("invalid_arguments"));
    }
}
