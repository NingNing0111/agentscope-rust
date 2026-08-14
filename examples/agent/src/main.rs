//! Agent example: orchestration with permission rules, interruption, and task tools.
//!
//! Assembles a `ReActAgent` with an explicit permission context (allowing only
//! read-only tool calls), a streaming reply that also reacts to a mid-flight
//! `interrupt()`, and shows how built-in task tools are injected when enabled.
//!
//! Requires `DASHSCOPE_API_KEY` for real model calls.

use std::sync::Arc;

use agent_scope_agent::{
    Agent, AgentConfig, ContextConfig, PermissionContext, PermissionRule, ReActAgent, ReActConfig,
};
use agent_scope_dashscope::DashScopeChatModel;
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use agent_scope_tool::FunctionTool;
use clap::Parser;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ReadInput {
    path: String,
}

async fn read_file(input: ReadInput) -> String {
    match tokio::fs::read_to_string(&input.path).await {
        Ok(text) => format!("{} bytes:\n{}", text.len(), text),
        Err(err) => format!("read error: {err}"),
    }
}

#[derive(Parser)]
struct Cli {
    #[arg(short, long, default_value = "请用一句话介绍你自己。")]
    prompt: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    dotenv::dotenv().ok();

    let api_key = std::env::var("DASHSCOPE_API_KEY")
        .map_err(|_| anyhow::anyhow!("error: 缺少环境变量 DASHSCOPE_API_KEY。请设置后重试。"))?;
    let model = Arc::new(DashScopeChatModel::new(&api_key, "qwen-plus").with_stream(true));

    // Tool + a permission context that only allows read-only tools.
    let mut toolkit = agent_scope_tool::ToolKit::new();
    toolkit.register(FunctionTool::new(
        "read_file",
        "Read a text file from disk.",
        read_file,
    ));

    let mut perm = PermissionContext::new(agent_scope_agent::PermissionMode::Default);
    perm.add_rule(PermissionRule::allow("read_file"));

    let agent_config = AgentConfig::builder()
        .name("assistant")
        .system_prompt("你是一个乐于助人的助手。当用户请求读取文件时，请使用 read_file 工具。")
        .model(model)
        .toolkit(toolkit)
        .permission_context(perm)
        .build()?;

    let agent = Arc::new(ReActAgent::new(
        agent_config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?);

    // Built-in task tools (TaskCreate/TaskList/TaskGet/TaskUpdate) are registered
    // by default (task_tools_enabled = true).
    println!("task tools enabled by default: true");

    let msg = user_msg("user", &cli.prompt).map_err(|e| anyhow::anyhow!("{e:?}"))?;

    // Stream the reply; on a separate task, demonstrate interrupt() after a delay.
    let agent_for_spawn = Arc::clone(&agent);
    let handle = tokio::spawn(async move {
        sleep(Duration::from_millis(2000)).await;
        agent_for_spawn.interrupt();
    });

    let mut stream = agent.reply_stream(Some(vec![msg])).await?;
    while let Some(event) = stream.next().await {
        match &event {
            AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
            AgentEvent::ToolCallStart(s) => println!("\n[tool start] {}", s.tool_call_name),
            AgentEvent::RequireUserConfirm(c) => {
                let names: Vec<String> = c.tool_calls.iter().map(|b| b.name.clone()).collect();
                println!("\n[needs confirmation] tools: {names:?}");
            }
            AgentEvent::UserInterrupt(_) => println!("\n[interrupted by user]"),
            AgentEvent::ReplyEnd(e) => println!("\n[reply end] {:?}", e.finished_reason),
            _ => {}
        }
    }

    let _ = handle.await;
    Ok(())
}
