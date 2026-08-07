use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_scope_memory::{Memory, MemoryEntry, MemoryType};
use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use agent_scope_tool::{FunctionTool, LocalSkillLoader, SkillViewer, ToolKit};
use agent_scope_workspace::Skill;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::config::RuntimeConfig;

const DEFAULT_READ_LIMIT: usize = 400;
const MAX_TOOL_OUTPUT_CHARS: usize = 16_000;
/// Upper bound on a single file read, so a multi-GB file cannot be loaded into
/// memory wholesale and stall/OOM the host.
const MAX_READ_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB

/// Per-file byte cap for Grep so a single huge file (e.g. a multi-GB log)
/// cannot stall the host.  We check the file size before reading; if the file
/// exceeds this limit, grep skips it with a note rather than loading its
/// contents.
pub const MAX_GREP_FILE_BYTES: u64 = 32 * 1024 * 1024; // 32 MiB

/// Maximum entries scanned by Glob before stopping, to prevent a deep tree
/// from stalling the host.
pub const MAX_GLOB_SCAN_ENTRIES: usize = 100_000;

/// Binary-signature check: look for a NUL byte in the first 8 KiB.
const BINARY_CHECK_WINDOW: usize = 8192;

/// Default result cap for Grep matches.
const DEFAULT_GREP_MAX_RESULTS: usize = 50;
/// Hard upper bound on Grep matches regardless of the requested `max_results`.
const MAX_GREP_RESULTS: usize = 500;
/// Per-line character cap in Grep output so a huge line cannot dominate the result.
const MAX_GREP_LINE_CHARS: usize = 200;
/// Stop scanning after this many files so Grep over a huge tree cannot stall the host.
const MAX_GREP_SCAN_FILES: usize = 50_000;
/// Default cap on Glob results.
pub const DEFAULT_GLOB_MAX_RESULTS: usize = 200;
/// Default cap on ListDir entries.
const DEFAULT_LISTDIR_MAX_ENTRIES: usize = 500;

#[derive(Clone)]
pub struct ToolState {
    pub cwd: PathBuf,
    pub command_timeout_secs: u64,
    /// Host-side approved operation fingerprints shared with the REPL. A tool
    /// skips its confirmation gate when the operation fingerprint is present
    /// (the REPL inserted it after the user approved).
    pub approvals: Arc<Mutex<HashSet<String>>>,
    /// Shared long-term memory store, injected by the agent runtime so the
    /// Memory tool and the library's MemoryMiddleware see the same files.
    /// `None` when memory is disabled (`--no-memory`).
    pub memory: Option<Arc<dyn Memory>>,
}

impl ToolState {
    pub fn new(cwd: PathBuf, command_timeout_secs: u64) -> Self {
        Self {
            cwd,
            command_timeout_secs,
            approvals: Arc::new(Mutex::new(HashSet::new())),
            memory: None,
        }
    }

    pub fn from_config(config: &RuntimeConfig) -> Self {
        Self::new(config.cwd.clone(), config.command_timeout_secs)
    }
}

/// Deterministic fingerprint for an operation that requires host confirmation.
///
/// Used on both sides of the confirmation loop — the tool checks it against
/// `ToolState.approvals` before gating, and the render layer derives the same
/// value from the tool-call input so the REPL knows which operation to offer
/// for approval. Keeping both sides on this one function guarantees the
/// fingerprints always agree.
pub fn approval_fingerprint(
    tool_name: &str,
    input_json: &serde_json::Value,
    cwd: &Path,
) -> Option<String> {
    match tool_name {
        "Bash" => input_json
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(|cmd| format!("bash:{}", cmd.trim())),
        "Write" => input_json
            .get("path")
            .and_then(serde_json::Value::as_str)
            .and_then(|path| resolve_workspace_path(cwd, path).ok())
            .map(|path| format!("write:{}", path.display())),
        _ => None,
    }
}

