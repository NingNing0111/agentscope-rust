use std::collections::HashSet;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use agent_scope_agent::Agent;
use agent_scope_message::factory::user_msg;
use futures::StreamExt;

use crate::agent::AgentRuntime;
use crate::error::{PiError, PiResult};
use crate::render::{ConfirmationCandidate, RenderConfig, RenderedTurn, render_event};

/// Upper bound on automatic retries after the user approves a denied operation.
/// A higher cap is not useful: if the agent keeps producing *new* destructive
/// commands, the loop would retry forever; if it replays an approved one, the
/// tool now executes it.
const MAX_CONFIRMATION_RETRIES: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalCommand {
    Empty,
    Help,
    Model,
    Tools,
    Skills,
    Skill(String),
    Sessions,
    Save,
    Tasks,
    Approvals,
    Context,
    Events(bool),
    Json(bool),
    Exit,
    Unknown(String),
}

/// Outcome of handling a `/` command: human-readable messages to surface to the
/// user plus whether the frontend should exit. The line REPL prints the
/// messages; the TUI appends them to the message stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandOutput {
    pub messages: Vec<String>,
    pub should_exit: bool,
}

pub fn parse_repl_command(input: &str) -> LocalCommand {
    match input.trim() {
        "" => LocalCommand::Empty,
        "/help" => LocalCommand::Help,
        "/model" => LocalCommand::Model,
        "/tools" => LocalCommand::Tools,
        "/skills" => LocalCommand::Skills,
        other if other.starts_with("/skill ") => {
            let name = other.trim_start_matches("/skill ").trim();
            if name.is_empty() {
                LocalCommand::Unknown(other.to_string())
            } else {
                LocalCommand::Skill(name.to_string())
            }
        }
        "/sessions" => LocalCommand::Sessions,
        "/save" => LocalCommand::Save,
        "/tasks" => LocalCommand::Tasks,
        "/approvals" => LocalCommand::Approvals,
        "/context" => LocalCommand::Context,
        "/events on" => LocalCommand::Events(true),
        "/events off" => LocalCommand::Events(false),
        "/json on" => LocalCommand::Json(true),
        "/json off" => LocalCommand::Json(false),
        "/exit" | "/quit" => LocalCommand::Exit,
        other if other.starts_with('/') => LocalCommand::Unknown(other.to_string()),
        _ => LocalCommand::Unknown(input.trim().to_string()),
    }
}

pub async fn run_one_shot(mut runtime: AgentRuntime, prompt: String) -> PiResult<()> {
    let ask = line_ask();
    let turn = run_turn_with_confirmations(&runtime, &prompt, ask).await?;
    let tasks = task_context_json(&runtime);
    runtime
        .session
        .add_turn(prompt, turn.events, turn.text, None);
    runtime.session.snapshot_tasks(tasks);
    runtime.store.save(&runtime.session)
}

