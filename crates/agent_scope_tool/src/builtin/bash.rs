//! BashTool — built-in `Bash` tool for executing shell commands inside the
//! workspace.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_bash.py` (upstream commit `9d1026fa`).

use agent_scope_message::ToolResultState;
#[cfg(test)]
use agent_scope_message::{ToolOutput, ToolResultBlock};
use agent_scope_utils::command::{
    TimeoutUnit, command_timeout, format_command_output, truncate_chars,
};
use serde_json::Value as JsonValue;

use crate::make_text_result as make_result;
use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::{BuiltInToolContext, ToolErrorCategory};

/// Default command timeout in milliseconds (Python `timeout=120000`).
const DEFAULT_TIMEOUT_MS: i64 = 120_000;
/// Upper bound for a command timeout in milliseconds (Python max 600000).
const MAX_TIMEOUT_MS: i64 = 600_000;
/// Maximum number of characters returned to the model before truncation.
const MAX_OUTPUT_CHARS: usize = 30_000;

/// Built-in `Bash` tool.
///
/// Executes a shell command through the workspace backend. The command is a
/// full shell command line wrapped in the platform's native shell (`/bin/sh -c`
/// on POSIX, `cmd /c` on Windows) because the backend primitive runs the argv
/// array directly without a shell. All process I/O is confined to the
/// workspace `workdir` via the backend.
pub struct BashTool {
    ctx: BuiltInToolContext,
    mode: BashToolMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BashToolMode {
    Legacy,
    Pi,
}

impl BashTool {
    /// Create a new [`BashTool`] bound to a workspace context.
    #[must_use]
    pub fn new(ctx: BuiltInToolContext) -> Self {
        Self {
            ctx,
            mode: BashToolMode::Legacy,
        }
    }

    /// Create a pi-compatible lowercase `bash` tool.
    #[must_use]
    pub fn new_pi(ctx: BuiltInToolContext) -> Self {
        Self {
            ctx,
            mode: BashToolMode::Pi,
        }
    }