fn is_approved(state: &ToolState, fingerprint: &str) -> bool {
    // A poisoned lock fails closed (treat as not approved) rather than panicking.
    state
        .approvals
        .lock()
        .map(|approvals| approvals.contains(fingerprint))
        .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultShape {
    pub ok: bool,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolErrorShape>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolErrorShape {
    pub code: String,
    pub category: String,
    pub message: String,
    pub retryable: bool,
}

impl ToolResultShape {
    fn ok(summary: impl Into<String>, content: Option<String>) -> Self {
        Self {
            ok: true,
            summary: summary.into(),
            content,
            error: None,
            metadata: serde_json::json!({}),
        }
    }

    fn err(
        code: impl Into<String>,
        category: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        let message = message.into();
        Self {
            ok: false,
            summary: message.clone(),
            content: None,
            error: Some(ToolErrorShape {
                code: code.into(),
                category: category.into(),
                message,
                retryable,
            }),
            metadata: serde_json::json!({}),
        }
    }

    fn into_block(self, name: &str) -> ToolResultBlock {
        let state = if self.ok {
            ToolResultState::Success
        } else if self
            .error
            .as_ref()
            .is_some_and(|err| err.category == "permission")
        {
            ToolResultState::Denied
        } else {
            ToolResultState::Error
        };
        ToolResultBlock {
            id: uuid::Uuid::new_v4().as_simple().to_string(),
            name: name.to_string(),
            output: ToolOutput::Text(
                serde_json::to_string(&self).unwrap_or_else(|_| "{\"ok\":false}".to_string()),
            ),
            state,
            is_last: true,
            metadata: std::collections::HashMap::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ReadInput {
    pub path: String,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WriteInput {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EditInput {
    pub path: String,
    pub old_string: String,
    pub new_string: String,
    #[serde(default)]
    pub replace_all: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct BashInput {
    pub command: String,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GrepInput {
    pub pattern: String,
    /// Subdirectory or file to search under; defaults to the workspace root.
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub case_insensitive: bool,
    #[serde(default)]
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GlobInput {
    /// Glob pattern relative to `path`, e.g. `src/**/*.rs` or `*.txt`.
    pub pattern: String,
    /// Base directory for the pattern; defaults to the workspace root.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ListDirInput {
    pub path: String,
    #[serde(default)]
    pub show_hidden: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct MemoryInput {
    /// Identifier for the memory entry. The tool sanitizes it into a safe ASCII
    /// filename component, so any string works; keep semantic detail in
    /// `description` (which is what appears in the MEMORY.md index).
    pub name: String,
    /// One-line description shown in the MEMORY.md index (may contain any text,
    /// e.g. "the user's name is 张德帅").
    pub description: String,
    /// Memory category: user | feedback | project | reference.
    pub mem_type: String,
    /// Full body of the memory entry.
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    Allow,
    Confirm,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPermissionDecision {
    pub level: PermissionLevel,
    pub reason: String,
}

impl ToolPermissionDecision {
    fn allow(reason: impl Into<String>) -> Self {
        Self {
            level: PermissionLevel::Allow,
            reason: reason.into(),
        }
    }

    fn confirm(reason: impl Into<String>) -> Self {
        Self {
            level: PermissionLevel::Confirm,
            reason: reason.into(),
        }
    }
}

pub fn build_toolkit(state: ToolState, skills: Vec<Skill>, skills_dir: PathBuf) -> ToolKit {
    let state = Arc::new(state);
    let mut toolkit = ToolKit::new();
    // 启动快照注册:供 `get_skill_instructions` 生成 system prompt 的
    // <agent-skills> 静态列表。
    for skill in &skills {
        toolkit.add_skill(skill.clone());
    }
    // Skill 工具实时查询 workspace/skills 目录:运行中复制进来的新 skill
    // 无需重启即可被查看到(复制目录后立即生效)。回调内部每次重新扫描,
    // 而非启动时快照。必须先移除 `ToolKit::new()` 自动注册的默认 SkillViewer
    //(基于 skill_cache 快照),否则 `register` 因同名重复而忽略我们的回调。
    toolkit.remove("Skill");
    toolkit.register(SkillViewer::new(Box::new(move |_groups| {
        LocalSkillLoader::new(&skills_dir.to_string_lossy(), true)
            .list_skills_blocking()
            .into_iter()
            .map(|skill| (skill.name.clone(), skill))
            .collect()
    })));

    let read_state = Arc::clone(&state);
    toolkit.register(FunctionTool::new(
        "Read",
        "Read a UTF-8 text file from the configured project working directory.",
        move |input: ReadInput| {
            let state = Arc::clone(&read_state);
            async move { read_tool(&state, input).into_block("Read") }
        },
    ));

    let write_state = Arc::clone(&state);
    toolkit.register(FunctionTool::new(
        "Write",
        "Create or replace a UTF-8 text file inside the configured project working directory.",
        move |input: WriteInput| {
            let state = Arc::clone(&write_state);
            async move { write_tool(&state, input).into_block("Write") }
        },
    ));

    let edit_state = Arc::clone(&state);
    toolkit.register(FunctionTool::new(
        "Edit",
        "Perform exact string replacement in a UTF-8 text file inside the configured project working directory.",
        move |input: EditInput| {
            let state = Arc::clone(&edit_state);
            async move { edit_tool(&state, input).into_block("Edit") }
        },
    ));

    let bash_state = Arc::clone(&state);
    toolkit.register(FunctionTool::new(
        "Bash",
        "Execute a shell command (or CLI pipeline) in the configured project working directory with timeout and truncation. Use this to run any command the user asks for — fetching data (curl), git operations, builds, tests, or verification.",
        move |input: BashInput| {
            let state = Arc::clone(&bash_state);
            async move { bash_tool(&state, input).await.into_block("Bash") }
        },
    ));

    let grep_state = Arc::clone(&state);
    toolkit.register(FunctionTool::new(
        "Grep",
        "Recursively search for a substring in UTF-8 text files under the workspace, returning file:line:content matches. Prefer limiting the search path to a subdirectory.",
        move |input: GrepInput| {
            let state = Arc::clone(&grep_state);
            async move { grep_tool(&state, input).into_block("Grep") }
        },
    ));

    let glob_state = Arc::clone(&state);
    toolkit.register(FunctionTool::new(
        "Glob",
        "List files under the workspace matching a glob pattern such as **/*.rs or src/**.",
        move |input: GlobInput| {
            let state = Arc::clone(&glob_state);
            async move { glob_tool(&state, input).into_block("Glob") }
        },
    ));

    let listdir_state = Arc::clone(&state);
    toolkit.register(FunctionTool::new(
        "ListDir",
        "List the direct entries (files and subdirectories) of a directory under the workspace.",
        move |input: ListDirInput| {
            let state = Arc::clone(&listdir_state);
            async move { list_dir_tool(&state, input).into_block("ListDir") }
        },
    ));

    let memory_state = Arc::clone(&state);
    toolkit.register(FunctionTool::new(
        "Memory",
        "Persist a fact the user asked you to remember (their name, a preference, a project decision) into long-term memory. Provide a name, a one-line description, a type (user|feedback|project|reference), and the content. The entry is written to disk and shown in MEMORY.md.",
        move |input: MemoryInput| {
            let state = Arc::clone(&memory_state);
            async move { memory_tool(&state, input).await.into_block("Memory") }
        },
    ));

    toolkit
}

#[allow(clippy::result_large_err)]
pub fn resolve_workspace_path(cwd: &Path, input: &str) -> Result<PathBuf, ToolResultShape> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(ToolResultShape::err(
            "invalid_arguments",
            "validation",
            "path must not be empty",
            false,
        ));
    }
    let candidate = PathBuf::from(raw);
    let joined = if candidate.is_absolute() {
        candidate
    } else {
        cwd.join(candidate)
    };
    let normalized = normalize_path(&joined);
    if !normalized.starts_with(cwd) {
        return Err(ToolResultShape::err(
            "path_outside_workspace",
            "permission",
            "path escapes configured project directory",
            false,
        ));
    }

    // Symlink containment check: a workspace that contains `link -> /etc` (planted
    // by a previous command) would pass the lexical `starts_with` check but
    // resolve outside the workspace, letting Read/Write/Edit touch host files
    // (audit S5). Resolve the real path and re-check containment.
    let existing_ancestor = {
        let mut p = normalized.as_path();
        let mut deepest: Option<PathBuf> = None;
        loop {
            if p.exists() {
                deepest = Some(p.to_path_buf());
                break;
            }
            let Some(parent) = p.parent() else { break };
            if parent == p {
                break;
            }
            p = parent;
        }
        deepest
    };
    if let Some(existing) = existing_ancestor {
        let canon = existing.canonicalize().map_err(|e| {
            ToolResultShape::err(
                "path_resolution",
                "io",
                format!("failed to resolve path: {e}"),
                false,
            )
        })?;
        // The resolved ancestor must still live inside the real workspace root.
        let real_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        if !canon.starts_with(&real_cwd) {
            return Err(ToolResultShape::err(
                "path_outside_workspace",
                "permission",
                "path resolves through a symlink outside the workspace",
                false,
            ));
        }
    }

    Ok(normalized)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn read_tool(state: &ToolState, input: ReadInput) -> ToolResultShape {
    let path = match resolve_workspace_path(&state.cwd, &input.path) {
        Ok(path) => path,
        Err(err) => return err,
    };
    if !path.exists() {
        return ToolResultShape::err("file_not_found", "io", "target file does not exist", false);
    }
    if path.is_dir() {
        return ToolResultShape::err(
            "unsupported_file_type",
            "validation",
            "target is a directory",
            false,
        );
    }
    // Guard against reading an arbitrarily large file (e.g. a multi-GB log or
    // `dd` output) into memory, which would stall or OOM the agent host.
    if let Ok(meta) = fs::metadata(&path)
        && meta.len() > MAX_READ_BYTES
    {
        return ToolResultShape::err(
            "file_too_large",
            "validation",
            format!("file exceeds {MAX_READ_BYTES} byte read limit"),
            false,
        );
    }
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            return ToolResultShape::err("permission_denied", "permission", err.to_string(), false);
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return ToolResultShape::err(
                "unsupported_file_type",
                "validation",
                "file is not valid UTF-8 text",
                false,
            );
        }
    };
    let offset = input.offset.unwrap_or(0);
    let limit = input.limit.unwrap_or(DEFAULT_READ_LIMIT);
    let total_lines = text.lines().count();
    let mut lines: Vec<String> = text
        .lines()
        .enumerate()
        .skip(offset)
        .take(limit)
        .map(|(idx, line)| format!("{}\t{}", idx + 1, line))
        .collect();
    if offset + lines.len() < total_lines {
        lines.push(format!(
            "... truncated: showing {} lines from offset {}, total {} lines",
            lines.len(),
            offset,
            total_lines
        ));
    }
    ToolResultShape::ok(
        format!(
            "read {} line(s) from {}",
            lines.len(),
            display_rel(&state.cwd, &path)
        ),
        Some(lines.join("\n")),
    )
}

pub fn write_tool(state: &ToolState, input: WriteInput) -> ToolResultShape {
    let path = match resolve_workspace_path(&state.cwd, &input.path) {
        Ok(path) => path,
        Err(err) => return err,
    };
    let decision = classify_write_permission(state, &input, &path);
    if decision.level == PermissionLevel::Confirm && !input.confirmed {
        let fingerprint = format!("write:{}", path.display());
        if !is_approved(state, &fingerprint) {
            return ToolResultShape::err(
                "confirmation_required",
                "permission",
                decision.reason,
                false,
            );
        }
    }
    if path.exists() && !input.overwrite {
        return ToolResultShape::err(
            "file_exists",
            "validation",
            "target file exists and overwrite was not allowed",
            false,
        );
    }
    if let Some(parent) = path.parent()
        && let Err(err) = fs::create_dir_all(parent)
    {
        return ToolResultShape::err("permission_denied", "permission", err.to_string(), false);
    }
    match fs::write(&path, input.content.as_bytes()) {
        Ok(()) => ToolResultShape::ok(
            format!(
                "wrote {} bytes to {}",
                input.content.len(),
                display_rel(&state.cwd, &path)
            ),
            None,
        ),
        Err(err) => ToolResultShape::err("permission_denied", "permission", err.to_string(), false),
    }
}

pub fn edit_tool(state: &ToolState, input: EditInput) -> ToolResultShape {
    let path = match resolve_workspace_path(&state.cwd, &input.path) {
        Ok(path) => path,
        Err(err) => return err,
    };
    if !path.exists() {
        return ToolResultShape::err("file_not_found", "io", "target file does not exist", false);
    }
    // Same size guard as read_tool (audit: oversized file reads).
    if let Ok(meta) = fs::metadata(&path)
        && meta.len() > MAX_READ_BYTES
    {
        return ToolResultShape::err(
            "file_too_large",
            "validation",
            format!("file exceeds {MAX_READ_BYTES} byte read limit"),
            false,
        );
    }
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) => {
            return ToolResultShape::err(
                "unsupported_file_type",
                "validation",
                err.to_string(),
                false,
            );
        }
    };
    let matches = content.matches(&input.old_string).count();
    if matches == 0 {
        return ToolResultShape::err(
            "pattern_not_found",
            "validation",
            "old_string was not found",
            false,
        );
    }
    if matches > 1 && !input.replace_all {
        return ToolResultShape::err(
            "ambiguous_edit",
            "validation",
            "old_string occurs more than once",
            false,
        );
    }
    let new_content = if input.replace_all {
        content.replace(&input.old_string, &input.new_string)
    } else {
        content.replacen(&input.old_string, &input.new_string, 1)
    };
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().as_simple()));
    if let Err(err) = fs::write(&tmp, new_content.as_bytes()) {
        return ToolResultShape::err("permission_denied", "permission", err.to_string(), false);
    }
    if let Err(err) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return ToolResultShape::err("permission_denied", "permission", err.to_string(), false);
    }
    ToolResultShape::ok(
        format!(
            "replaced {matches} occurrence(s) in {}",
            display_rel(&state.cwd, &path)
        ),
        None,
    )
}

pub async fn bash_tool(state: &ToolState, input: BashInput) -> ToolResultShape {
    let command = input.command.trim();
    if command.is_empty() {
        return ToolResultShape::err(
            "invalid_arguments",
            "validation",
            "command must not be empty",
            false,
        );
    }
    if is_destructive_command(command) && !input.confirmed {
        let fingerprint = format!("bash:{command}");
        if !is_approved(state, &fingerprint) {
            return ToolResultShape::err(
                "confirmation_required",
                "permission",
                "command requires confirmation before execution",
                false,
            );
        }
    }
    let timeout = Duration::from_secs(input.timeout_secs.unwrap_or(state.command_timeout_secs));
    run_shell_command(state, command, timeout).await
}

pub fn classify_write_permission(
    cwd: &ToolState,
    input: &WriteInput,
    resolved_path: &Path,
) -> ToolPermissionDecision {
    if resolved_path.exists() && input.overwrite {
        ToolPermissionDecision::confirm(format!(
            "overwriting existing file {} requires confirmation",
            display_rel(&cwd.cwd, resolved_path)
        ))
    } else {
        ToolPermissionDecision::allow("new file write is allowed")
    }
}

pub fn classify_bash_permission(input: &BashInput) -> ToolPermissionDecision {
    if is_destructive_command(&input.command) {
        ToolPermissionDecision::confirm(
            "command is potentially destructive and requires confirmation",
        )
    } else {
        ToolPermissionDecision::allow("safe command is allowed")
    }
}

async fn run_shell_command(state: &ToolState, command: &str, timeout: Duration) -> ToolResultShape {
    let child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(&state.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // If the timeout fires, `wait_with_output` is dropped; make sure the
        // child is killed rather than leaked (a `sh -c 'sleep 1000 &'` would
        // otherwise keep running in the background forever).
        .kill_on_drop(true)
        .spawn();
    let child = match child {
        Ok(child) => child,
        Err(err) => return ToolResultShape::err("command_failed", "tool", err.to_string(), true),
    };
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let content = truncate_output(&format!(
                "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
                output.status.code().unwrap_or(-1),
                stdout,
                stderr
            ));
            if output.status.success() {
                ToolResultShape::ok("command succeeded", Some(content))
            } else {
                let mut result = ToolResultShape::err(
                    "command_failed",
                    "tool",
                    format!("command exited with status {}", output.status),
                    true,
                );
                result.content = Some(content);
                result
            }
        }
        Ok(Err(err)) => ToolResultShape::err("command_failed", "tool", err.to_string(), true),
        Err(_) => ToolResultShape::err(
            "command_timeout",
            "tool",
            format!("command exceeded {} seconds", timeout.as_secs()),
            true,
        ),
    }
}

/// Heuristic classifier for potentially destructive shell commands.
///
/// **This is a risk-hint, not a sandbox.**  The corpus covers common dangerous
/// patterns but is inherently incomplete — a determined adversary (or an
/// unthinking agent) could still cause harm through un-listed commands,
/// obfuscation, or side-effects via intermediary scripts.  The true safety
/// boundary is the filesystem isolation (workspace containment, no `sudo`
/// without password, `kill_on_drop` for subprocesses).  This function gates
/// the "ask the host before executing" UX; it should err on the side of
/// caution (classifying potentially risky operations as destructive).
pub fn is_destructive_command(command: &str) -> bool {
    let lowered = command.to_ascii_lowercase().trim().to_string();
    let tokens: Vec<&str> = lowered.split_whitespace().collect();

    // Commands whose *first token* is inherently dangerous, even without flags.
    let destructive_first_token = matches!(
        tokens.first().copied(),
        Some(
            "rm" | "unlink"
                | "rmdir"
                | "dd"
                | "truncate"
                | "shred"
                | "chmod"
                | "chown"
                | "chgrp"
                | "kill"
                | "pkill"
                | "killall"
                | "reboot"
                | "shutdown"
                | "halt"
                | "poweroff"
                | "sudo"
                | "su"
        )
    ) || tokens
        .first()
        .is_some_and(|t| t.starts_with("mkfs") || t.starts_with("mkswap"));

    // `tee` is dangerous when it redirects, overwrites, or appends.
    let tee_dangerous = tokens.first() == Some(&"tee") && lowered.contains(">");

    // Commands that *can* be destructive in certain argument configurations
    // (checked below via substrings / multi-token patterns).
    let find_delete =
        lowered.contains("find") && (lowered.contains("-delete") || lowered.contains("-exec rm"));
    let git_destructive = lowered.contains("git reset")
        || lowered.contains("git clean")
        || lowered.contains("git checkout .")
        || lowered.contains("git stash drop")
        || lowered.contains("git push --force")
        || lowered.contains("git push -f")
        || lowered.contains("git branch -d")
        || lowered.contains("git branch -D");
    let cargo_install = lowered.contains("cargo install");
    let package_install = lowered.contains("npm install")
        || lowered.contains("pnpm install")
        || lowered.contains("yarn install")
        || lowered.contains("pip install")
        || lowered.contains("pip3 install")
        || lowered.contains("gem install");
    let piped_to_shell =
        (lowered.contains("curl ") || lowered.contains("wget ")) && lowered.contains("| sh");
    let redirect = lowered.contains('>') || lowered.contains(">>");
    let python_exec = tokens.first() == Some(&"python")
        || tokens.first() == Some(&"python3")
        || tokens.first() == Some(&"python2");
    let python_c_eval = python_exec && (lowered.contains("-c") || lowered.contains("-m"));
    let node_eval = tokens.first() == Some(&"node") && lowered.contains("-e");
    let perl_eval = tokens.first() == Some(&"perl")
        && (lowered.contains("-e") || lowered.contains("-ne") || lowered.contains("-pe"));
    let ruby_eval = tokens.first() == Some(&"ruby")
        && (lowered.contains("-e") || lowered.contains("-ne") || lowered.contains("-pe"));
    let mv_danger = tokens.first() == Some(&"mv");
    let cp_r = tokens.first() == Some(&"cp") && lowered.contains("-r");
    let mount_umount = matches!(tokens.first().copied(), Some("mount" | "umount"));
    let systemctl_danger = tokens.first() == Some(&"systemctl")
        && (lowered.contains("stop")
            || lowered.contains("disable")
            || lowered.contains("mask")
            || lowered.contains("halt")
            || lowered.contains("poweroff")
            || lowered.contains("reboot"));
    let docker_danger = tokens.first() == Some(&"docker")
        && (lowered.contains("rm ")
            || lowered.contains("rmi ")
            || lowered.contains("prune")
            || lowered.contains("system prune"));
    let fdisk = tokens.first() == Some(&"fdisk") || tokens.first() == Some(&"parted");
    // `eval` re-interprets an arbitrary string as shell source — the classic
    // `eval "$(curl ...)"` bypass (round-5 H2). Flag on first token.
    let eval_danger = tokens.first() == Some(&"eval");
    // `source` / bare `.` execute a script in the current shell. A lone `.`
    // token (whitespace-delimited) is the POSIX source shorthand.
    let source_danger = tokens.first() == Some(&"source") || tokens.first() == Some(&".");
    // `xargs` feeds stdin to another command as arguments; a destructive
    // downstream command (e.g. `find ... | xargs rm`) is not caught by
    // first-token matching alone. xargs may appear mid-pipeline, so match on
    // its presence + a destructive downstream verb, not the first token
    // (round-5 H2).
    let xargs_danger = lowered.contains("xargs")
        && (lowered.contains(" rm")
            || lowered.contains("shred")
            || lowered.contains("chmod")
            || lowered.contains("chown")
            || lowered.contains("chgrp")
            || lowered.contains("kill")
            || lowered.contains("dd ")
            || lowered.contains(" mv")
            || lowered.contains("truncate"));

    destructive_first_token
        || tee_dangerous
        || find_delete
        || git_destructive
        || cargo_install
        || package_install
        || piped_to_shell
        || redirect
        || python_c_eval
        || node_eval
        || perl_eval
        || ruby_eval
        || mv_danger
        || cp_r
        || mount_umount
        || systemctl_danger
        || docker_danger
        || fdisk
        || eval_danger
        || source_danger
        || xargs_danger
}

pub fn truncate_output(text: &str) -> String {
    if text.chars().count() <= MAX_TOOL_OUTPUT_CHARS {
        return text.to_string();
    }
    let truncated: String = text.chars().take(MAX_TOOL_OUTPUT_CHARS).collect();
    format!("{truncated}\n... truncated output to {MAX_TOOL_OUTPUT_CHARS} characters")
}

/// Convert a glob pattern into an anchored regular expression.
///
/// Supported syntax: `*` (within a path segment), `**` (across directories),
/// `?` (single non-separator character). Everything else is matched literally.
/// `**/` is expanded to `(?:.*/)?` so `src/**/*.rs` also matches `src/main.rs`.
pub fn glob_to_regex(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len() * 2);
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    if i + 2 < chars.len() && chars[i + 2] == '/' {
                        out.push_str("(?:.*/)?");
                        i += 3; // consume `**/`
                    } else {
                        out.push_str(".*");
                        i += 2;
                    }
                } else {
                    out.push_str("[^/]*");
                    i += 1;
                }
            }
            '?' => {
                out.push_str("[^/]");
                i += 1;
            }
            other => {
                out.push_str(&regex::escape(&other.to_string()));
                i += 1;
            }
        }
    }
    out
}

