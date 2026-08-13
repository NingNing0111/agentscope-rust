//! GrepTool — built-in `Grep` tool for regex content search across the
//! workspace.
//!
//! Mirrors the Python reference implementation in
//! `agentscope/tool/_builtin/_grep.py` (upstream commit `9d1026fa`).
//!
//! The search is implemented natively in Rust (`regex` + `walkdir` traversal)
//! — it never shells out to `rg`/`find`. Results are bounded (head_limit with
//! a hard cap, a per-file byte cap, and a scan-entry cap) so a pathological
//! tree cannot flood the model context.

use std::collections::HashSet;
use std::path::Path;

use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use regex::Regex;
use serde_json::Value as JsonValue;
use walkdir::WalkDir;

use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

use super::{BuiltInToolContext, ToolErrorCategory};

/// Default result cap when `head_limit` is unspecified (Python
/// `DEFAULT_HEAD_LIMIT = 250`).
const DEFAULT_HEAD_LIMIT: usize = 250;
/// Absolute upper bound on returned entries/lines regardless of `head_limit`
/// (spec SC-007; pi-rust `MAX_GREP_RESULTS`).
const MAX_GREP_RESULTS: usize = 1000;
/// Per-file byte cap — files larger than this are skipped rather than loaded
/// wholesale (pi-rust `MAX_GREP_FILE_BYTES`).
const MAX_GREP_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// Stop scanning after this many filesystem entries so a huge tree cannot
/// stall the host (pi-rust `MAX_GREP_SCAN_FILES`).
const MAX_GREP_SCAN_FILES: usize = 50_000;
/// Per-line character cap in output so a huge (minified/base64) line cannot
/// dominate the result (pi-rust `MAX_GREP_LINE_CHARS`).
const MAX_GREP_LINE_CHARS: usize = 200;
/// Binary-signature check: look for a NUL byte in the first 8 KiB.
const BINARY_CHECK_WINDOW: usize = 8192;
/// Extra lines allowed during collection so a match group's trailing context
/// is not cut off mid-emission.
const CONTEXT_SLACK: usize = 200;

/// Version-control directories excluded from the search (Python
/// `VCS_DIRECTORIES_TO_EXCLUDE`).
const VCS_DIRS: [&str; 6] = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl"];

/// The Grep output mode (Python `output_mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Content,
    FilesWithMatches,
    Count,
}

/// Per-invocation search parameters shared with the file-processing helpers.
struct SearchParams<'a> {
    output_mode: OutputMode,
    regex: &'a Regex,
    /// `multiline` — `^`/`$` anchor at line boundaries and `.` matches `\n`.
    multiline: bool,
    /// `n` — show line numbers (content mode only).
    show_line_numbers: bool,
    context_before: usize,
    context_after: usize,
    glob: Option<&'a GlobSet>,
    type_filter: Option<&'static [&'static str]>,
    /// Stop collecting once this many output lines have been accumulated.
    collect_cap: usize,
}

/// Mutable collection state threaded through the file scan.
#[derive(Default)]
struct SearchState {
    /// Formatted output lines for the current mode.
    output_lines: Vec<String>,
    /// Number of files actually read.
    files_scanned: usize,
    /// Number of files skipped (oversized, binary, or unreadable).
    files_skipped: usize,
    /// True when collection stopped early at the cap.
    truncated: bool,
    /// True when the tree walk hit the scan-entry cap.
    scan_cap_hit: bool,
}

/// Built-in `Grep` tool.
///
/// Searches file contents with a native Rust regex. `path` defaults to the
/// workspace root; every path is confined to the workspace via
/// [`BuiltInToolContext::resolve_in_workspace`]. VCS directories are excluded
/// and results are bounded so a huge match set cannot flood the model context.
pub struct GrepTool {
    ctx: BuiltInToolContext,
}

