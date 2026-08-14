//! BashTool — built-in `Bash` tool for executing shell commands inside the
//! workspace.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_bash.py` (upstream commit `9d1026fa`).

use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use serde_json::Value as JsonValue;

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
}

impl BashTool {
    /// Create a new [`BashTool`] bound to a workspace context.
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

/// Clamp a user-supplied timeout (ms) into `[0, MAX_TIMEOUT_MS]`, defaulting
/// to [`DEFAULT_TIMEOUT_MS`] when absent.
fn clamp_timeout_ms(timeout: Option<i64>) -> i64 {
    timeout
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(0, MAX_TIMEOUT_MS)
}

/// Decode stdout/stderr as UTF-8 (lossy), normalize CRLF, merge (stderr
/// appended when non-empty), and truncate to [`MAX_OUTPUT_CHARS`] characters.
fn format_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout).replace("\r\n", "\n");
    let stderr = String::from_utf8_lossy(stderr).replace("\r\n", "\n");
    let mut output = stdout;
    if !stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&stderr);
    }
    truncate_output(output)
}

/// Truncate `text` to [`MAX_OUTPUT_CHARS`] characters, appending the Python
/// `... (output truncated)` marker.
fn truncate_output(text: String) -> String {
    if text.chars().count() > MAX_OUTPUT_CHARS {
        let truncated: String = text.chars().take(MAX_OUTPUT_CHARS).collect();
        format!("{truncated}\n... (output truncated)")
    } else {
        text
    }
}

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
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

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
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
        })
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
                    "Bash",
                    format!(
                        "Error: {}: invalid_arguments: command must not be empty",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Bash",
                    format!(
                        "Error: {}: invalid_arguments: missing required 'command' parameter",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // Clamp timeout to the max window and convert milliseconds to seconds.
        let timeout_ms = clamp_timeout_ms(input.get("timeout").and_then(JsonValue::as_i64));
        let timeout_secs = timeout_ms as f64 / 1000.0;

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
            .exec_shell(&shell_cmd, &self.ctx.workdir, Some(timeout_secs))
            .await
        {
            Ok(out) => out,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Bash",
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
                "Bash",
                format!(
                    "Error: {}: command_timeout: Command timed out after {timeout_ms}ms: {command}",
                    ToolErrorCategory::Timeout.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        let output = format_output(&result.stdout, &result.stderr);

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
                "Bash",
                format!(
                    "Error: {}: command_failed: {}",
                    ToolErrorCategory::ExecutionFailure.as_str(),
                    truncate_output(error)
                ),
                ToolResultState::Error,
            )));
        }

        Ok(ToolExecOutput::Complete(make_result(
            "Bash",
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
        assert_eq!(clamp_timeout_ms(None), DEFAULT_TIMEOUT_MS);
        assert_eq!(clamp_timeout_ms(Some(0)), 0);
        assert_eq!(clamp_timeout_ms(Some(300_000)), 300_000);
        assert_eq!(clamp_timeout_ms(Some(900_000)), MAX_TIMEOUT_MS);
        assert_eq!(clamp_timeout_ms(Some(1_000_000)), MAX_TIMEOUT_MS);
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
        let out = truncate_output(long);
        assert!(out.contains("... (output truncated)"));
        assert!(out.chars().count() <= MAX_OUTPUT_CHARS + 24);
    }
}