/// Adapt the blocking y/n prompt into the async ask closure the confirmation
/// loop now expects. Used by the line REPL and the one-shot mode; the TUI
/// drives confirmation through its own event loop instead.
fn line_ask() -> impl FnMut(
    &[ConfirmationCandidate],
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<bool>> + Send>> {
    |candidates: &[ConfirmationCandidate]| {
        let candidates: Vec<ConfirmationCandidate> = candidates.to_vec();
        Box::pin(async move { ask_user_confirmation(&candidates) })
    }
}

pub async fn run_interactive(mut runtime: AgentRuntime) -> PiResult<()> {
    println!(
        "pi-rust ready · provider={} · model={} · mode={} · key={} · cwd={} · skills={}",
        runtime.config.provider.name(),
        runtime.config.model,
        runtime.config.mode.as_str(),
        runtime.config.masked_api_key,
        runtime.config.cwd.display(),
        runtime.skills.len()
    );
    println!("Type /help for commands, /exit to save and quit.");

    let stdin = io::stdin();
    loop {
        print!("pi> ");
        io::stdout()
            .flush()
            .map_err(|err| PiError::io("flush prompt", err))?;
        let mut input = String::new();
        let read = stdin
            .read_line(&mut input)
            .map_err(|err| PiError::io("read stdin", err))?;
        if read == 0 {
            runtime.store.save(&runtime.session)?;
            return Ok(());
        }
        let input = input.trim_end().to_string();
        if input.trim().is_empty() {
            continue;
        }
        if input.starts_with('/') {
            let output = handle_command(&mut runtime, &input)?;
            for message in &output.messages {
                println!("{message}");
            }
            if output.should_exit {
                return Ok(());
            }
            continue;
        }
        let ask = line_ask();
        let turn = run_turn_with_confirmations(&runtime, &input, ask).await?;
        let tasks = task_context_json(&runtime);
        runtime
            .session
            .add_turn(input, turn.events, turn.text, None);
        runtime.session.snapshot_tasks(tasks);
        runtime.store.save(&runtime.session)?;
    }
}

async fn run_turn(runtime: &AgentRuntime, input: &str) -> PiResult<RenderedTurn> {
    let msg = user_msg("user", input)?;
    let mut stream = runtime.agent.reply_stream(Some(vec![msg])).await?;
    let config = RenderConfig {
        cwd: runtime.config.cwd.clone(),
        show_events: runtime.config.show_events,
        show_json_events: runtime.config.show_json_events,
    };
    let mut turn = RenderedTurn::default();
    loop {
        tokio::select! {
            event = stream.next() => {
                match event {
                    Some(event) => {
                        for chunk in render_event(event, &config, &mut turn)? {
                            print!("{chunk}");
                        }
                        let _ = io::stdout().flush();
                    }
                    None => break,
                }
            }
            _ = tokio::signal::ctrl_c() => {
                if !turn.interrupted {
                    println!("\n[interrupting…]");
                    // Interrupts the in-flight model call / tool loop; the agent
                    // is reusable for the next turn.
                    runtime.agent.interrupt();
                }
            }
        }
    }
    Ok(turn)
}

/// Production entry point: run a turn and loop through the confirmation
/// workflow until no operation is left awaiting approval (or the user rejects,
/// the retry cap is hit, or the turn was interrupted).
pub async fn run_turn_with_confirmations<A, AFut>(
    runtime: &AgentRuntime,
    input: &str,
    mut ask: A,
) -> PiResult<RenderedTurn>
where
    A: FnMut(&[ConfirmationCandidate]) -> AFut,
    AFut: std::future::Future<Output = Vec<bool>>,
{
    let approvals = Arc::clone(&runtime.approvals);
    let first = run_turn(runtime, input).await?;
    // A failure in a *retry* turn is tolerated: return whatever was accumulated
    // so far instead of throwing away the whole turn, but surface the error so
    // the user knows the retry did not succeed silently.  The first turn's error
    // is still propagated via `?` above.
    let result = run_confirmation_loop(
        &approvals,
        first,
        || async {
            match run_turn(runtime, input).await {
                Ok(turn) => turn,
                Err(err) => {
                    eprintln!("warning: retry turn failed: {}", err.safe_message());
                    RenderedTurn::default()
                }
            }
        },
        &mut ask,
    )
    .await;
    Ok(result)
}

/// Drive the confirmation loop. Pure over injected closures so it is unit-testable.
///
/// Each iteration offers the turn's un-denied confirmation candidates to the
/// host; every approval is recorded into the shared `approvals` set and the
/// turn is re-run (same input) so the tool now executes the approved operation.
pub async fn run_confirmation_loop<F, Fut, A, AFut>(
    approvals: &Arc<Mutex<HashSet<String>>>,
    mut attempt: RenderedTurn,
    mut run_attempt: F,
    mut ask: A,
) -> RenderedTurn
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = RenderedTurn>,
    A: FnMut(&[ConfirmationCandidate]) -> AFut,
    AFut: std::future::Future<Output = Vec<bool>>,
{
    let mut aggregate = RenderedTurn::default();
    merge_turn(&mut aggregate, &attempt);
    let mut denied: HashSet<String> = HashSet::new();
    let mut retries = 0usize;

    loop {
        // Skip candidates the user already denied this turn; never ask after an
        // interrupt.
        let pending: Vec<ConfirmationCandidate> = attempt
            .confirmation_candidates
            .iter()
            .filter(|candidate| !denied.contains(&candidate.fingerprint))
            .cloned()
            .collect();
        if pending.is_empty() || attempt.interrupted {
            break;
        }
        if retries >= MAX_CONFIRMATION_RETRIES {
            break;
        }

        let decisions = ask(&pending).await;
        let mut granted_any = false;
        for (candidate, approved) in pending.iter().zip(decisions) {
            if approved {
                approvals
                    .lock()
                    .unwrap()
                    .insert(candidate.fingerprint.clone());
                granted_any = true;
                println!("approved: {}", candidate.description);
            } else {
                denied.insert(candidate.fingerprint.clone());
                println!("denied:   {}", candidate.description);
            }
        }
        if !granted_any {
            break;
        }

        retries += 1;
        attempt = run_attempt().await;
        merge_turn(&mut aggregate, &attempt);

        // Defense-in-depth: if a fresh turn still reports confirmation_required
        // for an operation whose fingerprint is already approved, the
        // fingerprint no longer matches (e.g. the agent re-issued a different
        // command) and retrying cannot converge — stop with a warning.
        if !attempt.confirmation_candidates.is_empty()
            && attempt.confirmation_candidates.iter().all(|candidate| {
                approvals
                    .lock()
                    .map(|guard| guard.contains(&candidate.fingerprint))
                    .unwrap_or(false)
            })
        {
            eprintln!(
                "warning: approved operation still reports confirmation_required; stopping retry"
            );
            break;
        }
    }
    aggregate
}

/// Combine multiple loop attempts: append events/lines, keep the last reply text.
fn merge_turn(aggregate: &mut RenderedTurn, attempt: &RenderedTurn) {
    aggregate.events.extend(attempt.events.clone());
    aggregate.tool_lines.extend(attempt.tool_lines.clone());
    aggregate
        .tool_call_names
        .extend(attempt.tool_call_names.clone());
    aggregate
        .tool_call_inputs
        .extend(attempt.tool_call_inputs.clone());
    aggregate.tool_outputs.extend(attempt.tool_outputs.clone());
    aggregate.text = attempt.text.clone();
    aggregate.confirmation_candidates = attempt.confirmation_candidates.clone();
    aggregate.interrupted |= attempt.interrupted;
}

/// Parse a single y/n line into an approval decision.
pub fn parse_confirmation_response(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Ask the host whether to approve each pending operation.
///
/// Returns one decision per candidate, in order. EOF (piped input / CI) is
/// treated as a denial so the loop never blocks.
pub fn ask_user_confirmation(candidates: &[ConfirmationCandidate]) -> Vec<bool> {
    let stdin = io::stdin();
    candidates
        .iter()
        .map(|candidate| {
            print!("Approve {}? [y/N] ", candidate.description);
            let _ = io::stdout().flush();
            let mut line = String::new();
            if stdin.read_line(&mut line).unwrap_or(0) == 0 {
                return false;
            }
            parse_confirmation_response(&line)
        })
        .collect()
}

/// Serialize the agent's current task list for the session snapshot.
fn task_context_json(runtime: &AgentRuntime) -> serde_json::Value {
    serde_json::to_value(runtime.agent.try_state().tasks_context.clone()).unwrap_or_default()
}

pub fn handle_command(runtime: &mut AgentRuntime, input: &str) -> PiResult<CommandOutput> {
    let mut out = CommandOutput::default();
    match parse_repl_command(input) {
        LocalCommand::Empty => {}
        LocalCommand::Help => out.messages.push(help_text(runtime)),
        LocalCommand::Model => out.messages.push(format!(
            "provider={} model={} api_key={}",
            runtime.config.provider.name(),
            runtime.config.model,
            runtime.config.masked_api_key
        )),
        LocalCommand::Tools => {
            if runtime.config.no_tools {
                out.messages.push("tools disabled".to_string());
            } else {
                let mut tools = vec![
                    "Read",
                    "Write",
                    "Edit",
                    "Bash",
                    "Grep",
                    "Glob",
                    "ListDir",
                    "Memory",
                    "TaskCreate",
                    "TaskList",
                    "TaskGet",
                    "TaskUpdate",
                ];
                if !runtime.skills.is_empty() {
                    tools.push("Skill");
                }
                out.messages.push(format!(
                    "tools: {} (risky overwrites and destructive commands require host-side confirmation)",
                    tools.join(", ")
                ));
            }
        }
        LocalCommand::Skills => out.messages.push(skills_text(runtime)),
        LocalCommand::Skill(name) => out.messages.push(skill_text(runtime, &name)),
        LocalCommand::Sessions => {
            for summary in runtime.store.list()? {
                out.messages.push(format!(
                    "{}  {}  {}",
                    summary.id, summary.updated_at, summary.summary
                ));
            }
        }
        LocalCommand::Tasks => {
            let state = runtime.agent.try_state();
            if state.tasks_context.tasks.is_empty() {
                out.messages.push("no tasks".to_string());
            } else {
                for task in &state.tasks_context.tasks {
                    let state_str = serde_json::to_value(task.state)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_string))
                        .unwrap_or_else(|| "?".to_string());
                    out.messages
                        .push(format!("{}  [{}]  {}", task.id, state_str, task.subject));
                }
            }
        }
        LocalCommand::Approvals => {
            let approvals = runtime.approvals.lock().unwrap_or_else(|e| e.into_inner());
            if approvals.is_empty() {
                out.messages.push("no approved operations".to_string());
            } else {
                for fingerprint in approvals.iter() {
                    out.messages.push(fingerprint.clone());
                }
            }
        }
        LocalCommand::Context => {
            let state = runtime.agent.try_state();
            out.messages
                .push(format!("context messages: {}", state.context.len()));
        }
        LocalCommand::Save => {
            runtime.store.save(&runtime.session)?;
            out.messages
                .push(format!("saved session {}", runtime.session.id));
        }
        LocalCommand::Events(enabled) => {
            runtime.config.show_events = enabled;
            out.messages.push(format!(
                "events {}",
                if enabled { "enabled" } else { "disabled" }
            ));
        }
        LocalCommand::Json(enabled) => {
            runtime.config.show_json_events = enabled;
            out.messages.push(format!(
                "JSON events {}",
                if enabled { "enabled" } else { "disabled" }
            ));
        }
        LocalCommand::Exit => {
            runtime.store.save(&runtime.session)?;
            out.messages
                .push(format!("saved session {}", runtime.session.id));
            out.should_exit = true;
        }
        LocalCommand::Unknown(other) => {
            out.messages.push(format!(
                "unknown command: {other}. Type /help for available commands."
            ));
        }
    }
    Ok(out)
}