pub fn grep_tool(state: &ToolState, input: GrepInput) -> ToolResultShape {
    let pattern = input.pattern.trim();
    if pattern.is_empty() {
        return ToolResultShape::err(
            "invalid_arguments",
            "validation",
            "pattern must not be empty",
            false,
        );
    }
    let base = match resolve_workspace_path(&state.cwd, input.path.as_deref().unwrap_or(".")) {
        Ok(path) => path,
        Err(err) => return err,
    };
    if !base.exists() {
        return ToolResultShape::err("file_not_found", "io", "search path does not exist", false);
    }
    let max_results = input
        .max_results
        .unwrap_or(DEFAULT_GREP_MAX_RESULTS)
        .min(MAX_GREP_RESULTS);
    let needle = if input.case_insensitive {
        pattern.to_lowercase()
    } else {
        pattern.to_string()
    };

    let mut matches: Vec<String> = Vec::new();
    let mut matched_files: HashSet<String> = HashSet::new();
    let mut files_scanned = 0usize;
    let mut files_skipped_large = 0usize;
    let mut files_skipped_binary = 0usize;
    let mut scan_limit_hit = false;
    let mut stack = vec![base];
    while let Some(dir) = stack.pop() {
        if files_scanned >= MAX_GREP_SCAN_FILES {
            scan_limit_hit = true;
            break;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if matches.len() >= max_results {
                break;
            }
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue; // skip hidden entries (covers .git, .pi-rust, etc.)
            }
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue; // never follow symlinks (avoids escaping the workspace)
            }
            if ft.is_dir() {
                stack.push(entry.path());
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            files_scanned += 1;

            // Per-file size guard: skip files larger than MAX_GREP_FILE_BYTES
            // so a single huge file (e.g. multi-GB log) cannot stall the host.
            let file_path = entry.path();
            let file_size = fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
            if file_size > MAX_GREP_FILE_BYTES {
                files_skipped_large += 1;
                continue;
            }

            // Binary guard: peek at the first 8 KiB for a NUL byte.
            if is_binary_file(&file_path) {
                files_skipped_binary += 1;
                continue;
            }

            let Ok(text) = fs::read_to_string(&file_path) else {
                continue;
            }; // skip non-UTF8
            for (idx, line) in text.lines().enumerate() {
                let hay = if input.case_insensitive {
                    line.to_lowercase()
                } else {
                    line.to_string()
                };
                if hay.contains(&needle) {
                    let rel = display_rel(&state.cwd, &file_path);
                    matched_files.insert(rel.clone());
                    let line_capped: String = line.chars().take(MAX_GREP_LINE_CHARS).collect();
                    matches.push(format!("{rel}:{}:{line_capped}", idx + 1));
                    if matches.len() >= max_results {
                        break;
                    }
                }
            }
        }
    }
    if matches.is_empty() {
        let mut skip_note = String::new();
        if files_skipped_large > 0 {
            skip_note.push_str(&format!(
                "; skipped {files_skipped_large} file(s) exceeding {MAX_GREP_FILE_BYTES} byte limit"
            ));
        }
        if files_skipped_binary > 0 {
            skip_note.push_str(&format!("; skipped {files_skipped_binary} binary file(s)"));
        }
        return ToolResultShape::ok(format!("no matches for {pattern:?}{skip_note}"), None);
    }
    let limit_note = if scan_limit_hit {
        "; scan hit the file cap, results may be incomplete"
    } else {
        ""
    };
    let mut skip_note = String::new();
    if files_skipped_large > 0 {
        skip_note.push_str(&format!("; skipped {files_skipped_large} large file(s)"));
    }
    if files_skipped_binary > 0 {
        skip_note.push_str(&format!("; skipped {files_skipped_binary} binary file(s)"));
    }
    ToolResultShape::ok(
        format!(
            "{} match(es) in {} file(s){limit_note}{skip_note}",
            matches.len(),
            matched_files.len()
        ),
        Some(truncate_output(&matches.join("\n"))),
    )
}

