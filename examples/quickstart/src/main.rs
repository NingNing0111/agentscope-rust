//! Quickstart example: the minimal AgentScope Rust agent.
//!
//! Shows the four steps to run a conversational agent with an OpenAI chat
//! model: credential, model, toolkit, and a ReActAgent. Both entry
//! points of the [`Agent`] trait are demonstrated — `reply` returns the final
//! message, while `reply_stream` yields incremental events.

use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use agent_scope_rig::RigChatModel;
use agent_scope_tool::ToolKit;
use clap::Parser;
use futures::StreamExt;

/// Minimal AgentScope Rust agent.
#[derive(Parser)]
struct Cli {
    /// User prompt to send to the agent.
    #[arg(short, long, default_value = "你好，请用一句话介绍你自己。")]
    prompt: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load a .env file from the repo root, if present (DEFAULT_API_KEY=...).
    dotenv::dotenv().ok();

    // 1. Credential — read from the environment (or a .env file in the repo root).
    let api_key = std::env::var("DEFAULT_API_KEY").map_err(|_| {
        anyhow::anyhow!(
            "error: 缺少环境变量 DEFAULT_API_KEY。\n\
             运行前请设置：\n    export DEFAULT_API_KEY=sk-your-key\n\
             或将 DEFAULT_API_KEY=sk-your-key 写入仓库根目录 .env 文件（示例会通过 dotenv 自动加载）。"
        )
    })?;

    // 2. Chat model — OpenAI via a rig-backed provider.
    //    模型名从 DEFAULT_CHAT_MODEL 读取（fallback qwen3.7-plus）；
    //    设了 DEFAULT_URL 则覆盖端点（可指向 DashScope 兼容端点等）。
    let model_name =
        std::env::var("DEFAULT_CHAT_MODEL").unwrap_or_else(|_| "qwen3.7-plus".to_string());
    let mut model = RigChatModel::openai(&api_key, &model_name)?;
    if let Ok(base_url) = std::env::var("DEFAULT_URL") {
        model = model.with_base_url(base_url);
    }
    let model = Arc::new(model.with_stream(true));

    // 3. Toolkit — empty here; add tools (e.g. Bash/Read/Write) to enable tool calls.
    let toolkit = ToolKit::new();

    // 4. Assemble the ReActAgent.
    let agent_config = AgentConfig::builder()
        .name("assistant")
        .system_prompt("你是一个乐于助人的助手。")
        .model(model)
        .toolkit(toolkit)
        .build()?;

    let agent = ReActAgent::new(
        agent_config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?;

    let msg =
        user_msg("user", &cli.prompt).map_err(|e| anyhow::anyhow!("无效的 user 消息: {e:?}"))?;

    // Option 1: await the final assistant message.
    println!("--- reply() ---");
    let reply = agent.reply(Some(vec![msg.clone()])).await?;
    if let Some(text) = reply.get_text_content("") {
        println!("assistant: {text}");
    }

    // Option 2: stream incremental events (text deltas, tool calls, ...).
    println!("\n--- reply_stream() ---");
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;
    while let Some(event) = stream.next().await {
        match event {
            AgentEvent::TextBlockDelta(delta) => print!("{}", delta.delta),
            AgentEvent::ToolCallStart(start) => {
                println!(
                    "\n[tool call] {} ({})",
                    start.tool_call_name, start.tool_call_id
                );
            }
            AgentEvent::RequireUserConfirm(confirm) => {
                let names: Vec<String> =
                    confirm.tool_calls.iter().map(|b| b.name.clone()).collect();
                println!("\n[needs confirmation] tools: {names:?}");
            }
            AgentEvent::ReplyEnd(_) => println!("\n[reply end]"),
            AgentEvent::ExceedMaxIters(_) => println!("\n[exceeded max iterations]"),
            _ => {}
        }
    }

    Ok(())
}
