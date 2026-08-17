//! Streaming chat example: consume every event type produced by `reply_stream`.
//!
//! Unlike the `quickstart` example (which mostly prints final text), this one
//! dispatches on each [`AgentEvent`] variant — text deltas, thinking deltas,
//! tool-call lifecycle, confirmation requests, and the reply end event.

use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use agent_scope_rig::RigChatModel;
use agent_scope_tool::{FunctionTool, ToolKit};
use clap::Parser;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::Deserialize;

/// Arguments for the calculator tool. Only needs `Deserialize` + `JsonSchema` —
/// the tool's JSON schema is derived automatically.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput {
    expression: String,
}

async fn calculator(input: CalcInput) -> String {
    format!("calced: {}", input.expression)
}

#[derive(Parser)]
struct Cli {
    /// User prompt to send to the agent.
    #[arg(short, long, default_value = "你好，请用一句话介绍你自己。")]
    prompt: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    dotenv::dotenv().ok();

    let api_key = std::env::var("DEFAULT_API_KEY")
        .map_err(|_| anyhow::anyhow!("error: 缺少环境变量 DEFAULT_API_KEY。请设置后重试。"))?;

    // 模型名从 DEFAULT_CHAT_MODEL 读取（fallback qwen3.7-plus）；DEFAULT_URL 可选覆盖端点。
    let model_name =
        std::env::var("DEFAULT_CHAT_MODEL").unwrap_or_else(|_| "qwen3.7-plus".to_string());
    let mut model = RigChatModel::openai(&api_key, &model_name)?;
    if let Ok(base_url) = std::env::var("DEFAULT_URL") {
        model = model.with_base_url(base_url);
    }
    let model = Arc::new(model.with_stream(true));

    let mut toolkit = ToolKit::new();
    toolkit.register(FunctionTool::new(
        "calculator",
        "Evaluate a math expression.",
        calculator,
    ));

    let agent_config = AgentConfig::builder()
        .name("assistant")
        .system_prompt("你是一个乐于助人的助手。当用户请求数学计算时，请使用 calculator 工具。")
        .model(model)
        .toolkit(toolkit)
        .build()?;

    let agent = ReActAgent::new(
        agent_config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?;

    let msg = user_msg("user", &cli.prompt).map_err(|e| anyhow::anyhow!("{e:?}"))?;

    println!("--- streaming events ---");
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;
    while let Some(event) = stream.next().await {
        match &event {
            AgentEvent::ReplyStart(e) => println!("[reply start] id={}", e.reply_id),
            // Thinking deltas render dimmed so they read differently from final text.
            AgentEvent::ThinkingBlockDelta(d) => print!("\x1b[2m{}\x1b[0m", d.delta),
            AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
            AgentEvent::ModelCallStart(m) => println!("\n[model call] model={}", m.model_name),
            AgentEvent::ToolCallStart(s) => {
                println!("\n[tool start] {} ({})", s.tool_call_name, s.tool_call_id)
            }
            AgentEvent::ToolCallDelta(d) => print!("{}", d.delta),
            AgentEvent::ToolCallEnd(e) => println!(" [tool end] {}", e.tool_call_id),
            AgentEvent::ToolResultStart(r) => {
                println!(
                    "[tool result start] {} ({})",
                    r.tool_call_name, r.tool_call_id
                )
            }
            AgentEvent::ToolResultTextDelta(d) => print!("{}", d.delta),
            AgentEvent::ToolResultEnd(e) => println!("\n[tool result end] {}", e.tool_call_id),
            AgentEvent::RequireUserConfirm(c) => {
                let names: Vec<String> = c.tool_calls.iter().map(|b| b.name.clone()).collect();
                println!("\n[needs confirmation] tools: {names:?}");
            }
            AgentEvent::UserInterrupt(_) => println!("\n[interrupted by user]"),
            AgentEvent::ExceedMaxIters(_) => println!("\n[exceeded max iterations]"),
            AgentEvent::ReplyEnd(e) => {
                println!("\n[reply end] finished_reason={:?}", e.finished_reason)
            }
            _ => {}
        }
    }

    Ok(())
}