pub fn glob_tool(state: &ToolState, input: GlobInput) -> ToolResultShape {
    let pattern = input.pattern.trim();
    if pattern.is_empty() {
        return ToolResultShape::err(
            "invalid_arguments",
            "validation",
            "pattern must not be empty",
            false,
        );
    }
    let base = match resolve_workspace_path(&state.cwd, input.path.as_deref().unwrap_or(".")) {
        Ok(path) => path,
        Err(err) => return err,
    };
    if !base.exists() {
        return ToolResultShape::err("file_not_found", "io", "glob path does not exist", false);
    }
    let regex_str = glob_to_regex(pattern);
    let re = match Regex::new(&format!("^{regex_str}$")) {
        Ok(re) => re,
        Err(err) => {
            return ToolResultShape::err(
                "invalid_pattern",
                "validation",
                format!("invalid glob pattern: {err}"),
                false,
            );
        }
    };

    let mut files: Vec<String> = Vec::new();
    let mut entries_scanned = 0usize;
    let mut scan_cap_hit = false;
    let mut stack = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if files.len() >= DEFAULT_GLOB_MAX_RESULTS {
                break;
            }
            entries_scanned += 1;
            if entries_scanned > MAX_GLOB_SCAN_ENTRIES {
                scan_cap_hit = true;
                break;
            }
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file()
                && let Ok(rel) = entry.path().strip_prefix(&base)
            {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if re.is_match(&rel_str) {
                    files.push(rel_str);
                }
            }
        }
        if scan_cap_hit {
            break;
        }
    }
    if files.is_empty() {
        let cap_note = if scan_cap_hit {
            format!(" (scan stopped at {MAX_GLOB_SCAN_ENTRIES} entries)")
        } else {
            String::new()
        };
        return ToolResultShape::ok(format!("no files match {pattern:?}{cap_note}"), None);
    }
    files.sort();
    let cap_note = if scan_cap_hit {
        format!("; scan stopped at {MAX_GLOB_SCAN_ENTRIES} entries, results may be incomplete")
    } else {
        String::new()
    };
    ToolResultShape::ok(
        format!("{} file(s) match {pattern:?}{cap_note}", files.len()),
        Some(truncate_output(&files.join("\n"))),
    )
}

