//! LsTool — pi-compatible built-in `ls` tool for listing workspace directories.

use agent_scope_message::ToolResultState;
use serde_json::Value as JsonValue;

use crate::make_text_result as make_result;
use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::{BuiltInToolContext, ToolErrorCategory};

/// Default maximum number of directory entries returned.
const DEFAULT_LS_LIMIT: usize = 500;
/// Hard upper bound for directory entries returned to the model.
const MAX_LS_LIMIT: usize = 2_000;

/// Built-in pi-compatible `ls` tool.
pub struct LsTool {
    ctx: BuiltInToolContext,
}

impl LsTool {
    /// Create a new [`LsTool`] bound to a workspace context.
    #[must_use]
    pub fn new(ctx: BuiltInToolContext) -> Self {
        Self { ctx }
    }
}

fn clamp_limit(input: &JsonValue) -> usize {
    input
        .get("limit")
        .and_then(JsonValue::as_u64)
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_LS_LIMIT)
        .min(MAX_LS_LIMIT)
}

#[async_trait::async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "ls"
    }

    fn description(&self) -> &str {
        "Lists files and directories directly under a workspace directory. Use `path` to select a directory and `limit` to bound output. Directories are marked with a trailing `/`."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory to list. Defaults to the workspace root."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of entries to return (default: 500, max: 2000).",
                    "minimum": 0,
                    "maximum": 2000
                }
            }
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let target = match input
            .get("path")
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(path) => match self.ctx.resolve_in_workspace(path) {
                Ok(path) => path,
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
            },
            None => self.ctx.workdir.clone(),
        };

        let is_dir = match self.ctx.backend.is_dir(&target).await {
            Ok(is_dir) => is_dir,
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
        if !is_dir {
            return Ok(ToolExecOutput::Complete(make_result(
                self.name(),
                format!(
                    "Error: {}: file_not_found: Directory not found: {target}",
                    ToolErrorCategory::ExecutionFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        let mut entries = match self.ctx.backend.list_dir(&target, false).await {
            Ok(entries) => entries,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: execution: failed to list directory: {e}",
                        ToolErrorCategory::ExecutionFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };
        entries.sort();

        let limit = clamp_limit(&input);
        let total = entries.len();
        let mut lines = Vec::new();
        for entry in entries.into_iter().take(limit) {
            let mut name = self.ctx.backend.basename(&entry);
            if self.ctx.backend.is_dir(&entry).await.unwrap_or(false) {
                name.push('/');
            }
            lines.push(name);
        }

        let mut output = if total == 0 {
            format!("No entries found in {target}")
        } else if lines.is_empty() {
            format!("No entries returned from {target}; limit {limit}")
        } else {
            lines.join("\n")
        };
        if total > limit {
            output.push_str(&format!(
                "\n... ({} more entries truncated; limit {limit})",
                total - limit
            ));
        }

        Ok(ToolExecOutput::Complete(make_result(
            self.name(),
            output,
            ToolResultState::Success,
        )))
    }
}
