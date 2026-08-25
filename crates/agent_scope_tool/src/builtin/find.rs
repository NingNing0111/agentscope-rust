//! FindTool — pi-compatible built-in `find` tool for workspace file discovery.

use agent_scope_message::ToolResultState;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value as JsonValue;

use crate::make_text_result as make_result;
use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::{BuiltInToolContext, ToolErrorCategory};

/// Default maximum number of matched files returned.
const DEFAULT_FIND_LIMIT: usize = 1_000;
/// Hard upper bound for matched files returned to the model.
const MAX_FIND_LIMIT: usize = 5_000;
/// Maximum entries scanned before stopping so huge trees cannot stall the host.
const MAX_FIND_SCAN_ENTRIES: usize = 100_000;

/// Built-in pi-compatible `find` tool.
pub struct FindTool {
    ctx: BuiltInToolContext,
}

impl FindTool {
    /// Create a new [`FindTool`] bound to a workspace context.
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
        .unwrap_or(DEFAULT_FIND_LIMIT)
        .min(MAX_FIND_LIMIT)
}

fn build_glob_set(pattern: &str) -> Result<GlobSet, globset::Error> {
    let mut builder = GlobSetBuilder::new();
    builder.add(GlobBuilder::new(pattern).literal_separator(true).build()?);
    builder.build()
}

fn should_skip_dir(name: &str) -> bool {
    matches!(name, ".git" | "node_modules")
}

#[async_trait::async_trait]
impl Tool for FindTool {
    fn name(&self) -> &str {
        "find"
    }

    fn description(&self) -> &str {
        "Finds files in the workspace by glob pattern. Uses the workspace backend for traversal, ignores `.git` and `node_modules`, and returns workspace-relative paths."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files (for example, '*.rs' or '**/*.ts')."
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search from. Defaults to the workspace root."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of files to return (default: 1000, max: 5000).",
                    "minimum": 0,
                    "maximum": 5000
                }
            },
            "required": ["pattern"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let pattern = match input.get("pattern").and_then(JsonValue::as_str) {
            Some(pattern) => {
                let trimmed = pattern.trim();
                if trimmed.is_empty() {
                    return Ok(ToolExecOutput::Complete(make_result(
                        self.name(),
                        format!(
                            "Error: {}: invalid_arguments: pattern must not be empty",
                            ToolErrorCategory::ValidationFailure.as_str()
                        ),
                        ToolResultState::Error,
                    )));
                }
                trimmed.to_string()
            }
            None => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: invalid_arguments: missing required 'pattern' parameter",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        let base_dir = match input
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

        let is_dir = match self.ctx.backend.is_dir(&base_dir).await {
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
                    "Error: {}: file_not_found: Directory not found: {base_dir}",
                    ToolErrorCategory::ExecutionFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        let glob_set = match build_glob_set(&pattern) {
            Ok(glob_set) => glob_set,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    self.name(),
                    format!(
                        "Error: {}: invalid_pattern: invalid glob pattern: {e}",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        let limit = clamp_limit(&input);
        let mut matches = Vec::new();
        let mut entries_scanned = 0usize;
        let mut scan_cap_hit = false;
        let mut result_cap_hit = false;

        let entries = match self.ctx.backend.list_dir(&base_dir, true).await {
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

        for entry in entries {
            entries_scanned += 1;
            if entries_scanned > MAX_FIND_SCAN_ENTRIES {
                scan_cap_hit = true;
                break;
            }
            let name = self.ctx.backend.basename(&entry);
            if self.ctx.backend.is_dir(&entry).await.unwrap_or(false) {
                if should_skip_dir(&name) {
                    continue;
                }
                continue;
            }
            let rel_str = match entry.strip_prefix(self.ctx.workdir.as_str()) {
                Some(rel) => rel.trim_start_matches('/').replace('\\', "/"),
                None => continue,
            };
            if rel_str.is_empty() {
                continue;
            }
            if rel_str.split('/').any(should_skip_dir) {
                continue;
            }
            let base_rel_str = match entry.strip_prefix(base_dir.as_str()) {
                Some(rel) => rel.trim_start_matches('/').replace('\\', "/"),
                None => rel_str.clone(),
            };
            if glob_set.is_match(rel_str.as_str()) || glob_set.is_match(base_rel_str.as_str()) {
                if matches.len() >= limit {
                    result_cap_hit = true;
                    break;
                }
                matches.push(rel_str);
            }
        }

        matches.sort();

        let mut output = if matches.is_empty() {
            format!("No files found matching pattern: {pattern}")
        } else {
            matches.join("\n")
        };
        if result_cap_hit {
            output.push_str(&format!("\n... (results truncated at limit {limit})"));
        }
        if scan_cap_hit {
            output.push_str(&format!(
                "\n... (scan stopped at {MAX_FIND_SCAN_ENTRIES} entries; results may be incomplete)"
            ));
        }

        Ok(ToolExecOutput::Complete(make_result(
            self.name(),
            output,
            ToolResultState::Success,
        )))
    }
}