pub fn list_dir_tool(state: &ToolState, input: ListDirInput) -> ToolResultShape {
    let path = match resolve_workspace_path(&state.cwd, &input.path) {
        Ok(path) => path,
        Err(err) => return err,
    };
    if !path.exists() {
        return ToolResultShape::err("file_not_found", "io", "directory does not exist", false);
    }
    if path.is_file() {
        return ToolResultShape::err(
            "unsupported_file_type",
            "validation",
            "target is a file",
            false,
        );
    }
    let read_dir = match fs::read_dir(&path) {
        Ok(rd) => rd,
        Err(err) => {
            return ToolResultShape::err("permission_denied", "permission", err.to_string(), false);
        }
    };
    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    for entry in read_dir.flatten() {
        if dirs.len() + files.len() >= DEFAULT_LISTDIR_MAX_ENTRIES {
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !input.show_hidden && name.starts_with('.') {
            continue;
        }
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            dirs.push(format!("{name}/"));
        } else {
            files.push(name);
        }
    }
    dirs.sort();
    files.sort();
    let mut entries = dirs;
    entries.extend(files);
    ToolResultShape::ok(
        format!(
            "listed {} entr(ies) in {}",
            entries.len(),
            display_rel(&state.cwd, &path)
        ),
        Some(entries.join("\n")),
    )
}