impl GrepTool {
    /// Create a new [`GrepTool`] bound to a workspace context.
    #[must_use]
    pub fn new(ctx: BuiltInToolContext) -> Self {
        Self { ctx }
    }

    /// Search a single file and push its results into `state`.
    async fn search_file(
        &self,
        full: &str,
        rel: &str,
        params: &SearchParams<'_>,
        state: &mut SearchState,
    ) {
        // Apply glob / type filters before touching the file.
        let file_name = Path::new(full)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Some(glob) = params.glob
            && !glob.is_match(rel)
            && !glob.is_match(&file_name)
        {
            return;
        }
        if let Some(exts) = params.type_filter {
            let ext = Path::new(full)
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase);
            let Some(ext) = ext else { return };
            if !exts.contains(&ext.as_str()) {
                return;
            }
        }

        state.files_scanned += 1;

        // Read through the backend (containment enforced there). Unreadable /
        // oversized files (e.g. beyond the backend 10 MiB read cap) are
        // skipped rather than failing the whole search.
        let bytes = match self.ctx.backend.read_file(full).await {
            Ok(b) => b,
            Err(_) => {
                state.files_skipped += 1;
                return;
            }
        };
        if bytes.len() as u64 > MAX_GREP_FILE_BYTES {
            state.files_skipped += 1;
            return;
        }
        if looks_binary(&bytes) {
            state.files_skipped += 1;
            return;
        }
        let content = String::from_utf8_lossy(&bytes);
        let content: &str = content.as_ref();

