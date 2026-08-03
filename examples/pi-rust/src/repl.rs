use std::io::{self, Write};

use agent_scope_agent::Agent;
use agent_scope_message::factory::user_msg;
use futures::StreamExt;

use crate::agent::AgentRuntime;
use crate::error::{PiError, PiResult};
use crate::render::{RenderConfig, RenderedTurn, render_event};

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
    Events(bool),
    Json(bool),
    Exit,
    Unknown(String),
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
    let turn = run_turn(&runtime, &prompt).await?;
    runtime
        .session
        .add_turn(prompt, turn.events, turn.text, None);
    runtime.store.save(&runtime.session)
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
            if handle_command(&mut runtime, &input)? {
                return Ok(());
            }
            continue;
        }
        let turn = run_turn(&runtime, &input).await?;
        runtime
            .session
            .add_turn(input, turn.events, turn.text, None);
        runtime.store.save(&runtime.session)?;
    }
}

async fn run_turn(runtime: &AgentRuntime, input: &str) -> PiResult<RenderedTurn> {
    let msg = user_msg("user", input)?;
    let mut stream = runtime.agent.reply_stream(Some(vec![msg])).await?;
    let config = RenderConfig {
        show_events: runtime.config.show_events,
        show_json_events: runtime.config.show_json_events,
    };
    let mut turn = RenderedTurn::default();
    while let Some(event) = stream.next().await {
        render_event(event, &config, &mut turn)?;
    }
    Ok(turn)
}

fn handle_command(runtime: &mut AgentRuntime, input: &str) -> PiResult<bool> {
    match parse_repl_command(input) {
        LocalCommand::Empty => {}
        LocalCommand::Help => print_help(runtime),
        LocalCommand::Model => println!(
            "provider={} model={} api_key={}",
            runtime.config.provider.name(),
            runtime.config.model,
            runtime.config.masked_api_key
        ),
        LocalCommand::Tools => {
            if runtime.config.no_tools {
                println!("tools disabled");
            } else {
                let mut tools = vec!["Read", "Write", "Edit", "Bash"];
                if !runtime.skills.is_empty() {
                    tools.push("Skill");
                }
                println!(
                    "tools: {} (risky overwrites and destructive commands require host-side confirmation)",
                    tools.join(", ")
                );
            }
        }
        LocalCommand::Skills => print_skills(runtime),
        LocalCommand::Skill(name) => print_skill(runtime, &name),
        LocalCommand::Sessions => {
            for summary in runtime.store.list()? {
                println!(
                    "{}  {}  {}",
                    summary.id, summary.updated_at, summary.summary
                );
            }
        }
        LocalCommand::Save => {
            runtime.store.save(&runtime.session)?;
            println!("saved session {}", runtime.session.id);
        }
        LocalCommand::Events(enabled) => {
            runtime.config.show_events = enabled;
            println!("events {}", if enabled { "enabled" } else { "disabled" });
        }
        LocalCommand::Json(enabled) => {
            runtime.config.show_json_events = enabled;
            println!(
                "JSON events {}",
                if enabled { "enabled" } else { "disabled" }
            );
        }
        LocalCommand::Exit => {
            runtime.store.save(&runtime.session)?;
            println!("saved session {}", runtime.session.id);
            return Ok(true);
        }
        LocalCommand::Unknown(other) => {
            println!("unknown command: {other}. Type /help for available commands.")
        }
    }
    Ok(false)
}

fn print_skills(runtime: &AgentRuntime) {
    if runtime.skills.is_empty() {
        println!(
            "skills: none loaded; use --skill-path <DIR> to load a directory containing SKILL.md"
        );
        return;
    }
    println!("loaded skills:");
    for skill in &runtime.skills {
        println!("  - {}: {}", skill.name, skill.description);
    }
}

fn print_skill(runtime: &AgentRuntime, name: &str) {
    if let Some(skill) = runtime.skills.iter().find(|skill| skill.name == name) {
        println!("{}", skill.markdown);
    } else {
        println!("skill not found: {name}. Type /skills for loaded skills.");
    }
}

fn print_help(runtime: &AgentRuntime) {
    println!(
        r#"pi-rust commands:
  /help       Show this help, active config, and examples
  /model      Show provider/model without secrets
  /tools      Show registered tools and permission behavior
  /skills     List loaded workspace skills
  /skill NAME Show a loaded skill's full instructions
  /sessions   List persisted sessions
  /save       Save current session
  /events on|off  Toggle human-readable lifecycle/tool events
  /json on|off    Toggle redacted JSON event lines
  /exit, /quit    Save and exit

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
    );
}
