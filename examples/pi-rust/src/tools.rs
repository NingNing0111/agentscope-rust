use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use agent_scope_tool::{FunctionTool, SkillViewer, ToolKit};
use agent_scope_workspace::Skill;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::config::RuntimeConfig;

const DEFAULT_READ_LIMIT: usize = 400;
const MAX_TOOL_OUTPUT_CHARS: usize = 16_000;
/// Upper bound on a single file read, so a multi-GB file cannot be loaded into
/// memory wholesale and stall/OOM the host.
const MAX_READ_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB

#[derive(Debug, Clone)]
pub struct ToolState {
    pub cwd: PathBuf,
    pub command_timeout_secs: u64,
}

impl ToolState {
    pub fn from_config(config: &RuntimeConfig) -> Self {
        Self {
            cwd: config.cwd.clone(),
            command_timeout_secs: config.command_timeout_secs,
        }
    }
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

pub fn build_toolkit(state: ToolState, skills: Vec<Skill>) -> ToolKit {
    let state = Arc::new(state);
    let mut toolkit = ToolKit::new();
    if skills.is_empty() {
        toolkit.remove("Skill");
    } else {
        toolkit.remove("Skill");
        for skill in &skills {
            toolkit.add_skill(skill.clone());
        }
        let skill_map: HashMap<String, Skill> = skills
            .into_iter()
            .map(|skill| (skill.name.clone(), skill))
            .collect();
        toolkit.register(SkillViewer::new(Box::new(move |_groups| skill_map.clone())));
    }

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
        "Execute a shell command in the configured project working directory with timeout and truncation.",
        move |input: BashInput| {
            let state = Arc::clone(&bash_state);
            async move { bash_tool(&state, input).await.into_block("Bash") }
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
        let canon = existing
            .canonicalize()
            .map_err(|e| {
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
        return ToolResultShape::err(
            "confirmation_required",
            "permission",
            decision.reason,
            false,
        );
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
        return ToolResultShape::err(
            "confirmation_required",
            "permission",
            "command requires confirmation before execution",
            false,
        );
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

pub fn is_destructive_command(command: &str) -> bool {
    let lowered = command.to_ascii_lowercase();
    let tokens: Vec<&str> = lowered.split_whitespace().collect();
    matches!(tokens.first().copied(), Some("rm" | "unlink" | "rmdir"))
        || lowered.contains("git reset")
        || lowered.contains("git clean")
        || lowered.contains("git checkout .")
        || lowered.contains("git stash")
        || lowered.contains("cargo install")
        || lowered.contains("npm install")
        || lowered.contains("pnpm install")
        || lowered.contains("yarn install")
        || lowered.contains("curl ") && lowered.contains("| sh")
        || lowered.contains("wget ") && lowered.contains("| sh")
        || lowered.contains(">")
        || lowered.contains(">>")
}

pub fn truncate_output(text: &str) -> String {
    if text.chars().count() <= MAX_TOOL_OUTPUT_CHARS {
        return text.to_string();
    }
    let truncated: String = text.chars().take(MAX_TOOL_OUTPUT_CHARS).collect();
    format!("{truncated}\n... truncated output to {MAX_TOOL_OUTPUT_CHARS} characters")
}

fn display_rel(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}
