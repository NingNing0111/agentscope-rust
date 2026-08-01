use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{io, io::Write};

use agent_scope_memory::{Memory, MemoryEntry, MemoryType};
use agent_scope_tool::{FunctionTool, SkillViewer, ToolKit};
use agent_scope_workspace::{LocalWorkspace, Skill, WorkspaceBase};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone)]
pub struct ToolState {
    pub workspace: Option<WorkspaceSnapshot>,
    pub workspace_exec: Option<Arc<LocalWorkspace>>,
    pub memory_store: Option<Arc<dyn Memory>>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceSnapshot {
    pub workspace_id: String,
    pub workdir: String,
    pub is_alive: bool,
    pub instructions_summary: String,
    pub tools: Vec<WorkspaceToolSummary>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceToolSummary {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    pub enabled: bool,
    pub workdir: String,
    pub memory_dir: String,
}

#[derive(Debug, Clone)]
pub struct RagSnapshot {
    pub enabled: bool,
    pub mode: String,
    pub sources: Vec<String>,
    pub chunk_count: usize,
    pub collection: String,
    pub embedding_model: String,
}

pub fn build_toolkit(state: ToolState, skills: Vec<Skill>) -> ToolKit {
    let mut toolkit = ToolKit::new();
    toolkit.register(FunctionTool::new(
        "calculator",
        "Evaluate a safe arithmetic expression. Use this for arithmetic questions. Supported operators: +, -, *, /, parentheses, and unary minus.",
        calculator,
    ));
    toolkit.register(FunctionTool::new(
        "safe_time",
        "Return the current real local and UTC time. Use this when the user asks for current time.",
        safe_time,
    ));

    if skills.is_empty() {
        toolkit.remove("Skill");
    } else {
        for skill in &skills {
            toolkit.add_skill(skill.clone());
        }
        let skill_map = skills
            .into_iter()
            .map(|skill| (skill.name.clone(), skill))
            .collect::<HashMap<_, _>>();
        toolkit.register(SkillViewer::new(Box::new(move |_groups| skill_map.clone())));
    }

    let state = Arc::new(state);

    if state.workspace.is_some() {
        let workspace_state = Arc::clone(&state);
        toolkit.register(FunctionTool::new(
            "workspace_info",
            "Return information about the active LocalWorkspace.",
            move |_input: EmptyInput| {
                let state = Arc::clone(&workspace_state);
                async move { workspace_info(state) }
            },
        ));

        let workspace_tools_state = Arc::clone(&state);
        toolkit.register(FunctionTool::new(
            "workspace_list_tools",
            "List the active LocalWorkspace tool inventory by name and description without executing those tools.",
            move |_input: EmptyInput| {
                let state = Arc::clone(&workspace_tools_state);
                async move { workspace_list_tools(state) }
            },
        ));

        let workspace_bash_state = Arc::clone(&state);
        toolkit.register(FunctionTool::new(
            "Bash",
            "Execute a shell command inside the active LocalWorkspace. Low-risk diagnostics run directly; potentially risky commands require terminal confirmation. Empty, long-lived, and path-escape commands are rejected.",
            move |input: BashInput| {
                let state = Arc::clone(&workspace_bash_state);
                async move { workspace_bash(state, input).await }
            },
        ));

        let workspace_write_state = Arc::clone(&state);
        toolkit.register(FunctionTool::new(
            "workspace_write_file",
            "Write a UTF-8 text file inside the active LocalWorkspace after interactive terminal confirmation. Use this when the user asks to create or write a file.",
            move |input: WorkspaceWriteFileInput| {
                let state = Arc::clone(&workspace_write_state);
                async move { workspace_write_file(state, input).await }
            },
        ));
    }

    if state.memory_store.is_some() {
        let memory_write_state = Arc::clone(&state);
        toolkit.register(FunctionTool::new(
            "memory_write",
            "Save or update a durable memory entry. Use this when the user asks you to remember stable preferences, project facts, feedback, or references. Never store secrets.",
            move |input: MemoryWriteInput| {
                let state = Arc::clone(&memory_write_state);
                async move { memory_write(state, input).await }
            },
        ));

        let memory_search_state = Arc::clone(&state);
        toolkit.register(FunctionTool::new(
            "memory_search",
            "Search durable memories by keyword and optional type filter before answering recall questions.",
            move |input: MemorySearchInput| {
                let state = Arc::clone(&memory_search_state);
                async move { memory_search(state, input).await }
            },
        ));

        let memory_read_state = Arc::clone(&state);
        toolkit.register(FunctionTool::new(
            "memory_read",
            "Read one durable memory by exact name when you already know the memory name.",
            move |input: MemoryReadInput| {
                let state = Arc::clone(&memory_read_state);
                async move { memory_read(state, input).await }
            },
        ));

        let memory_list_state = Arc::clone(&state);
        toolkit.register(FunctionTool::new(
            "memory_list",
            "List durable memory entries so you can discover available memory names and descriptions.",
            move |input: MemoryListInput| {
                let state = Arc::clone(&memory_list_state);
                async move { memory_list(state, input).await }
            },
        ));
    }

    toolkit
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct EmptyInput {}

fn workspace_info(state: Arc<ToolState>) -> String {
    match &state.workspace {
        Some(workspace) => format!(
            "Workspace is enabled.\nworkspace_id: {}\nworkdir: {}\nis_alive: {}\ninstructions: {}",
            workspace.workspace_id,
            workspace.workdir,
            workspace.is_alive,
            workspace.instructions_summary
        ),
        None => "Workspace is not available for this run.".to_string(),
    }
}

fn workspace_list_tools(state: Arc<ToolState>) -> String {
    let Some(workspace) = &state.workspace else {
        return "Workspace is not available for this run.".to_string();
    };

    if workspace.tools.is_empty() {
        return "Workspace is enabled but reported no built-in tools.".to_string();
    }

    let tools = workspace
        .tools
        .iter()
        .map(|tool| format!("- {}: {}", tool.name, tool.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!("Workspace built-in tool inventory:\n{tools}")
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct BashInput {
    /// Shell command to execute in the active workspace.
    command: String,
}

async fn workspace_bash(state: Arc<ToolState>, input: BashInput) -> String {
    let Some(workspace) = &state.workspace_exec else {
        return "Bash is not available for this run because LocalWorkspace execution is disabled."
            .to_string();
    };

    let command = input.command.trim();
    match classify_bash_command(command) {
        Ok(BashCommandPolicy::Allow) => {}
        Ok(BashCommandPolicy::Confirm(reason)) => {
            if let Err(err) = confirm_bash_command(workspace.workdir(), command, &reason) {
                return err;
            }
        }
        Err(err) => return format!("Bash rejected command {:?}: {err}", input.command),
    }

    let backend = match workspace.get_backend() {
        Ok(backend) => backend,
        Err(err) => return format!("Bash could not access workspace backend: {err}"),
    };

    match backend
        .exec_shell(
            &["/bin/bash", "-lc", command],
            workspace.workdir(),
            Some(10.0),
        )
        .await
    {
        Ok(output) => format_exec_output(
            workspace.workdir(),
            output.exit_code,
            &output.stdout,
            &output.stderr,
        ),
        Err(err) => format!("Bash failed to execute {:?}: {err}", command),
    }
}

enum BashCommandPolicy {
    Allow,
    Confirm(String),
}

fn classify_bash_command(command: &str) -> Result<BashCommandPolicy, String> {
    if command.is_empty() {
        return Err("command must not be empty".to_string());
    }

    let lowered = command.to_ascii_lowercase();
    let hard_blocked_patterns = [
        "sleep",
        "tail -f",
        "yes",
        "while true",
        "..",
        "/etc",
        "/users",
    ];
    for pattern in hard_blocked_patterns {
        if command_contains_blocked_pattern(&lowered, pattern) {
            return Err(format!(
                "pattern {pattern:?} is blocked because it is unsafe for this interactive demo"
            ));
        }
    }

    let confirm_patterns = [
        "rm",
        "rmdir",
        "mv",
        "cp",
        "chmod",
        "chown",
        "sudo",
        "git add",
        "git commit",
        "git push",
        "git reset",
        "git checkout",
        "git switch",
        "curl",
        "wget",
        "ssh",
        "scp",
        "npm install",
        "pnpm install",
        "cargo install",
        ">",
        ">>",
        "<<",
        "tee",
        "/tmp",
    ];

    let reasons = confirm_patterns
        .into_iter()
        .filter(|pattern| command_contains_blocked_pattern(&lowered, pattern))
        .map(|pattern| pattern.to_string())
        .collect::<Vec<_>>();

    if reasons.is_empty() {
        Ok(BashCommandPolicy::Allow)
    } else {
        Ok(BashCommandPolicy::Confirm(format!(
            "contains potentially risky pattern(s): {}",
            reasons.join(", ")
        )))
    }
}

fn confirm_bash_command(cwd: &str, command: &str, reason: &str) -> Result<(), String> {
    eprintln!("[permission:confirm] Bash wants to execute a command in {cwd}");
    eprintln!("[permission:confirm] reason: {reason}");
    eprintln!("[permission:confirm] command: {command:?}");
    eprint!("Allow this Bash command? Type 'y' or 'yes' to continue: ");
    if let Err(err) = io::stderr().flush() {
        return Err(format!("Bash could not prompt for confirmation: {err}"));
    }

    let mut answer = String::new();
    if let Err(err) = io::stdin().read_line(&mut answer) {
        return Err(format!("Bash could not read confirmation: {err}"));
    }

    let answer = answer.trim().to_ascii_lowercase();
    if answer == "y" || answer == "yes" {
        Ok(())
    } else {
        Err(format!(
            "User declined Bash command {:?}; command was not executed.",
            command
        ))
    }
}

fn command_contains_blocked_pattern(command: &str, pattern: &str) -> bool {
    if pattern
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        command
            .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            .any(|token| token == pattern)
    } else {
        command.contains(pattern)
    }
}

fn format_exec_output(cwd: &str, exit_code: i32, stdout: &[u8], stderr: &[u8]) -> String {
    const MAX_STREAM_CHARS: usize = 8192;

    let (stdout_text, stdout_truncated) = decode_and_truncate(stdout, MAX_STREAM_CHARS);
    let (stderr_text, stderr_truncated) = decode_and_truncate(stderr, MAX_STREAM_CHARS);
    let mut result = format!("cwd: {cwd}\nexit_code: {exit_code}\n");
    result.push_str("stdout:\n");
    result.push_str(if stdout_text.is_empty() {
        "(empty)"
    } else {
        &stdout_text
    });
    if stdout_truncated {
        result.push_str("\n[stdout truncated]");
    }
    result.push_str("\nstderr:\n");
    result.push_str(if stderr_text.is_empty() {
        "(empty)"
    } else {
        &stderr_text
    });
    if stderr_truncated {
        result.push_str("\n[stderr truncated]");
    }
    result
}

fn decode_and_truncate(bytes: &[u8], max_chars: usize) -> (String, bool) {
    let text = String::from_utf8_lossy(bytes).to_string();
    if text.chars().count() <= max_chars {
        return (text, false);
    }
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.push('…');
    (truncated, true)
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct WorkspaceWriteFileInput {
    /// File path relative to the workspace root, for example "hello.txt" or "notes/hello.txt".
    path: String,
    /// UTF-8 text content to write.
    content: String,
}

async fn workspace_write_file(state: Arc<ToolState>, input: WorkspaceWriteFileInput) -> String {
    let Some(workspace) = &state.workspace else {
        return "Workspace is not available for this run.".to_string();
    };

    if !workspace.is_alive {
        return "Workspace is not alive, so workspace_write_file cannot write files.".to_string();
    }

    let target = match resolve_workspace_relative_path(&workspace.workdir, &input.path) {
        Ok(path) => path,
        Err(err) => return format!("workspace_write_file rejected path {:?}: {err}", input.path),
    };

    let relative_path = input.path.trim();
    let byte_len = input.content.len();
    eprintln!(
        "[permission:confirm] workspace_write_file wants to write {byte_len} byte(s) to {relative_path:?} inside {}",
        workspace.workdir
    );
    eprint!("Allow this write? Type 'y' or 'yes' to continue: ");
    if let Err(err) = io::stderr().flush() {
        return format!("workspace_write_file could not prompt for confirmation: {err}");
    }

    let mut answer = String::new();
    if let Err(err) = io::stdin().read_line(&mut answer) {
        return format!("workspace_write_file could not read confirmation: {err}");
    }

    let answer = answer.trim().to_ascii_lowercase();
    if answer != "y" && answer != "yes" {
        return format!(
            "User declined workspace_write_file; no file was written to {relative_path:?}."
        );
    }

    if let Some(parent) = target.parent()
        && let Err(err) = tokio::fs::create_dir_all(parent).await
    {
        return format!("workspace_write_file failed to create parent directory: {err}");
    }

    match tokio::fs::write(&target, input.content.as_bytes()).await {
        Ok(()) => format!(
            "workspace_write_file wrote {byte_len} byte(s) to {relative_path:?} inside the workspace."
        ),
        Err(err) => format!("workspace_write_file failed to write {relative_path:?}: {err}"),
    }
}

fn resolve_workspace_relative_path(root: &str, requested: &str) -> Result<PathBuf, String> {
    let trimmed = requested.trim();
    if trimmed.is_empty() {
        return Err("path must not be empty".to_string());
    }

    let relative = Path::new(trimmed);
    if relative.is_absolute() {
        return Err("absolute paths are not allowed".to_string());
    }

    let mut clean = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err("parent directory traversal is not allowed".to_string());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("path must stay inside the workspace".to_string());
            }
        }
    }

    if clean.as_os_str().is_empty() {
        return Err("path must include a file name".to_string());
    }

    Ok(Path::new(root).join(clean))
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct MemoryWriteInput {
    /// Slug-style memory name. Only letters, digits, underscores, and hyphens are valid.
    name: String,
    /// One-line summary shown in MEMORY.md.
    description: String,
    /// Memory type: user, feedback, project, or reference. Defaults to project.
    memory_type: Option<String>,
    /// Markdown memory body. Do not include secrets such as API keys, tokens, or passwords.
    content: String,
    /// Optional tags for the memory metadata.
    tags: Option<Vec<String>>,
}

async fn memory_write(state: Arc<ToolState>, input: MemoryWriteInput) -> String {
    let Some(memory) = &state.memory_store else {
        return "Memory is not available for this run.".to_string();
    };

    let memory_type = memory_type_from_optional(input.memory_type.as_deref(), MemoryType::Project);
    let type_label = memory_type.as_str().to_string();
    let mut entry = MemoryEntry::new(
        input.name.trim().to_string(),
        input.description.trim().to_string(),
        memory_type,
        input.content,
    );
    entry.metadata.tags = input.tags;

    match memory.write(entry).await {
        Ok(()) => format!(
            "memory_write saved memory {:?} as type {} with description {:?}.",
            input.name, type_label, input.description
        ),
        Err(err) => format!(
            "memory_write failed for {:?}: {err}. Memory names must match [A-Za-z0-9_-]+ and descriptions must not be empty.",
            input.name
        ),
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct MemorySearchInput {
    /// Keyword query to search in memory descriptions and content.
    query: String,
    /// Optional type filter: user, feedback, project, or reference.
    type_filter: Option<String>,
    /// Maximum results to return. Capped at 10.
    max_results: Option<usize>,
}

async fn memory_search(state: Arc<ToolState>, input: MemorySearchInput) -> String {
    let Some(memory) = &state.memory_store else {
        return "Memory is not available for this run.".to_string();
    };

    let query = input.query.trim();
    if query.is_empty() {
        return "memory_search requires a non-empty query.".to_string();
    }

    let type_filter = input
        .type_filter
        .as_deref()
        .map(|value| MemoryType::from(value.trim()));
    let limit = clamp_limit(input.max_results, 5, 10);
    match memory.search(query, type_filter).await {
        Ok(entries) if entries.is_empty() => {
            format!("memory_search found no memories for {query:?}.")
        }
        Ok(entries) => {
            let lines = entries
                .into_iter()
                .take(limit)
                .map(|entry| format_memory_entry_summary(&entry, 280))
                .collect::<Vec<_>>()
                .join("\n\n");
            format!("memory_search results for {query:?}:\n{lines}")
        }
        Err(err) => format!("memory_search failed for {query:?}: {err}"),
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct MemoryReadInput {
    /// Exact memory name without the .md extension.
    name: String,
}

async fn memory_read(state: Arc<ToolState>, input: MemoryReadInput) -> String {
    let Some(memory) = &state.memory_store else {
        return "Memory is not available for this run.".to_string();
    };

    let name = input.name.trim();
    if name.is_empty() {
        return "memory_read requires a non-empty memory name.".to_string();
    }

    match memory.read(name).await {
        Ok(Some(entry)) => format_memory_entry_summary(&entry, 1200),
        Ok(None) => format!("memory_read did not find a memory named {name:?}."),
        Err(err) => format!("memory_read failed for {name:?}: {err}"),
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct MemoryListInput {
    /// Optional type filter: user, feedback, project, or reference.
    type_filter: Option<String>,
    /// Maximum headers to return. Capped at 20.
    max_results: Option<usize>,
}

async fn memory_list(state: Arc<ToolState>, input: MemoryListInput) -> String {
    let Some(memory) = &state.memory_store else {
        return "Memory is not available for this run.".to_string();
    };

    let type_filter = input
        .type_filter
        .as_deref()
        .map(|value| MemoryType::from(value.trim()));
    let limit = clamp_limit(input.max_results, 10, 20);
    match memory.list().await {
        Ok(headers) => {
            let lines = headers
                .into_iter()
                .filter(|header| match (&type_filter, &header.mem_type) {
                    (Some(expected), Some(actual)) => expected == actual,
                    (Some(_), None) => false,
                    (None, _) => true,
                })
                .take(limit)
                .map(|header| {
                    let name = header.filename.trim_end_matches(".md");
                    let description = header.description.unwrap_or_else(|| "(no description)".to_string());
                    let memory_type = header
                        .mem_type
                        .as_ref()
                        .map(MemoryType::as_str)
                        .unwrap_or("unknown");
                    let mtime = header
                        .mtime
                        .map(|value| format!("{value:.3}"))
                        .unwrap_or_else(|| "unknown".to_string());
                    format!(
                        "- {name} ({memory_type})\n  description: {description}\n  path: {}\n  mtime: {mtime}",
                        header.path
                    )
                })
                .collect::<Vec<_>>();
            if lines.is_empty() {
                "memory_list found no matching memory entries.".to_string()
            } else {
                format!("memory_list entries:\n{}", lines.join("\n"))
            }
        }
        Err(err) => format!("memory_list failed: {err}"),
    }
}

fn memory_type_from_optional(value: Option<&str>, default: MemoryType) -> MemoryType {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(MemoryType::from)
        .unwrap_or(default)
}

fn clamp_limit(value: Option<usize>, default: usize, max: usize) -> usize {
    value.unwrap_or(default).clamp(1, max)
}

fn format_memory_entry_summary(entry: &MemoryEntry, content_limit: usize) -> String {
    format!(
        "- {} ({})\n  description: {}\n  content: {}",
        entry.name,
        entry.metadata.mem_type.as_str(),
        entry.description,
        preview_text(&entry.content, content_limit)
    )
}

fn preview_text(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut preview = normalized.chars().take(max_chars).collect::<String>();
    preview.push('…');
    preview
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalculatorInput {
    /// Arithmetic expression, for example: 23 * (17 + 5)
    expression: String,
}

async fn calculator(input: CalculatorInput) -> String {
    match eval_expression(&input.expression) {
        Ok(value) => format!("{} = {}", input.expression, trim_float(value)),
        Err(err) => format!("Calculator error for {:?}: {}", input.expression, err),
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct TimeInput {
    /// Preferred timezone label: "utc", "local", or "both".
    timezone: Option<String>,
}

async fn safe_time(input: TimeInput) -> String {
    let now = SystemTime::now();
    let unix_seconds = now
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    match input
        .timezone
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("utc") => {
            format!("Current Unix time: {unix_seconds} seconds since 1970-01-01T00:00:00Z")
        }
        Some("local") => format!("Current system time: {now:?}"),
        _ => format!(
            "Current Unix time: {unix_seconds} seconds since 1970-01-01T00:00:00Z\nCurrent system time: {now:?}"
        ),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn eval_expression(expression: &str) -> Result<f64, String> {
    let tokens = tokenize(expression)?;
    if tokens.is_empty() {
        return Err("empty expression".to_string());
    }
    let mut parser = Parser { tokens, pos: 0 };
    let value = parser.parse_expression()?;
    if parser.pos != parser.tokens.len() {
        return Err("unexpected trailing tokens".to_string());
    }
    if value.is_finite() {
        Ok(value)
    } else {
        Err("result is not finite".to_string())
    }
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        match chars[index] {
            ch if ch.is_whitespace() => index += 1,
            '+' => {
                tokens.push(Token::Plus);
                index += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                index += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                index += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                index += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                index += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                index += 1;
            }
            ch if ch.is_ascii_digit() || ch == '.' => {
                let start = index;
                index += 1;
                while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.')
                {
                    index += 1;
                }
                let number: String = chars[start..index].iter().collect();
                let value = number
                    .parse::<f64>()
                    .map_err(|_| format!("invalid number: {number}"))?;
                tokens.push(Token::Number(value));
            }
            ch => return Err(format!("unsupported character: {ch:?}")),
        }
    }

    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn parse_expression(&mut self) -> Result<f64, String> {
        let mut value = self.parse_term()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.pos += 1;
                    value += self.parse_term()?;
                }
                Some(Token::Minus) => {
                    self.pos += 1;
                    value -= self.parse_term()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn parse_term(&mut self) -> Result<f64, String> {
        let mut value = self.parse_factor()?;
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.pos += 1;
                    value *= self.parse_factor()?;
                }
                Some(Token::Slash) => {
                    self.pos += 1;
                    let divisor = self.parse_factor()?;
                    if divisor == 0.0 {
                        return Err("division by zero".to_string());
                    }
                    value /= divisor;
                }
                _ => return Ok(value),
            }
        }
    }

    fn parse_factor(&mut self) -> Result<f64, String> {
        match self.peek().cloned() {
            Some(Token::Number(value)) => {
                self.pos += 1;
                Ok(value)
            }
            Some(Token::Minus) => {
                self.pos += 1;
                Ok(-self.parse_factor()?)
            }
            Some(Token::LParen) => {
                self.pos += 1;
                let value = self.parse_expression()?;
                match self.peek() {
                    Some(Token::RParen) => {
                        self.pos += 1;
                        Ok(value)
                    }
                    _ => Err("missing closing ')'".to_string()),
                }
            }
            Some(token) => Err(format!("unexpected token: {token:?}")),
            None => Err("unexpected end of expression".to_string()),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }
}

fn trim_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        let mut text = format!("{value:.12}");
        while text.contains('.') && text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::{preview_text, resolve_workspace_relative_path};
    use std::path::Path;

    #[test]
    fn resolves_simple_workspace_paths() {
        let root = "/tmp/agent-demo-workspace";
        assert_eq!(
            resolve_workspace_relative_path(root, "hello.txt").unwrap(),
            Path::new(root).join("hello.txt")
        );
        assert_eq!(
            resolve_workspace_relative_path(root, "notes/hello.txt").unwrap(),
            Path::new(root).join("notes").join("hello.txt")
        );
    }

    #[test]
    fn rejects_paths_that_escape_workspace() {
        let root = "/tmp/agent-demo-workspace";
        for path in ["", "   ", "/tmp/x", "../x", "a/../../x"] {
            assert!(
                resolve_workspace_relative_path(root, path).is_err(),
                "path should be rejected: {path:?}"
            );
        }
    }

    #[test]
    fn preview_text_truncates_by_char_count() {
        assert_eq!(preview_text("a   b\n c", 10), "a b c");
        assert_eq!(preview_text("abcdef", 3), "abc…");
    }
}