        match params.output_mode {
            OutputMode::FilesWithMatches => {
                let has_match = if params.multiline {
                    params.regex.is_match(content)
                } else {
                    content.lines().any(|l| params.regex.is_match(l))
                };
                if has_match {
                    state.output_lines.push(rel.to_string());
                    if state.output_lines.len() >= params.collect_cap {
                        state.truncated = true;
                    }
                }
            }
            OutputMode::Count => {
                let count = if params.multiline {
                    params.regex.find_iter(content).count()
                } else {
                    content.lines().filter(|l| params.regex.is_match(l)).count()
                };
                if count > 0 {
                    state.output_lines.push(format!("{rel}:{count}"));
                    if state.output_lines.len() >= params.collect_cap {
                        state.truncated = true;
                    }
                }
            }
            OutputMode::Content => emit_content_matches(rel, content, params, state),
        }
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

/// Compile the search regex. `multiline` enables `^`/`$` at line boundaries
/// and `.` matching `\n` (Python `rg -U --multiline-dotall`).
fn build_regex(
    pattern: &str,
    case_insensitive: bool,
    multiline: bool,
) -> Result<Regex, regex::Error> {
    let mut builder = regex::RegexBuilder::new(pattern);
    builder
        .case_insensitive(case_insensitive)
        .multi_line(multiline)
        .dot_matches_new_line(multiline);
    builder.build()
}

/// Compile a comma/whitespace-separated glob filter into a [`GlobSet`].
///
/// `literal_separator(true)` keeps `*`/`?` within a single path segment while
/// `**` still spans zero or more components — mirroring the Python helper's
/// segment-wise handling. Returns `Ok(None)` when every token was empty (no
/// effective filter).
fn build_glob_set(pattern: &str) -> Result<Option<GlobSet>, globset::Error> {
    let mut builder = GlobSetBuilder::new();
    let mut added = false;
    for raw in pattern.split_whitespace() {
        if raw.contains('{') && raw.contains('}') {
            builder.add(GlobBuilder::new(raw).literal_separator(true).build()?);
            added = true;
        } else {
            for p in raw.split(',') {
                let p = p.trim();
                if p.is_empty() {
                    continue;
                }
                builder.add(GlobBuilder::new(p).literal_separator(true).build()?);
                added = true;
            }
        }
    }
    if !added {
        return Ok(None);
    }
    builder.build().map(Some)
}

/// Map a `rg --type` name to its known file extensions. Returns `None` for
/// unrecognized types (mirrors `rg`'s "unrecognized file type" error).
fn file_type_extensions(t: &str) -> Option<&'static [&'static str]> {
    match t {
        "js" | "javascript" => Some(&["js", "mjs", "cjs", "jsx"]),
        "ts" | "typescript" => Some(&["ts", "mts", "cts", "tsx"]),
        "py" | "python" => Some(&["py", "pyi", "pyw"]),
        "rust" | "rs" => Some(&["rs"]),
        "go" | "golang" => Some(&["go"]),
        "java" => Some(&["java"]),
        "c" => Some(&["c", "h"]),
        "cpp" | "cxx" | "cc" => Some(&["cpp", "cc", "cxx", "hpp", "hh", "hxx"]),
        "cs" | "csharp" => Some(&["cs"]),
        "rb" | "ruby" => Some(&["rb"]),
        "php" => Some(&["php"]),
        "sh" | "bash" | "shell" => Some(&["sh", "bash", "zsh"]),
        "html" => Some(&["html", "htm"]),
        "css" => Some(&["css"]),
        "json" => Some(&["json"]),
        "md" | "markdown" => Some(&["md", "markdown"]),
        "yaml" | "yml" => Some(&["yaml", "yml"]),
        "toml" => Some(&["toml"]),
        "txt" | "text" => Some(&["txt", "text"]),
        "sql" => Some(&["sql"]),
        "tsx" => Some(&["tsx"]),
        "jsx" => Some(&["jsx"]),
        "vue" => Some(&["vue"]),
        "svelte" => Some(&["svelte"]),
        "dockerfile" => Some(&["dockerfile"]),
        "swift" => Some(&["swift"]),
        "kt" | "kotlin" => Some(&["kt", "kts"]),
        "scala" => Some(&["scala", "sc"]),
        "pl" | "perl" => Some(&["pl", "pm"]),
        "lua" => Some(&["lua"]),
        "r" => Some(&["r"]),
        "dart" => Some(&["dart"]),
        "ex" | "elixir" => Some(&["ex", "exs"]),
        "erl" | "erlang" => Some(&["erl", "hrl"]),
        "clj" | "clojure" => Some(&["clj", "cljs", "cljc"]),
        "hs" | "haskell" => Some(&["hs"]),
        "ml" | "ocaml" => Some(&["ml", "mli"]),
        "vb" => Some(&["vb", "vbs"]),
        "ps1" | "powershell" => Some(&["ps1", "psm1", "psd1"]),
        _ => None,
    }
}

/// True when the leading bytes contain a NUL (a binary-signature heuristic).
fn looks_binary(bytes: &[u8]) -> bool {
    let window = &bytes[..bytes.len().min(BINARY_CHECK_WINDOW)];
    window.contains(&0)
}

/// Truncate an over-long matched line to [`MAX_GREP_LINE_CHARS`] characters.
fn truncate_line(line: &str) -> String {
    line.chars().take(MAX_GREP_LINE_CHARS).collect()
}

/// Format a match line as `{rel}:{lineno}:{line}` (or without the line number
/// when `n` is false).
fn format_match_line(rel: &str, lineno: usize, line: &str, show_lineno: bool) -> String {
    let line = truncate_line(line);
    if show_lineno {
        format!("{rel}:{lineno}:{line}")
    } else {
        format!("{rel}:{line}")
    }
}

/// Format a context line rg-style with a `-` separator: `{rel}-{lineno}:{line}`.
fn format_context_line(rel: &str, lineno: usize, line: &str, show_lineno: bool) -> String {
    let line = truncate_line(line);
    if show_lineno {
        format!("{rel}-{lineno}:{line}")
    } else {
        format!("{rel}-{line}")
    }
}