fn skills_text(runtime: &AgentRuntime) -> String {
    if runtime.skills.is_empty() {
        return "skills: none loaded; use --skill-path <DIR> to load a directory containing SKILL.md"
            .to_string();
    }
    let mut text = String::from("loaded skills:");
    for skill in &runtime.skills {
        text.push_str(&format!("\n  - {}: {}", skill.name, skill.description));
    }
    text
}

fn skill_text(runtime: &AgentRuntime, name: &str) -> String {
    if let Some(skill) = runtime.skills.iter().find(|skill| skill.name == name) {
        skill.markdown.clone()
    } else {
        format!("skill not found: {name}. Type /skills for loaded skills.")
    }
}

fn help_text(runtime: &AgentRuntime) -> String {
    format!(
        r#"pi-rust commands:
  /help       Show this help, active config, and examples
  /model      Show provider/model without secrets
  /tools      Show registered tools and permission behavior
  /skills     List loaded workspace skills
  /skill NAME Show a loaded skill's full instructions
  /sessions   List persisted sessions
  /save       Save current session
  /tasks      Show the agent's task plan/progress/completion state
  /approvals  List host-approved destructive operations this session
  /context    Show the agent's context message count
  /events on|off  Toggle human-readable lifecycle/tool events
  /json on|off    Toggle redacted JSON event lines
  /exit, /quit    Save and exit

Confirmations:
  Risky overwrites and destructive shell commands are gated: the agent is
  denied, then you are asked to approve y/n. Approved operations are retried
  automatically (up to {max_retries} times) and listed via /approvals.

Active config:
  provider: {provider}
  model: {model}
  mode: {mode}
  cwd: {cwd}
  workdir: {workdir}
  tools: {tools}
  skills: {skills}
  memory: {memory}
  rag: {rag}

Sample prompts:
  请读取 src/main.rs 并说明它的主要功能。
  用 Grep 搜索项目里所有调用 println! 的地方。
  创建 hello.txt，内容是 Hello, World!
  把 hello.txt 中的 World 改成 Rust。
  执行 pwd，并告诉我返回了什么。
  请按 coding workflow 修改并验证这个项目。"#,
        provider = runtime.config.provider.name(),
        model = runtime.config.model,
        mode = runtime.config.mode.as_str(),
        cwd = runtime.config.cwd.display(),
        workdir = runtime.config.workdir.display(),
        tools = !runtime.config.no_tools,
        skills = runtime.skills.len(),
        memory = !runtime.config.no_memory,
        rag = !runtime.config.no_rag,
        max_retries = MAX_CONFIRMATION_RETRIES,
    )
}