fn display_rel(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}

/// Persist a fact the user asked to be remembered into the shared long-term
/// memory store. The write is durable on disk (`workdir/Memory/{name}.md` plus
/// the MEMORY.md index line), so the next reply in this session and any later
/// session that re-reads the index can see it.
pub async fn memory_tool(state: &ToolState, input: MemoryInput) -> ToolResultShape {
    let Some(memory) = &state.memory else {
        return ToolResultShape::err(
            "memory_disabled",
            "validation",
            "memory is disabled (--no-memory)",
            false,
        );
    };
    let name = input.name.trim();
    if name.is_empty() {
        return ToolResultShape::err(
            "invalid_arguments",
            "validation",
            "name must not be empty",
            false,
        );
    }
    let description = input.description.trim();
    if description.is_empty() {
        return ToolResultShape::err(
            "invalid_arguments",
            "validation",
            "description must not be empty",
            false,
        );
    }
    let mem_type = MemoryType::from(input.mem_type.as_str());
    if matches!(mem_type, MemoryType::Unknown(_)) {
        return ToolResultShape::err(
            "invalid_arguments",
            "validation",
            "mem_type must be one of: user, feedback, project, reference",
            false,
        );
    }
    let safe_name = sanitize_memory_name(name);
    let entry = MemoryEntry::new(
        safe_name.clone(),
        description,
        mem_type.clone(),
        input.content,
    );
    match memory.write(entry).await {
        Ok(()) => ToolResultShape::ok(
            format!("Saved memory '{safe_name}'."),
            Some(format!(
                "description: {description}\ntype: {}",
                mem_type.as_str()
            )),
        ),
        Err(err) => ToolResultShape::err(
            "memory_write_failed",
            "internal",
            format!("failed to write memory: {err}"),
            false,
        ),
    }
}

