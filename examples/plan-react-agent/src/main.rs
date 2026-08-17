//! Plan mode example — built-in task planning tools on a `ReActAgent`.
//!
//! Demonstrates:
//! 1. Task tools (`TaskCreate` / `TaskList` / `TaskGet` / `TaskUpdate`) are
//!    auto-registered at construction time (`task_tools_enabled` defaults to
//!    `true`), verifiable without any model call.
//! 2. A multi-step request where the model drives the full task lifecycle via
//!    tool calls: `TaskCreate` → `TaskList` → `TaskUpdate(status=in_progress)`
//!    → `TaskUpdate(status=completed)`.
//! 3. The task list persists in `AgentState::tasks_context` after the reply
//!    and can be read back programmatically.
//!
//! Run:
//! ```bash
//! cargo run -p plan-react-agent -- --prompt "..."
//! ```
//!
//! Requires `DASHSCOPE_API_KEY` for real model calls.

use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_dashscope::DashScopeChatModel;
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use agent_scope_state::TaskState;
use clap::Parser;
use futures::StreamExt;

#[derive(Parser)]
struct Cli {
    /// Prompt sent to the agent.
    #[arg(
        short,
        long,
        default_value = "请规划并执行：1) 阅读本仓库根目录的 README.md；2) 列出其中提到的三个 crate；3) 汇总成一段话。"
    )]
    prompt: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    dotenv::dotenv().ok();

    let api_key = std::env::var("DASHSCOPE_API_KEY").map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("缺少环境变量 DASHSCOPE_API_KEY。请设置后重试（{e}）。"),
        )
    })?;

    let model = Arc::new(DashScopeChatModel::new(&api_key, "qwen-plus").with_stream(true));

    let config = AgentConfig::builder()
        .name("assistant")
        .system_prompt(
            "你是一个任务规划助手。面对 3 步以上的多步工作时，\
             先用 TaskCreate 拆分任务，开始执行前用 TaskUpdate 标记 in_progress，\
             完成后再用 TaskUpdate 标记 completed，并调用 TaskList 检查进度。",
        )
        .model(model)
        .build()?;

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?;

    // 1) 任务工具默认自动注册（无需模型调用即可验证）。
    let toolkit = agent.toolkit().expect("toolkit is configured");
    for name in ["TaskCreate", "TaskList", "TaskGet", "TaskUpdate"] {
        println!("[tool] {name} registered: {}", toolkit.contains(name));
    }

    // 2) 让模型用任务工具规划并执行多步工作。
    let msg = user_msg("user", &cli.prompt)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{e:?}")))?;
    println!("\n--- reply_stream ---");
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;
    while let Some(event) = stream.next().await {
        match &event {
            AgentEvent::ToolCallStart(s) => {
                println!("\n[tool call] {} ({})", s.tool_call_name, s.tool_call_id)
            }
            AgentEvent::ToolCallEnd(e) => {
                if let Some(input) = &e.input {
                    println!("  input: {input}");
                }
                // Feature 033: blank line so each tool call's input↔result pair
                // reads as a unit (tool results now carry a trailing newline).
                println!();
            }
            AgentEvent::ToolResultTextDelta(d) => print!("{}", d.delta),
            AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
            AgentEvent::ReplyEnd(e) => println!("\n[reply end] {:?}", e.finished_reason),
            _ => {}
        }
    }

    // 3) 回复结束后任务清单随 AgentState 持久化，可直接读取。
    println!("\n--- tasks_context after reply ---");
    let state = agent.try_state();
    if state.tasks_context.tasks.is_empty() {
        println!("(no tasks)");
    }
    for task in &state.tasks_context.tasks {
        let state_str = match task.state {
            TaskState::Pending => "pending",
            TaskState::InProgress => "in_progress",
            TaskState::Completed => "completed",
        };
        println!("  #{} [{}] {}", task.id, state_str, task.subject);
    }

    Ok(())
}