/// Emit content-mode match lines (with optional `-A`/`-B`/`-C` context) for a
/// single file's decoded text.
fn emit_content_matches(
    rel: &str,
    content: &str,
    params: &SearchParams<'_>,
    state: &mut SearchState,
) {
    if params.multiline {
        // Multiline mode: report the first line of each match span. Context
        // lines are not meaningful when a match spans lines, so they are
        // omitted.
        for m in params.regex.find_iter(content) {
            let line_no = content[..m.start()].matches('\n').count() + 1;
            let line = content.lines().nth(line_no - 1).unwrap_or("");
            state.output_lines.push(format_match_line(
                rel,
                line_no,
                line,
                params.show_line_numbers,
            ));
            if state.output_lines.len() >= params.collect_cap {
                state.truncated = true;
                return;
            }
        }
        return;
    }

    // Line mode: emit match lines with optional context, keeping output in
    // line order. Context lines that are themselves matches are skipped (they
    // are emitted as match lines when the scan reaches them).
    let lines: Vec<&str> = content.lines().collect();
    let is_match: Vec<bool> = lines.iter().map(|l| params.regex.is_match(l)).collect();
    let mut emitted_context: HashSet<usize> = HashSet::new();
    for (idx, line) in lines.iter().enumerate() {
        if !is_match[idx] {
            continue;
        }

        // Before context.
        let before = params.context_before.min(idx);
        for b in (idx - before)..idx {
            if is_match[b] || emitted_context.contains(&b) {
                continue;
            }
            emitted_context.insert(b);
            state.output_lines.push(format_context_line(
                rel,
                b + 1,
                lines[b],
                params.show_line_numbers,
            ));
            if state.output_lines.len() >= params.collect_cap {
                state.truncated = true;
                return;
            }
        }

        // The match line itself.
        state.output_lines.push(format_match_line(
            rel,
            idx + 1,
            line,
            params.show_line_numbers,
        ));
        if state.output_lines.len() >= params.collect_cap {
            state.truncated = true;
            return;
        }

        // After context.
        let after = params
            .context_after
            .min(lines.len().saturating_sub(idx + 1));
        for a in (idx + 1)..(idx + 1 + after) {
            if is_match[a] || emitted_context.contains(&a) {
                continue;
            }
            emitted_context.insert(a);
            state.output_lines.push(format_context_line(
                rel,
                a + 1,
                lines[a],
                params.show_line_numbers,
            ));
            if state.output_lines.len() >= params.collect_cap {
                state.truncated = true;
                return;
            }
        }
    }
}

/// True when a walkdir entry is a version-control directory to prune.
fn is_vcs_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .is_some_and(|n| VCS_DIRS.contains(&n))
}

/// Render `full` relative to the workspace root, using `/` separators.
fn rel_to_workdir(workdir: &str, full: &str) -> String {
    let full_path = Path::new(full);
    let rel = full_path.strip_prefix(workdir).unwrap_or(full_path);
    rel.to_string_lossy().replace('\\', "/")
}

/// Build the "skipped / scan-cap" note list appended to success output.
fn skip_notes(state: &SearchState) -> Vec<String> {
    let mut notes = Vec::new();
    if state.scan_cap_hit {
        notes.push(format!(
            "scan stopped at {MAX_GREP_SCAN_FILES} entries, results may be incomplete"
        ));
    }
    if state.files_skipped > 0 {
        notes.push(format!(
            "skipped {} file(s) (too large, binary, or unreadable)",
            state.files_skipped
        ));
    }
    notes
}

