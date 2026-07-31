//! Interactive terminal-chat example for AgentScope with thinking mode.
//!
//! Connects to DashScope (Alibaba Cloud Model Studio) and runs a
//! conversational ReActAgent with streaming output and reasoning display.
//!
//! Every content block (text, thinking, tool call, tool result, hint, data)
//! is rendered to the terminal in real-time.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example chat -- --api-key sk-xxxxx
//! cargo run --example chat -- --api-key sk-xxxxx --model qwen-plus
//! cargo run --example chat -- --api-key sk-xxxxx --model qwen-plus --no-thinking
//! ```
//!
//! In the chat loop, type `exit` or `quit` to leave, Ctrl+C to interrupt.

use std::io::{self, Write};

use agent_scope_agent::Agent;
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use agent_scope_tool::ToolKit;
use clap::Parser;
use futures::StreamExt;

mod common;
use common::{build_agent, create_calculator_tool, create_model_with_thinking};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Terminal chat with a DashScope-powered Agent — streaming with thinking display.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// DashScope API key (starts with "sk-").
    #[arg(short = 'k', long)]
    api_key: String,

    /// Model name, e.g. "qwen-plus" or "qwen-max".
    #[arg(short = 'm', long, default_value = "qwen-plus")]
    model: String,

    /// Disable thinking/reasoning mode (enabled by default).
    #[arg(long)]
    no_thinking: bool,
}

// ---------------------------------------------------------------------------
// Block tracking for structured output
// ---------------------------------------------------------------------------

/// Track per-block state so we can render start/end markers.
#[derive(Default)]
struct BlockTracker {
    current_text_id: Option<String>,
    current_thinking_id: Option<String>,
    current_tool_call_id: Option<String>,
    current_tool_result_id: Option<String>,
    current_data_id: Option<String>,
}