    fn timeout_unit(&self) -> TimeoutUnit {
        match self.mode {
            BashToolMode::Legacy => TimeoutUnit::Milliseconds,
            BashToolMode::Pi => TimeoutUnit::Seconds,
        }
    }
}

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        match self.mode {
            BashToolMode::Legacy => "Bash",
            BashToolMode::Pi => "bash",
        }
    }

    fn description(&self) -> &str {
        match self.mode {
            BashToolMode::Legacy => {
                "Executes a bash command and returns its output.\n\
                 \n\
                 The working directory persists between commands, but shell state does not.\n\
                 \n\
                 IMPORTANT: Avoid using this tool to run `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, or `echo` commands, unless explicitly instructed or after you have verified that a dedicated tool cannot accomplish your task. Instead, use the appropriate dedicated tool as this will provide a much better experience for the user:\n\
                 \n\
                 - File search: Use Glob (NOT find or ls)\n\
                 - Content search: Use Grep (NOT grep or rg)\n\
                 - Read files: Use Read (NOT cat/head/tail)\n\
                 - Edit files: Use Edit (NOT sed/awk)\n\
                 - Write files: Use Write (NOT echo >/cat <<EOF)\n\
                 - Communication: Output text directly (NOT echo/printf)\n\
                 \n\
                 While the Bash tool can do similar things, it's better to use the built-in tools as they provide a better user experience and make it easier to review tool calls and give permission.\n\
                 \n\
                 You may specify an optional timeout in milliseconds (up to 600000ms / 10 minutes). By default, your command will timeout after 120000ms (2 minutes)."
            }
            BashToolMode::Pi => {
                "Executes a shell command in the workspace and returns its output. Prefer dedicated tools for filesystem work: `find`/`ls` for discovery, `grep` for content search, `read` for reading files, `edit` for changes, and `write` for new files. The optional timeout is in seconds (default: 120, max: 600)."
            }
        }
    }

    fn input_schema(&self) -> JsonValue {
        match self.mode {
            BashToolMode::Legacy => serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute."
                    },
                    "description": {
                        "type": "string",
                        "description": "Clear, concise description of what this command does."
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Optional timeout in milliseconds (default: 120000, max: 600000)",
                        "default": 120000,
                        "maximum": 600000,
                        "minimum": 0
                    }
                },
                "required": ["command"]
            }),
            BashToolMode::Pi => serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute."
                    },
                    "description": {
                        "type": "string",
                        "description": "Clear, concise description of what this command does."
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Optional timeout in seconds (default: 120, max: 600)",
                        "default": 120,
                        "maximum": 600,
                        "minimum": 0
                    }
                },
                "required": ["command"]
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
        // Extract the required, non-empty `command` parameter.
        let command = match input.get("command").and_then(JsonValue::as_str) {
            Some(s) if !s.is_empty() => s.to_string(),
            Some(_) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: invalid_arguments: command must not be empty",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: invalid_arguments: missing required 'command' parameter",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        let timeout = command_timeout(
            input.get("timeout").and_then(JsonValue::as_i64),
            self.timeout_unit(),
            DEFAULT_TIMEOUT_MS,
            MAX_TIMEOUT_MS,
        );

        // `command` is a full shell command line (it may contain pipes,
        // redirects, `&&`, …), so wrap it in the platform's native shell. The
        // backend primitive runs the argv array directly without a shell.
        let shell_cmd: Vec<&str> = if std::env::consts::OS == "windows" {
            vec!["cmd", "/c", command.as_str()]
        } else {
            vec!["/bin/sh", "-c", command.as_str()]
        };

        // cwd is the workspace root itself (backend-controlled), not a
        // resolved path — the command runs inside the workspace boundary.
        let result = match self
            .ctx
            .backend
            .exec_shell(&shell_cmd, &self.ctx.workdir, Some(timeout.seconds))
            .await
        {
            Ok(out) => out,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: command_failed: Command failed: {command}\nError: {e}",
                        ToolErrorCategory::ExecutionFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // Backend timeout convention: `exit_code == -1` signals the command was
        // killed after exceeding its configured timeout window.
        if result.exit_code == -1 {
            return Ok(ToolExecOutput::Complete(make_result(
                self.name(),
                format!(
                    "Error: {}: command_timeout: Command timed out after {}: {command}",
                    ToolErrorCategory::Timeout.as_str(),
                    timeout.display
                ),
                ToolResultState::Error,
            )));
        }

        let output = format_command_output(&result.stdout, &result.stderr, MAX_OUTPUT_CHARS);

        if !result.ok() {
            let stdout_text = String::from_utf8_lossy(&result.stdout).replace("\r\n", "\n");
            let stderr_text = String::from_utf8_lossy(&result.stderr).replace("\r\n", "\n");
            let mut error = format!("Command failed: {command}\n");
            if !stdout_text.is_empty() {
                error.push_str(&format!("\nStdout:\n{stdout_text}"));
            }
            if !stderr_text.is_empty() {
                error.push_str(&format!("\nStderr:\n{stderr_text}"));
            }
            return Ok(ToolExecOutput::Complete(make_result(
                self.name(),
                format!(
                    "Error: {}: command_failed: {}",
                    ToolErrorCategory::ExecutionFailure.as_str(),
                    truncate_chars(error, MAX_OUTPUT_CHARS, "\n... (output truncated)")
                ),
                ToolResultState::Error,
            )));
        }

        Ok(ToolExecOutput::Complete(make_result(
            self.name(),
            output,
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

    /// Build a context rooted at a temp directory. Returns the context and the
    /// workdir string.
    fn ctx_in(dir: &tempfile::TempDir) -> (BuiltInToolContext, String) {
        let workdir = dir.path().to_string_lossy().to_string();
        let backend: Arc<dyn WorkspaceBackend> = Arc::new(LocalBackend::new());
        let session = Arc::new(RwLock::new(WorkspaceToolSession::new("ws-1")));
        let ctx = BuiltInToolContext::new(backend, workdir.clone(), session);
        (ctx, workdir)
    }

    fn text_of(block: &ToolResultBlock) -> String {
        match &block.output {
            ToolOutput::Text(t) => t.clone(),
            _ => String::new(),
        }
    }

    #[test]
    fn timeout_clamped_to_max() {
        assert_eq!(
            command_timeout(
                None,
                TimeoutUnit::Milliseconds,
                DEFAULT_TIMEOUT_MS,
                MAX_TIMEOUT_MS
            )
            .display,
            "120000ms"
        );
        assert_eq!(
            command_timeout(
                Some(900_000),
                TimeoutUnit::Milliseconds,
                DEFAULT_TIMEOUT_MS,
                MAX_TIMEOUT_MS
            )
            .display,
            "600000ms"
        );
    }

    #[tokio::test]
    async fn bash_echo_success() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = BashTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "command": "echo hello" }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Success);
        assert!(
            text_of(&block).contains("hello"),
            "got: {}",
            text_of(&block)
        );
    }

    #[tokio::test]
    async fn bash_empty_command_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = BashTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "command": "" }))
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
    async fn bash_missing_command_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = BashTool::new(ctx);

        let out = tool.call(serde_json::json!({})).await.unwrap();
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
    async fn bash_nonzero_exit_failed() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = BashTool::new(ctx);

        let out = tool
            .call(serde_json::json!({ "command": "exit 3" }))
            .await
            .unwrap();
        let block = match out {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        };
        assert_eq!(block.state, ToolResultState::Error);
        assert!(
            text_of(&block).contains("command_failed"),
            "got: {}",
            text_of(&block)
        );
    }

    #[test]
    fn output_truncated_over_30000_chars() {
        let long = "x".repeat(40_000);
        let out = truncate_chars(long, MAX_OUTPUT_CHARS, "\n... (output truncated)");
        assert!(out.contains("... (output truncated)"));
        assert!(out.chars().count() <= MAX_OUTPUT_CHARS + 24);
    }
}