#[async_trait::async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        "A powerful search tool built on ripgrep\n\
         \n\
         Usage:\n\
         - ALWAYS use Grep for search tasks. NEVER invoke `grep` or `rg` as a Bash command. The Grep tool has been optimized for correct permissions and access.\n\
         - Supports full regex syntax (e.g., \"log.*Error\", \"function\\s+\\w+\")\n\
         - Filter files with glob parameter (e.g., \"*.js\", \"**/*.tsx\") or type parameter (e.g., \"js\", \"py\", \"rust\")\n\
         - Output modes: \"content\" shows matching lines, \"files_with_matches\" shows only file paths (default), \"count\" shows match counts per file\n\
         - Context lines: use context parameter or -A/-B/-C for lines after/before/around matches\n\
         - Case-insensitive search: set i to true\n\
         - Multiline regex: set multiline to true for patterns spanning multiple lines\n\
         - Limit results: use head_limit to cap the number of results returned"
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regular expression pattern to search for in file contents."
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in. Defaults to current working directory."
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output mode: 'content' shows matching lines (supports -A/-B/-C context, -n line numbers, head_limit), 'files_with_matches' shows file paths, 'count' shows match counts. Defaults to 'files_with_matches'.",
                    "default": "files_with_matches"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., '*.js', '*.{ts,tsx}')."
                },
                "type": {
                    "type": "string",
                    "description": "File type to search (rg --type). Common types: js, py, rust, go, java, etc."
                },
                "-A": {
                    "type": "integer",
                    "description": "Number of lines to show after each match. Requires output_mode: 'content'."
                },
                "-B": {
                    "type": "integer",
                    "description": "Number of lines to show before each match. Requires output_mode: 'content'."
                },
                "-C": {
                    "type": "integer",
                    "description": "Alias for context."
                },
                "context": {
                    "type": "integer",
                    "description": "Number of context lines to show before and after matches. Requires output_mode: 'content'."
                },
                "n": {
                    "type": "boolean",
                    "description": "Show line numbers in output. Requires output_mode: 'content'. Defaults to true.",
                    "default": true
                },
                "i": {
                    "type": "boolean",
                    "description": "Case insensitive search.",
                    "default": false
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case insensitive search (alias for i).",
                    "default": false
                },
                "multiline": {
                    "type": "boolean",
                    "description": "Enable multiline mode where . matches newlines and patterns can span lines. Default: false.",
                    "default": false
                },
                "head_limit": {
                    "type": "integer",
                    "description": "Limit output to first N lines/entries. Defaults to 250 when unspecified. Pass 0 for unlimited.",
                    "minimum": 0
                },
                "offset": {
                    "type": "integer",
                    "description": "Skip first N lines/entries before applying head_limit. Defaults to 0.",
                    "default": 0,
                    "minimum": 0
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
                        "Grep",
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
                    "Grep",
                    format!(
                        "Error: {}: invalid_arguments: missing required 'pattern' parameter",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // `head_limit` / `offset` must be non-negative (contract §错误契约).
        let head_limit = match input.get("head_limit") {
            Some(v) if v.is_i64() => {
                let v = v.as_i64().unwrap();
                if v < 0 {
                    return Ok(ToolExecOutput::Complete(make_result(
                        "Grep",
                        format!(
                            "Error: {}: invalid_arguments: head_limit must be non-negative",
                            ToolErrorCategory::ValidationFailure.as_str()
                        ),
                        ToolResultState::Error,
                    )));
                }
                v as usize
            }
            _ => DEFAULT_HEAD_LIMIT,
        };
        let offset = match input.get("offset") {
            Some(v) if v.is_i64() => {
                let v = v.as_i64().unwrap();
                if v < 0 {
                    return Ok(ToolExecOutput::Complete(make_result(
                        "Grep",
                        format!(
                            "Error: {}: invalid_arguments: offset must be non-negative",
                            ToolErrorCategory::ValidationFailure.as_str()
                        ),
                        ToolResultState::Error,
                    )));
                }
                v as usize
            }
            _ => 0,
        };

        // Output mode (defaults to `files_with_matches`).
        let output_mode = match input.get("output_mode").and_then(JsonValue::as_str) {
            Some("content") => OutputMode::Content,
            Some("files_with_matches") => OutputMode::FilesWithMatches,
            Some("count") => OutputMode::Count,
            Some(other) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Grep",
                    format!(
                        "Error: {}: invalid_arguments: invalid output_mode: {other}",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
            None => OutputMode::FilesWithMatches,
        };

        let case_insensitive = input.get("i").and_then(JsonValue::as_bool).unwrap_or(false)
            || input
                .get("case_insensitive")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false);
        let multiline = input
            .get("multiline")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        let show_line_numbers = input.get("n").and_then(JsonValue::as_bool).unwrap_or(true);

        // Context: `context` wins, then `-C`, else `-B`/`-A` independently
        // (Python arg-building order). Negative values are clamped to 0.
        let context = input.get("context").and_then(JsonValue::as_i64);
        let c = input.get("-C").and_then(JsonValue::as_i64);
        let b = input.get("-B").and_then(JsonValue::as_i64);
        let a = input.get("-A").and_then(JsonValue::as_i64);
        let (context_before, context_after) = if let Some(v) = context {
            let v = v.max(0) as usize;
            (v, v)
        } else if let Some(v) = c {
            let v = v.max(0) as usize;
            (v, v)
        } else {
            (
                b.unwrap_or(0).max(0) as usize,
                a.unwrap_or(0).max(0) as usize,
            )
        };

        // Optional glob filter.
        let glob_set = match input.get("glob").and_then(JsonValue::as_str) {
            Some(g) if !g.trim().is_empty() => match build_glob_set(g) {
                Ok(Some(gs)) => Some(gs),
                Ok(None) => None,
                Err(e) => {
                    return Ok(ToolExecOutput::Complete(make_result(
                        "Grep",
                        format!(
                            "Error: {}: invalid_pattern: invalid glob pattern: {e}",
                            ToolErrorCategory::ValidationFailure.as_str()
                        ),
                        ToolResultState::Error,
                    )));
                }
            },
            _ => None,
        };

        // Optional `rg --type` file-type filter.
        let type_filter = match input.get("type").and_then(JsonValue::as_str) {
            Some(t) if !t.trim().is_empty() => match file_type_extensions(t.trim()) {
                Some(exts) => Some(exts),
                None => {
                    return Ok(ToolExecOutput::Complete(make_result(
                        "Grep",
                        format!(
                            "Error: {}: invalid_arguments: unknown file type: {t}",
                            ToolErrorCategory::ValidationFailure.as_str()
                        ),
                        ToolResultState::Error,
                    )));
                }
            },
            _ => None,
        };

        // Compile the regex; a build failure is an invalid pattern.
        let regex = match build_regex(&pattern, case_insensitive, multiline) {
            Ok(re) => re,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Grep",
                    format!(
                        "Error: {}: invalid_pattern: {e}",
                        ToolErrorCategory::ValidationFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // Resolve the search root: an explicit `path` is confined to the
        // workspace, otherwise the workspace root itself is used.
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
                        "Grep",
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

        // The search path must exist.
        let exists = match self.ctx.backend.file_exists(&base_dir).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Grep",
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
                "Grep",
                format!(
                    "Error: {}: file_not_found: File does not exist: {base_dir}",
                    ToolErrorCategory::ExecutionFailure.as_str()
                ),
                ToolResultState::Error,
            )));
        }
        let is_dir = match self.ctx.backend.is_dir(&base_dir).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(ToolExecOutput::Complete(make_result(
                    "Grep",
                    format!(
                        "Error: {}: file_not_found: {e}",
                        ToolErrorCategory::ExecutionFailure.as_str()
                    ),
                    ToolResultState::Error,
                )));
            }
        };

        // Effective head limit: `0` means unlimited up to the hard cap.
        let effective_head = if head_limit == 0 {
            MAX_GREP_RESULTS
        } else {
            head_limit
        };
        let collect_cap = offset + effective_head + CONTEXT_SLACK;

        let params = SearchParams {
            output_mode,
            regex: &regex,
            multiline,
            show_line_numbers,
            context_before,
            context_after,
            glob: glob_set.as_ref(),
            type_filter,
            collect_cap,
        };

        let mut state = SearchState::default();

        if is_dir {
            // Walk the tree, pruning VCS directories and symlinks. The scan is
            // bounded so a pathological tree cannot stall the host.
            let walker = WalkDir::new(base_dir.as_str())
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !is_vcs_dir(e));
            let mut entries_scanned = 0usize;
            for entry in walker {
                let Ok(entry) = entry else { continue };
                if entry.depth() == 0 {
                    continue;
                }
                entries_scanned += 1;
                if entries_scanned > MAX_GREP_SCAN_FILES
                    || state.files_scanned >= MAX_GREP_SCAN_FILES
                {
                    state.scan_cap_hit = true;
                    break;
                }
                let ft = entry.file_type();
                if ft.is_symlink() || !ft.is_file() {
                    continue;
                }
                let full = entry.path().to_string_lossy().to_string();
                let rel = rel_to_workdir(&self.ctx.workdir, &full);
                self.search_file(&full, &rel, &params, &mut state).await;
                if state.truncated {
                    break;
                }
            }
        } else {
            let rel = rel_to_workdir(&self.ctx.workdir, &base_dir);
            self.search_file(&base_dir, &rel, &params, &mut state).await;
        }

        // No matches — success with the exact Python message.
        if state.output_lines.is_empty() {
            let mut msg = format!("No matches found for pattern: {pattern}");
            let notes = skip_notes(&state);
            if !notes.is_empty() {
                msg.push_str(&format!("; {}", notes.join("; ")));
            }
            return Ok(ToolExecOutput::Complete(make_result(
                "Grep",
                msg,
                ToolResultState::Success,
            )));
        }

        // Apply offset, then head_limit, over the collected output lines.
        let start = offset.min(state.output_lines.len());
        let end = (start + effective_head).min(state.output_lines.len());
        let sliced: Vec<String> = state.output_lines[start..end].to_vec();

        let was_truncated = state.truncated || state.output_lines.len() - start > effective_head;
        let mut out = sliced.join("\n");

        let mut notes = Vec::new();
        if was_truncated {
            let mut page =
                format!("\n\n[Showing results with pagination = limit: {effective_head}");
            if offset > 0 {
                page.push_str(&format!(", offset: {offset}"));
            }
            page.push(']');
            out.push_str(&page);
        }
        notes.extend(skip_notes(&state));
        if !notes.is_empty() {
            out.push_str(&format!("\n\n{}", notes.join("\n")));
        }

        Ok(ToolExecOutput::Complete(make_result(
            "Grep",
            out,
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
    async fn run(tool: &GrepTool, input: JsonValue) -> ToolResultBlock {
        match tool.call(input).await.unwrap() {
            ToolExecOutput::Complete(b) => b,
            _ => panic!("expected Complete"),
        }
    }

    #[tokio::test]
    async fn grep_empty_pattern_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "  " })).await;
        assert_eq!(block.state, ToolResultState::Error);
        let text = text_of(&block);
        assert!(text.contains("invalid_arguments"), "got: {text}");
        assert!(text.contains("pattern must not be empty"), "got: {text}");
    }

    #[tokio::test]
    async fn grep_content_mode_matches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.txt"),
            "hello world\nfoo bar\nhello again\n",
        )
        .unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "hello", "output_mode": "content" }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(text.contains("a.txt:1:hello world"), "got: {text}");
        assert!(text.contains("a.txt:3:hello again"), "got: {text}");
        assert!(!text.contains("foo bar"), "got: {text}");
    }

    #[tokio::test]
    async fn grep_files_with_matches_mode() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello\n").unwrap();
        std::fs::write(dir.path().join("b.txt"), "world\n").unwrap();
        std::fs::write(dir.path().join("sub.txt"), "hello again\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "hello" })).await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.contains(&"a.txt"), "got: {text}");
        assert!(lines.contains(&"sub.txt"), "got: {text}");
        assert!(!lines.contains(&"b.txt"), "got: {text}");
    }

    #[tokio::test]
    async fn grep_count_mode() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello\nnope\nhello\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "hello", "output_mode": "count" }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(text.contains("a.txt:2"), "got: {text}");
    }

    #[tokio::test]
    async fn grep_no_match_is_success() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "zzz_nomatch" })).await;
        assert_eq!(block.state, ToolResultState::Success);
        assert_eq!(text_of(&block), "No matches found for pattern: zzz_nomatch");
    }

    #[tokio::test]
    async fn grep_path_escape_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "x", "path": "../outside" }),
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
    async fn grep_path_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "x", "path": "missing" }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Error);
        assert!(
            text_of(&block).contains("file_not_found"),
            "got: {}",
            text_of(&block)
        );
    }

    #[tokio::test]
    async fn grep_head_limit_applies() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x\nx\nx\nx\nx\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "x", "output_mode": "content", "head_limit": 2 }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        let match_lines = text.lines().filter(|l| l.starts_with("a.txt:")).count();
        assert_eq!(match_lines, 2, "got: {text}");
        assert!(text.contains("limit: 2"), "got: {text}");
    }

    #[tokio::test]
    async fn grep_invalid_regex_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "[" })).await;
        assert_eq!(block.state, ToolResultState::Error);
        assert!(
            text_of(&block).contains("invalid_pattern"),
            "got: {}",
            text_of(&block)
        );
    }

    #[tokio::test]
    async fn grep_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "Hello World\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "hello", "output_mode": "content", "i": true }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Success);
        assert!(
            text_of(&block).contains("a.txt:1:Hello World"),
            "got: {}",
            text_of(&block)
        );
    }

    #[tokio::test]
    async fn grep_skips_vcs_dirs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello\n").unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git").join("config"), "hello\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(&tool, serde_json::json!({ "pattern": "hello" })).await;
        assert_eq!(block.state, ToolResultState::Success);
        assert_eq!(text_of(&block), "a.txt");
    }

    #[tokio::test]
    async fn grep_context_lines_emitted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\nMATCH\nthree\nfour\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "MATCH", "output_mode": "content", "-B": 1, "-A": 1 }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(text.contains("a.txt-2:two"), "got: {text}");
        assert!(text.contains("a.txt:3:MATCH"), "got: {text}");
        assert!(text.contains("a.txt-4:three"), "got: {text}");
    }

    #[tokio::test]
    async fn grep_glob_filter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "hello\n").unwrap();
        std::fs::write(dir.path().join("b.py"), "hello\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "hello", "glob": "*.rs" }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(text.contains("a.rs"), "got: {text}");
        assert!(!text.contains("b.py"), "got: {text}");
    }

    #[tokio::test]
    async fn grep_type_filter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "hello\n").unwrap();
        std::fs::write(dir.path().join("b.py"), "hello\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "hello", "type": "rust" }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(text.contains("a.rs"), "got: {text}");
        assert!(!text.contains("b.py"), "got: {text}");
    }

    #[tokio::test]
    async fn grep_offset_skips_first_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x\nx\nx\nx\n").unwrap();
        let (ctx, _) = ctx_in(&dir);
        let tool = GrepTool::new(ctx);

        let block = run(
            &tool,
            serde_json::json!({ "pattern": "x", "output_mode": "content", "offset": 2 }),
        )
        .await;
        assert_eq!(block.state, ToolResultState::Success);
        let text = text_of(&block);
        assert!(text.contains("a.txt:3:x"), "got: {text}");
        assert!(text.contains("a.txt:4:x"), "got: {text}");
        assert!(
            !text.contains("a.txt:1:x") && !text.contains("a.txt:2:x"),
            "got: {text}"
        );
    }
}