impl BlockTracker {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

// ---------------------------------------------------------------------------
// Event rendering
// ---------------------------------------------------------------------------

/// Render an incoming AgentEvent to stdout using the event's Rust enum variant
/// name as the label, so every event is shown exactly as it is emitted.
fn render_event(event: &AgentEvent, tracker: &mut BlockTracker) {
    match event {
        // ── Reply lifecycle ──────────────────────────────────────────
        AgentEvent::ReplyStart(e) => {
            println!(
                "\x1b[90m── ReplyStart (name={}, role={}) ──\x1b[0m",
                e.name, e.role
            );
        }
        AgentEvent::ReplyEnd(e) => {
            let reason = format!("{:?}", e.finished_reason).to_lowercase();
            if let Some(ref err) = e.error {
                println!(
                    "\x1b[90m── ReplyEnd ({reason}, error: {err:?}) ──\x1b[0m"
                );
            } else {
                println!("\x1b[90m── ReplyEnd ({reason}) ──\x1b[0m");
            }
            tracker.reset();
        }

        // ── Model call lifecycle ─────────────────────────────────────
        AgentEvent::ModelCallStart(_e) => {
            println!("\x1b[90m── ModelCallStart ──\x1b[0m");
        }
        AgentEvent::ModelCallEnd(e) => {
            println!(
                "\x1b[90m── ModelCallEnd (in={in_tok}, out={out_tok}) ──\x1b[0m",
                in_tok = e.input_tokens,
                out_tok = e.output_tokens,
            );
        }

        // ── Text block ───────────────────────────────────────────────
        AgentEvent::TextBlockStart(e) => {
            tracker.current_text_id = Some(e.block_id.clone());
            println!();
            println!(
                "\x1b[36m── TextBlockStart (id={}) ──\x1b[0m",
                e.block_id
            );
        }
        AgentEvent::TextBlockDelta(e) => {
            println!(
                "\x1b[36m── TextBlockDelta (id={}, len={}) ──\x1b[0m",
                e.block_id,
                e.delta.len()
            );
            println!("\x1b[36m{}\x1b[0m", e.delta);
        }
        AgentEvent::TextBlockEnd(e) => {
            println!();
            println!(
                "\x1b[36m── TextBlockEnd (id={}) ──\x1b[0m",
                e.block_id
            );
            tracker.current_text_id = None;
        }

        // ── Thinking block ───────────────────────────────────────────
        AgentEvent::ThinkingBlockStart(e) => {
            tracker.current_thinking_id = Some(e.block_id.clone());
            println!();
            println!(
                "\x1b[35m── ThinkingBlockStart (id={}) ──\x1b[0m",
                e.block_id
            );
        }
        AgentEvent::ThinkingBlockDelta(e) => {
            println!(
                "\x1b[35m── ThinkingBlockDelta (id={}, len={}) ──\x1b[0m",
                e.block_id,
                e.delta.len()
            );
            println!("\x1b[35;2m{}\x1b[0m", e.delta);
        }
        AgentEvent::ThinkingBlockEnd(e) => {
            println!();
            println!(
                "\x1b[35m── ThinkingBlockEnd (id={}) ──\x1b[0m",
                e.block_id
            );
            tracker.current_thinking_id = None;
        }

        // ── Tool call block ──────────────────────────────────────────
        AgentEvent::ToolCallStart(e) => {
            tracker.current_tool_call_id = Some(e.tool_call_id.clone());
            println!();
            println!(
                "\x1b[33m── ToolCallStart (id={}, name={}) ──\x1b[0m",
                e.tool_call_id, e.tool_call_name,
            );
        }
        AgentEvent::ToolCallDelta(e) => {
            println!(
                "\x1b[33m── ToolCallDelta (id={}, len={}) ──\x1b[0m",
                e.tool_call_id,
                e.delta.len()
            );
            println!("\x1b[33m{}\x1b[0m", e.delta);
        }
        AgentEvent::ToolCallEnd(e) => {
            println!();
            println!(
                "\x1b[33m── ToolCallEnd (id={}) ──\x1b[0m",
                e.tool_call_id
            );
            tracker.current_tool_call_id = None;
        }

        // ── Tool result block ────────────────────────────────────────
        AgentEvent::ToolResultStart(e) => {
            tracker.current_tool_result_id = Some(e.tool_call_id.clone());
            println!(
                "\x1b[32m── ToolResultStart (id={}, name={}) ──\x1b[0m",
                e.tool_call_id, e.tool_call_name,
            );
        }
        AgentEvent::ToolResultTextDelta(e) => {
            println!(
                "\x1b[32m── ToolResultTextDelta (id={}, len={}) ──\x1b[0m",
                e.tool_call_id,
                e.delta.len()
            );
            println!("\x1b[32m{}\x1b[0m", e.delta);
        }
        AgentEvent::ToolResultDataDelta(e) => {
            let len = e.data.as_ref().map_or(0, |s| s.len());
            println!(
                "\x1b[32m── ToolResultDataDelta (id={}, len={len}) ──\x1b[0m",
                e.tool_call_id,
            );
        }
        AgentEvent::ToolResultEnd(e) => {
            println!();
            println!(
                "\x1b[32m── ToolResultEnd (id={}) ──\x1b[0m",
                e.tool_call_id
            );
            tracker.current_tool_result_id = None;
        }

        // ── Data block ───────────────────────────────────────────────
        AgentEvent::DataBlockStart(e) => {
            tracker.current_data_id = Some(e.block_id.clone());
            println!(
                "\x1b[34m── DataBlockStart (id={}, media={}) ──\x1b[0m",
                e.block_id, e.media_type,
            );
        }
        AgentEvent::DataBlockDelta(e) => {
            // Don't flood terminal with raw base64
            println!(
                "\x1b[34m── DataBlockDelta (id={}, data_len={}) ──\x1b[0m",
                e.block_id,
                e.data.len()
            );
        }
        AgentEvent::DataBlockEnd(e) => {
            println!();
            println!(
                "\x1b[34m── DataBlockEnd (id={}) ──\x1b[0m",
                e.block_id
            );
            tracker.current_data_id = None;
        }

        // ── Hint block ───────────────────────────────────────────────
        AgentEvent::HintBlock(e) => {
            let detail = format!("{:?}", e.hint);
            println!("\x1b[90m── HintBlock ({detail}) ──\x1b[0m");
        }

        // ── User / control events ────────────────────────────────────
        AgentEvent::UserInterrupt(_e) => {
            println!("\x1b[31m── UserInterrupt ──\x1b[0m");
        }
        AgentEvent::ExceedMaxIters(e) => {
            println!(
                "\x1b[31m── ExceedMaxIters (agent={name}) ──\x1b[0m",
                name = e.name,
            );
        }
        AgentEvent::RequireUserConfirm(e) => {
            let count = e.tool_calls.len();
            println!(
                "\x1b[33m── RequireUserConfirm (count={count}) ──\x1b[0m"
            );
        }
        AgentEvent::UserConfirmResult(e) => {
            let approved = e.confirm_results.iter().filter(|r| r.confirmed).count();
            let rejected = e.confirm_results.len() - approved;
            println!(
                "\x1b[90m── UserConfirmResult (approved={approved}, rejected={rejected}) ──\x1b[0m"
            );
        }
        AgentEvent::RequireExternalExecution(e) => {
            let count = e.tool_calls.len();
            println!(
                "\x1b[33m── RequireExternalExecution (count={count}) ──\x1b[0m"
            );
        }
        AgentEvent::ExternalExecutionResult(e) => {
            let count = e.execution_results.len();
            println!(
                "\x1b[90m── ExternalExecutionResult (count={count}) ──\x1b[0m"
            );
        }

        // ── Session events ───────────────────────────────────────────
        AgentEvent::SessionCreated(e) => {
            println!(
                "\x1b[90m── SessionCreated ({id}) ──\x1b[0m",
                id = e.session_id
            );
        }
        AgentEvent::SessionClosed(e) => {
            println!(
                "\x1b[90m── SessionClosed ({id}, reason: {reason}) ──\x1b[0m",
                id = e.session_id,
                reason = e.reason,
            );
        }
        AgentEvent::SessionSaved(e) => {
            println!(
                "\x1b[90m── SessionSaved ({id}, {n} msgs) ──\x1b[0m",
                id = e.session_id,
                n = e.message_count,
            );
        }
        AgentEvent::SessionLoaded(e) => {
            println!(
                "\x1b[90m── SessionLoaded ({id}, {n} msgs) ──\x1b[0m",
                id = e.session_id,
                n = e.message_count,
            );
        }
        AgentEvent::SessionTrimmed(e) => {
            println!(
                "\x1b[90m── SessionTrimmed ({id}: {before} → {after} msgs) ──\x1b[0m",
                id = e.session_id,
                before = e.messages_before,
                after = e.messages_after,
            );
        }

        // ── Custom events ────────────────────────────────────────────
        AgentEvent::Custom(e) => {
            println!(
                "\x1b[90m── Custom (name={name}, value={value:?}) ──\x1b[0m",
                name = e.name,
                value = e.value,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // -- Build model with thinking enabled --
    let thinking_budget = if cli.no_thinking {
        None
    } else {
        // Enable thinking; pick a budget appropriate for the model
        // qwen-plus supports thinking with budget up to 16384
        Some(8192u32)
    };

    let model = if cli.no_thinking {
        common::create_model(&cli.api_key, &cli.model)
    } else {
        create_model_with_thinking(&cli.api_key, &cli.model, thinking_budget)
    };

    // -- Build toolkit --
    let mut toolkit = ToolKit::new();
    toolkit.register(create_calculator_tool());

    // -- Build agent --
    let agent = match build_agent(model, Some(toolkit)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Failed to create agent: {e}");
            std::process::exit(1);
        }
    };

    // -- Header --
    let thinking_status = if cli.no_thinking { "off" } else { "on" };
    println!("╔══════════════════════════════════════════════╗");
    println!("║   AgentScope Terminal Chat (Streaming)      ║");
    println!("║   Model: {:<36}║", cli.model);
    println!("║   Tools: calculator                        ║");
    println!("║   Thinking: {:<32}║", thinking_status);
    println!("║   Mode:   streaming (all blocks shown)     ║");
    println!("╠══════════════════════════════════════════════╣");
    println!("║   Type 'exit'/'quit' to leave               ║");
    println!("║   Ctrl+C to interrupt the agent             ║");
    println!("╚══════════════════════════════════════════════╝");
    println!();

    // -- Interaction loop --
    let stdin = io::stdin();
    let mut line = String::new();

    loop {
        // Prompt
        print!("\x1b[36m> \x1b[0m");
        let _ = io::stdout().flush();

        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0) => {
                // EOF (Ctrl+D)
                println!("\nGoodbye!");
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match trimmed.to_lowercase().as_str() {
            "exit" | "quit" => {
                println!("Goodbye!");
                break;
            }
            _ => {}
        }

        // Build user message
        let msg = match user_msg("user", trimmed) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to create message: {e:?}");
                continue;
            }
        };

        // ── Streaming reply ──────────────────────────────────────────
        let mut stream = match agent.reply_stream(Some(vec![msg])).await {
            Ok(s) => s,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.to_lowercase().contains("invalid")
                    && (err_str.to_lowercase().contains("api")
                        || err_str.to_lowercase().contains("key")
                        || err_str.to_lowercase().contains("apikey"))
                {
                    eprintln!("DashScope API error — check your API key.");
                    eprintln!("Details: {err_str}");
                    std::process::exit(1);
                }
                if err_str.to_lowercase().contains("timeout")
                    || err_str.to_lowercase().contains("connection")
                    || err_str.to_lowercase().contains("network")
                {
                    eprintln!("Request failed (network/timeout), retrying: {err_str}");
                    continue;
                }
                if err_str.to_lowercase().contains("already") {
                    eprintln!(
                        "Agent is busy (already streaming) — wait for the current reply to finish."
                    );
                    continue;
                }
                eprintln!("Agent stream error: {err_str}");
                continue;
            }
        };

        // Consume the event stream, rendering each event
        let mut tracker = BlockTracker::default();
        while let Some(event) = stream.next().await {
            render_event(&event, &mut tracker);
        }
        println!(); // trailing blank line after each reply
    }
}
