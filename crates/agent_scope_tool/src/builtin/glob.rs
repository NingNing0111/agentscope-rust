//! GlobTool — built-in `Glob` tool for fast file pattern matching.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_glob.py` (upstream commit `9d1026fa`).

use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value as JsonValue;
use walkdir::WalkDir;

use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::{BuiltInToolContext, ToolErrorCategory};

/// Default maximum number of matched files returned before truncation
/// (pi-rust `DEFAULT_GLOB_MAX_RESULTS`).
const MAX_GLOB_RESULTS: usize = 200;
/// Maximum entries scanned by Glob before stopping, so a deep tree cannot
/// stall the host (pi-rust `MAX_GLOB_SCAN_ENTRIES`).
const MAX_GLOB_SCAN_ENTRIES: usize = 100_000;

/// Built-in `Glob` tool.
///
/// Walks the workspace tree (native Rust `walkdir` traversal, no shell-out)
/// and matches each file's workspace-relative path against a compiled
/// `globset`. Results are sorted by modification time (newest first) with a
/// deterministic lexicographic tie-break, then truncated to a bounded result
/// set so a huge match set cannot flood the model context.
pub struct GlobTool {
    ctx: BuiltInToolContext,
}

impl GlobTool {
    /// Create a new [`GlobTool`] bound to a workspace context.
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

/// Compile `pattern` into a [`GlobSet`].
///
/// `literal_separator(true)` keeps `*`/`?` within a single path segment while
/// `**` still spans zero or more components — mirroring the Python helper's
/// segment-wise matching (so `*.rs` matches only top-level files, while
/// `**/*.rs` recurses). A build failure signals an invalid glob pattern.
fn build_glob_set(pattern: &str) -> Result<GlobSet, globset::Error> {
    let mut builder = GlobSetBuilder::new();
    builder.add(GlobBuilder::new(pattern).literal_separator(true).build()?);
    builder.build()
}

#[async_trait::async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    fn description(&self) -> &str {
        "Fast file pattern matching tool that works with any codebase size.\n\
         \n\
         Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\" and returns matching file paths sorted by modification time (newest first).\n\
         \n\
         Use this tool when you need to find files by pattern across the codebase."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match against (e.g., '**/*.py', 'src/**/*.ts')"
                },
                "path": {
                    "type": "string",
                    "description": "The base directory to search from (defaults to current working directory)"
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
        // Extract the required, non-empty `pattern` parameter.
        let pattern = match input.get("pattern").and_then(JsonValue::as_str) {
            Some(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(ToolExecOutput::Complete(make_result(
                        "Glob",
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
                    "Glob",
                    format!(
                        "Error: {}: invalid_arguments: missing required 'pattern' parameter",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // Resolve the optional base directory; default to the workspace root.
        // An empty `path` is treated as absent.
        let base_dir = match input
            .get("path")
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(p) => match self.ctx.resolve_in_workspace(p) {
                Ok(p) => p,
                Err(e) => {
                    return Ok(ToolExecOutput::Complete(make_result(
                        "Glob",
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

        // The base must be an existing directory.
        let is_dir = match self.ctx.backend.is_dir(&base_dir).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Glob",
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
                "Glob",
                format!(
                    "Error: {}: file_not_found: Directory not found: {base_dir}",
                    ToolErrorCategory::ExecutionFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }

        // Compile the glob set; a build failure is an invalid pattern.
        let glob_set = match build_glob_set(&pattern) {
            Ok(gs) => gs,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Glob",
                    format!(
                        "Error: {}: invalid_pattern: invalid glob pattern: {e}",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // Walk the tree, collecting workspace-relative paths of matching
        // regular files. Symlinks are skipped (never followed). The scan is
        // bounded so a pathological tree cannot stall the host.
        let mut matches: Vec<String> = Vec::new();
        let mut entries_scanned = 0usize;
        let mut scan_cap_hit = false;
        for entry in WalkDir::new(base_dir.as_str())
            .follow_links(false)
            .into_iter()
        {
            let Ok(entry) = entry else { continue };
            if entry.depth() == 0 {
                continue;
            }
            entries_scanned += 1;
            if entries_scanned > MAX_GLOB_SCAN_ENTRIES {
                scan_cap_hit = true;
                break;
            }
            let ft = entry.file_type();
            if ft.is_symlink() || !ft.is_file() {
                continue;
            }
            let Some(rel) = entry.path().strip_prefix(base_dir.as_str()).ok() else {
                continue;
            };
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if glob_set.is_match(rel_str.as_str()) {
                matches.push(rel_str);
            }
        }

        // No matches — report success with the exact Python message.
        if matches.is_empty() {
            let note = if scan_cap_hit {
                format!(
                    "; scan stopped at {MAX_GLOB_SCAN_ENTRIES} entries, results may be incomplete"
                )
            } else {
                String::new()
            };
            return Ok(ToolExecOutput::Complete(make_result(
                "Glob",
                format!("No files found matching pattern: {pattern}{note}"),
                ToolResultState::Success,
            )));
        }

        // Sort by modification time (newest first), falling back to a
        // lexicographic tie-break for determinism. Missing/errored mtimes
        // sort as the oldest (0.0), mirroring the Python helper's `_mtime`.
        let mut with_mtime: Vec<(String, f64)> = Vec::with_capacity(matches.len());
        for rel in matches {
            let full = self.ctx.backend.join_path(&base_dir, &rel);
            let mtime = match self.ctx.backend.stat_mtime(&full).await {
                Ok(Some(t)) => t,
                _ => 0.0,
            };
            with_mtime.push((rel, mtime));
        }
        with_mtime.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        // Bound the result set and build the newline-joined output.
        let mut truncated = false;
        if with_mtime.len() > MAX_GLOB_RESULTS {
            with_mtime.truncate(MAX_GLOB_RESULTS);
            truncated = true;
        }
        let files: Vec<&str> = with_mtime.iter().map(|(p, _)| p.as_str()).collect();

        let mut notes = Vec::new();
        if scan_cap_hit {
            notes.push(format!(
                "scan stopped at {MAX_GLOB_SCAN_ENTRIES} entries, results may be incomplete"
            ));
        }
        if truncated {
            notes.push(format!(
                "results truncated at {MAX_GLOB_RESULTS} matches, results may be incomplete"
            ));
        }
        let note = if notes.is_empty() {
            String::new()
        } else {
            format!("; {}", notes.join("; "))
        };

        Ok(ToolExecOutput::Complete(make_result(
            "Glob",
            format!("{}{note}", files.join("\n")),
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

    /// Run the tool synchronously and unwrap the single complete block.
    async fn run(tool: &GlobTool, input: JsonValue) -> ToolResultBlock {
        match tool.call(input).await.unwrap() {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        }
    }

    #[tokio::test]
    async fn glob_empty_pattern_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GlobTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "  " })).await;
        assert_eq!(block.state, ToolResultState::Error);
        assert!(
            text_of(&block).contains("invalid_arguments"),
            "got: {}",
            text_of(&block)
        );
    }

    #[tokio::test]
    async fn glob_missing_pattern_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GlobTool::new(ctx);

        let block = run(&tool, serde_json::json!({})).await;
        assert_eq!(block.state, ToolResultState::Error);
        assert!(
            text_of(&block).contains("invalid_arguments"),
            "got: {}",
            text_of(&block)
        );
    }

    #[tokio::test]
    async fn glob_simple_pattern_match() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn main() {}\n").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub").join("b.rs"), "fn b() {}\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GlobTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "*.rs" })).await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(text.contains("a.rs"), "got: {text}");
        // `*` stays within a single path segment — nested b.rs is not matched.
        assert!(!text.contains("b.rs"), "got: {text}");
    }

    #[tokio::test]
    async fn glob_recursive_double_star() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn main() {}\n").unwrap();
        std::fs::create_dir_all(dir.path().join("src/deep")).unwrap();
        std::fs::write(dir.path().join("src").join("lib.rs"), "fn lib() {}\n").unwrap();
        std::fs::write(
            dir.path().join("src/deep").join("util.rs"),
            "fn util() {}\n",
        )
        .unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GlobTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "**/*.rs" })).await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        for expected in ["a.rs", "src/lib.rs", "src/deep/util.rs"] {
            assert!(text.contains(expected), "missing {expected} in: {text}");
        }
    }

    #[tokio::test]
    async fn glob_path_param_scopes_base() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "").unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src").join("b.rs"), "").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GlobTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "*.rs", "path": "src" }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(text.contains("b.rs"), "got: {text}");
        assert!(!text.contains("a.rs"), "got: {text}");
    }

    #[tokio::test]
    async fn glob_no_match_success() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GlobTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "*.py" })).await;
        assert_eq!(block.state, ToolResultState::Success);
        assert_eq!(text_of(&block), "No files found matching pattern: *.py");
    }

    #[tokio::test]
    async fn glob_path_escape_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GlobTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "*.rs", "path": "../outside" }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Error);
        assert!(
            text_of(&block).contains("path_outside_workspace"),
            "got: {}",
            text_of(&block)
        );
    }

    #[tokio::test]
    async fn glob_directory_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GlobTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "*.rs", "path": "missing" }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Error);
        let text = text_of(&block);
        assert!(text.contains("file_not_found"), "got: {text}");
        assert!(text.contains("Directory not found"), "got: {text}");
    }

    #[tokio::test]
    async fn glob_invalid_pattern_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GlobTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "[" })).await;
        assert_eq!(block.state, ToolResultState::Error);
        assert!(
            text_of(&block).contains("invalid_pattern"),
            "got: {}",
            text_of(&block)
        );
    }

    #[tokio::test]
    async fn glob_sorted_by_mtime_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let now = std::time::SystemTime::now();
        let old = now - std::time::Duration::from_secs(200);
        let f = std::fs::File::create(dir.path().join("old.txt")).unwrap();
        f.set_modified(old).unwrap();
        let f = std::fs::File::create(dir.path().join("new.txt")).unwrap();
        f.set_modified(now).unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GlobTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "*.txt" })).await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        let new_pos = text.find("new.txt").expect("new.txt present");
        let old_pos = text.find("old.txt").expect("old.txt present");
        assert!(
            new_pos < old_pos,
            "expected new.txt before old.txt, got: {text}"
        );
    }
}
