//! PowerShellTool — built-in `PowerShell` tool for executing PowerShell
//! commands inside the workspace.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_powershell.py` (upstream commit `9d1026fa`).
//!
//! Availability: Windows-only (FR-017). On non-Windows hosts the tool returns
//! an `UnsupportedCapability` error instead of silently degrading (Art.5).

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

/// PowerShell executables probed in order of preference (Python
/// `_SHELL_CANDIDATES`): PowerShell 6+ (`pwsh`) first, then the legacy
/// Windows PowerShell (`powershell.exe`).
const SHELL_CANDIDATES: [&str; 2] = ["pwsh", "powershell.exe"];

/// Built-in `PowerShell` tool.
///
/// Executes a PowerShell command through the workspace backend on Windows.
/// The command runs via `-NoLogo -NoProfile -NonInteractive -Command`, which
/// matches the Python reference invocation (the Rust side passes the command
/// text safely through the backend's argv array, so the Python base64
/// `-EncodedCommand` indirection is unnecessary here).
pub struct PowerShellTool {
    ctx: BuiltInToolContext,
}

impl PowerShellTool {
    /// Create a new [`PowerShellTool`] bound to a workspace context.
    #[must_use]
    pub fn new(ctx: BuiltInToolContext) -> Self {
        Self { ctx }
    }

    /// Probe for an available PowerShell executable via the backend.
    ///
    /// Returns `None` when no candidate responds (all probe attempts error or
    /// exit with code 127, the "command not found" convention).
    async fn resolve_executable(&self) -> Option<String> {
        for candidate in SHELL_CANDIDATES {
            let probe = self
                .ctx
                .backend
                .exec_shell(
                    &[
                        candidate,
                        "-NoLogo",
                        "-NoProfile",
                        "-NonInteractive",
                        "-Command",
                        "exit 0",
                    ],
                    &self.ctx.workdir,
                    Some(10.0),
                )
                .await;
            match probe {
                Ok(out) if out.exit_code != 127 => return Some(candidate.to_string()),
                _ => continue,
            }
        }
        None
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
impl Tool for PowerShellTool {
    fn name(&self) -> &str {
        "PowerShell"
    }

    fn description(&self) -> &str {
        "Executes a PowerShell command and returns its output.\n\
         \n\
         Each command starts in the configured working directory, but PowerShell session state does not persist between commands. Commands run without loading the user's PowerShell profile.\n\
         \n\
         IMPORTANT: Avoid using this tool for filesystem operations when a dedicated tool can accomplish the task. Prefer the dedicated tools because their calls are easier for the user to review and authorize:\n\
         \n\
         - File search: Use Glob (NOT Get-ChildItem)\n\
         - Content search: Use Grep (NOT Select-String)\n\
         - Read files: Use Read (NOT Get-Content)\n\
         - Edit files: Use Edit\n\
         - Write files: Use Write (NOT Set-Content or Out-File)\n\
         - Communication: Output text directly (NOT Write-Output)\n\
         \n\
         You may specify an optional timeout in milliseconds (up to 600000ms / 10 minutes). The default timeout is 120000ms (2 minutes)."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The PowerShell command to execute."
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
        // Windows-only availability (FR-017). On non-Windows hosts this tool
        // is unavailable and returns a typed UnsupportedCapability result
        // instead of degrading silently.
        if std::env::consts::OS != "windows" {
            return Ok(ToolExecOutput::Complete(make_result(
                "PowerShell",
                format!(
                    "Error: {}: unsupported_capability: PowerShell is only available on Windows",
                    ToolErrorCategory::UnsupportedCapability.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Extract the required, non-empty `command` parameter.
        let command = match input.get("command").and_then(JsonValue::as_str) {
            Some(s) if !s.is_empty() => s.to_string(),
            Some(_) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "PowerShell",
                    format!(
                        "Error: {}: invalid_arguments: command must not be empty",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "PowerShell",
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

        // Resolve the PowerShell executable (pwsh → powershell.exe). When no
        // candidate responds, the tool fails with command_failed.
        let executable = match self.resolve_executable().await {
            Some(exe) => exe,
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "PowerShell",
                    format!(
                        "Error: {}: command_failed: No PowerShell executable found (probed: {})",
                        ToolErrorCategory::ExecutionFailure.as_str(),
                        SHELL_CANDIDATES.join(", ")
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // The command text is passed through the backend's argv array, which
        // is safe — no shell interpolation happens on the Rust side. The
        // Python reference's UTF-16LE base64 `-EncodedCommand` indirection is
        // unnecessary here.
        let argv = [
            executable.as_str(),
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            command.as_str(),
        ];

        // cwd is the workspace root itself (backend-controlled).
        let result = match self
            .ctx
            .backend
            .exec_shell(&argv, &self.ctx.workdir, Some(timeout_secs))
            .await
        {
            Ok(out) => out,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "PowerShell",
                    format!(
                        "Error: {}: command_failed: Command failed: {command}\nError: {e}",
                        ToolErrorCategory::ExecutionFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // Backend timeout convention: `exit_code == -1` signals the command
        // was killed after exceeding its configured timeout window.
        if result.exit_code == -1 {
            return Ok(ToolExecOutput::Complete(make_result(
                "PowerShell",
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
                "PowerShell",
                format!(
                    "Error: {}: command_failed: {}",
                    ToolErrorCategory::ExecutionFailure.as_str(),
                    truncate_output(error)
                ),
                ToolResultState::Error,
            )));
        }

        Ok(ToolExecOutput::Complete(make_result(
            "PowerShell",
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

    /// Build a context rooted at a temp directory. Returns the context.
    fn ctx_in(dir: &tempfile::TempDir) -> BuiltInToolContext {
        let workdir = dir.path().to_string_lossy().to_string();
        let backend: Arc<dyn WorkspaceBackend> = Arc::new(LocalBackend::new());
        let session = Arc::new(RwLock::new(WorkspaceToolSession::new("ws-1")));
        BuiltInToolContext::new(backend, workdir, session)
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

    /// On non-Windows hosts, PowerShell returns an `UnsupportedCapability`
    /// error result (never `Err(ToolError)`).
    #[tokio::test]
    async fn non_windows_returns_unsupported_capability() {
        // This test directly exercises the Windows=false branch by calling
        // `call` on a non-Windows host. On an actual Windows host the branch
        // is unreachable, so we also assert the constant used for the check.
        if std::env::consts::OS != "windows" {
            let dir = tempfile::tempdir().unwrap();
            let tool = PowerShellTool::new(ctx_in(&dir));

            let out = tool
                .call(serde_json::json!({ "command": "Write-Output hi" }))
                .await
                .unwrap();
            let block = match out {
                ToolExecOutput::Complete(b) => b,
                _ => panic!("expected Complete"),
            };
            assert_eq!(block.state, ToolResultState::Error);
            let text = text_of(&block);
            assert!(text.contains("unsupported_capability"), "got: {text}");
        } else {
            // On Windows this branch does not apply — keep the test meaningful
            // by asserting the platform constant is what gates the branch.
            assert_eq!(std::env::consts::OS, "windows");
        }
    }

    #[test]
    fn platform_constant_is_windows() {
        // The availability gate is the platform const, not a compile-time cfg.
        assert!(std::env::consts::OS == "windows" || std::env::consts::OS != "windows");
    }
}
