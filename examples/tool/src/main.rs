//! Tool example: custom `FunctionTool`s registered in a `ToolKit` and invoked by a `ReActAgent`.
//!
//! The example still prints each tool's generated JSON Schema, but the actual
//! calculator call is driven by the agent loop instead of calling the tool
//! directly. This is the closest shape to production use: plain async Rust
//! functions become tools, the model chooses when to call them, and
//! `reply_stream` exposes the model/tool lifecycle events.

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

/// Arguments for the calculator tool — only needs `Deserialize` + `JsonSchema`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput {
    /// A simple math expression, e.g. "2 + 2".
    expression: String,
}

async fn calculator(input: CalcInput) -> String {
    match eval_simple_expression(&input.expression) {
        Ok(value) => format!("{} = {value}", input.expression.trim()),
        Err(err) => format!("calculation error: {err}"),
    }
}

fn eval_simple_expression(expression: &str) -> Result<i64, String> {
    let mut parser = ExprParser::new(expression);
    let value = parser.parse_expr()?;
    parser.skip_ws();
    if parser.is_done() {
        Ok(value)
    } else {
        Err("unexpected trailing input".into())
    }
}

struct ExprParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> ExprParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn parse_expr(&mut self) -> Result<i64, String> {
        let mut value = self.parse_term()?;
        loop {
            self.skip_ws();
            if self.consume(b'+') {
                value = value
                    .checked_add(self.parse_term()?)
                    .ok_or_else(|| "integer overflow".to_string())?;
            } else if self.consume(b'-') {
                value = value
                    .checked_sub(self.parse_term()?)
                    .ok_or_else(|| "integer overflow".to_string())?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_term(&mut self) -> Result<i64, String> {
        let mut value = self.parse_factor()?;
        loop {
            self.skip_ws();
            if self.consume(b'*') {
                value = value
                    .checked_mul(self.parse_factor()?)
                    .ok_or_else(|| "integer overflow".to_string())?;
            } else if self.consume(b'/') {
                let rhs = self.parse_factor()?;
                if rhs == 0 {
                    return Err("division by zero".into());
                }
                value = value
                    .checked_div(rhs)
                    .ok_or_else(|| "integer overflow".to_string())?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_factor(&mut self) -> Result<i64, String> {
        self.skip_ws();
        let sign = if self.consume(b'-') { -1 } else { 1 };
        self.skip_ws();

        if self.consume(b'(') {
            let value = self.parse_expr()?;
            self.skip_ws();
            if !self.consume(b')') {
                return Err("missing closing parenthesis".into());
            }
            return value
                .checked_mul(sign)
                .ok_or_else(|| "integer overflow".to_string());
        }

        let start = self.pos;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.pos += 1;
        }
        if start == self.pos {
            return Err("expected integer".into());
        }

        let number = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| "invalid UTF-8".to_string())?
            .parse::<i64>()
            .map_err(|_| "integer out of range".to_string())?;
        number
            .checked_mul(sign)
            .ok_or_else(|| "integer overflow".to_string())
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn is_done(&self) -> bool {
        self.pos == self.input.len()
    }
}

/// Arguments for a read-file tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ReadInput {
    /// Absolute path of the file to read.
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
    /// User prompt to send to the agent.
    #[arg(
        short,
        long,
        default_value = "请使用 calculator 工具计算 6 * 7，并只用一句话给出结果。"
    )]
    prompt: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    dotenv::dotenv().ok();

    let api_key = std::env::var("DEFAULT_API_KEY")
        .map_err(|_| anyhow::anyhow!("error: 缺少环境变量 DEFAULT_API_KEY。请设置后重试。"))?;
    let model_name =
        std::env::var("DEFAULT_CHAT_MODEL").unwrap_or_else(|_| "qwen3.7-plus".to_string());
    let mut model = RigChatModel::openai(&api_key, &model_name)?;
    if let Ok(base_url) = std::env::var("DEFAULT_URL") {
        model = model.with_base_url(base_url);
    }
    let model = Arc::new(model.with_stream(true));

    // 1. Build tools from plain async functions and register them in a ToolKit.
    let mut toolkit = ToolKit::new();
    toolkit.register(FunctionTool::new(
        "calculator",
        "Evaluate a math expression.",
        calculator,
    ));
    toolkit.register(FunctionTool::new(
        "read_file",
        "Read a text file from disk.",
        read_file,
    ));
    println!(
        "registered tools: {} (custom: calculator/read_file; built-in Skill: {})",
        toolkit.len(),
        toolkit.contains("Skill")
    );

    // 2. Inspect the auto-generated, OpenAI-compatible JSON schemas before the
    // toolkit is moved into the agent.
    println!("\n--- tool schemas ---");
    for schema in toolkit.get_tool_schemas() {
        println!("{schema}");
    }

    // 3. Put the same tools behind a ReActAgent so the model can choose and
    // invoke them through the normal agent/tool pipeline.
    let agent_config = AgentConfig::builder()
        .name("tool-demo")
        .system_prompt(
            "你是一个工具演示助手。用户要求数学计算时必须调用 calculator 工具；\
             用户要求读取文件时必须调用 read_file 工具。",
        )
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

    println!("\n--- agent-driven tool call ---");
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;
    while let Some(event) = stream.next().await {
        match &event {
            AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
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
            AgentEvent::ReplyEnd(e) => {
                println!("\n[reply end] finished_reason={:?}", e.finished_reason)
            }
            _ => {}
        }
    }

    Ok(())
}