/// Reduce an arbitrary memory name to a safe, stable ASCII filename component
/// (`^[A-Za-z0-9_-]+$`, enforced by `agent_scope_memory`'s `validate_name`).
///
/// ASCII letters/digits/`-`/`_` are kept; any other character collapses into a
/// single separator. If the result is empty (e.g. a purely CJK name) or any
/// non-ASCII character was dropped, a short hash of the original is appended so
/// two distinct names still map to distinct files instead of colliding on the
/// bare prefix. Semantic detail lives in the entry `description`, not the name.
fn sanitize_memory_name(raw: &str) -> String {
    let mut slug = String::with_capacity(raw.len());
    let mut has_non_ascii = false;
    let mut prev_sep = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            slug.push(ch);
            prev_sep = false;
        } else {
            has_non_ascii = true;
            if !prev_sep && !slug.is_empty() {
                slug.push('-');
                prev_sep = true;
            }
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if has_non_ascii || slug.is_empty() {
        let hash = short_hash(raw);
        if slug.is_empty() {
            slug = format!("mem-{hash}");
        } else {
            slug = format!("{slug}-{hash}");
        }
    }
    slug
}

fn short_hash(input: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    let full = format!("{:x}", hasher.finish());
    full[..8.min(full.len())].to_string()
}

/// Quick binary-signature check: peek at the first `BINARY_CHECK_WINDOW` bytes
/// and look for a NUL byte.  Returns true when the file is likely binary.
fn is_binary_file(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = vec![0u8; BINARY_CHECK_WINDOW];
    let n = file.read(&mut buf).unwrap_or(0);
    buf[..n].contains(&0)
}
