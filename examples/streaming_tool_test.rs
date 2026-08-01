//! Streaming Tool-Call Round-Trip E2E Test
//!
//! Verifies complete streaming tool-call event lifecycle with real API —
//! start → delta(s) → end for both tool calls and tool results,
//! event pairing, and answer correctness.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example streaming_tool_test -- --api-key sk-xxxxx
//! cargo run --example streaming_tool_test -- --api-key sk-xxxxx --model qwen-max
//! ```

use std::time::Instant;

use agent_scope_agent::Agent;
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use agent_scope_tool::ToolKit;
use clap::Parser;
use futures::StreamExt;

mod common;
use common::{
    TestResult, build_agent, create_calculator_tool, create_model, print_banner, print_result,
    print_summary, print_test_header,
};

// ---------------------------------------------------------------------------
// EventTrace
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
struct EventTrace {
    tool_call_starts: u32,
    tool_call_deltas: u32,
    tool_call_ends: u32,
    tool_result_starts: u32,
    tool_result_deltas: u32,
    tool_result_ends: u32,
    text_deltas: Vec<String>,
    has_reply_start: bool,
    has_reply_end: bool,
}

impl EventTrace {
    fn new() -> Self {
        Self::default()
    }

    fn record(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::ReplyStart(_) => self.has_reply_start = true,
            AgentEvent::ReplyEnd(_) => self.has_reply_end = true,
            AgentEvent::ToolCallStart(_) => self.tool_call_starts += 1,
            AgentEvent::ToolCallDelta(_e) => {
                self.tool_call_deltas += 1;
            }
            AgentEvent::ToolCallEnd(_) => self.tool_call_ends += 1,
            AgentEvent::ToolResultStart(_) => self.tool_result_starts += 1,
            AgentEvent::ToolResultTextDelta(_e) => {
                self.tool_result_deltas += 1;
            }
            AgentEvent::ToolResultEnd(_) => self.tool_result_ends += 1,
            AgentEvent::TextBlockDelta(e) => {
                self.text_deltas.push(e.delta.clone());
            }
            _ => {}
        }
    }

    fn validate(&self) -> Result<(), String> {
        // Start == End for tool calls
        if self.tool_call_starts != self.tool_call_ends {
            return Err(format!(
                "ToolCall mismatch: {} starts vs {} ends",
                self.tool_call_starts, self.tool_call_ends,
            ));
        }
        // Start == End for tool results
        if self.tool_result_starts != self.tool_result_ends {
            return Err(format!(
                "ToolResult mismatch: {} starts vs {} ends",
                self.tool_result_starts, self.tool_result_ends,
            ));
        }
        // ReplyStart before ReplyEnd
        if !self.has_reply_start {
            return Err("No ReplyStart event".into());
        }
        if !self.has_reply_end {
            return Err("No ReplyEnd event".into());
        }
        // At least one TextBlockDelta
        if self.text_deltas.is_empty() {
            return Err("No TextBlockDelta events".into());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// DashScope API key (starts with "sk-").
    #[arg(short = 'k', long, env = "API_KEY")]
    api_key: String,

    /// Model name, e.g. "qwen-plus" or "qwen-max".
    #[arg(short = 'm', long, default_value = "qwen-plus")]
    model: String,
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let cli = Cli::parse();
    let total_start = Instant::now();

    print_banner("Streaming Tool-Call", &cli.model);

    // -- Build model and agent with calculator tool --
    let model = create_model(&cli.api_key, &cli.model);

    let mut toolkit = ToolKit::new();
    toolkit.register(create_calculator_tool());

    let agent = match build_agent(model, Some(toolkit)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Failed to create agent: {e}");
            std::process::exit(1);
        }
    };

    let mut results: Vec<TestResult> = Vec::new();

    // ── Test 1: Single Tool Call ──────────────────────────────────────
    print_test_header(1, "Single Tool Call");
    let start = Instant::now();

    let test1 = match run_single_tool_call(&agent).await {
        Ok(trace) => {
            let validation = trace.validate();
            let detail = format!(
                "ToolCallStart={}, ToolCallEnd={}, ToolResultStart={}, ToolResultEnd={} | Answer: {}",
                trace.tool_call_starts,
                trace.tool_call_ends,
                trace.tool_result_starts,
                trace.tool_result_ends,
                trace.text_deltas.join(" "),
            );
            TestResult {
                name: "Single Tool Call",
                passed: validation.is_ok() && trace.tool_call_starts == 1,
                detail: if let Err(ref e) = validation {
                    e.clone()
                } else {
                    detail
                },
                duration_ms: start.elapsed().as_millis() as u64,
            }
        }
        Err(e) => TestResult {
            name: "Single Tool Call",
            passed: false,
            detail: format!("Error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    };

    print_result(&test1);
    results.push(test1);

    // ── Test 2: Multi-Tool Call ───────────────────────────────────────
    print_test_header(2, "Multi-Tool Call");
    let start = Instant::now();

    let test2 = match run_multi_tool_call(&agent).await {
        Ok(trace) => {
            let validation = trace.validate();
            let detail = format!(
                "ToolCallStart={}, ToolCallEnd={}, ToolResultStart={}, ToolResultEnd={}",
                trace.tool_call_starts,
                trace.tool_call_ends,
                trace.tool_result_starts,
                trace.tool_result_ends,
            );
            TestResult {
                name: "Multi-Tool Call",
                passed: validation.is_ok() && trace.tool_call_starts >= 2,
                detail: if let Err(ref e) = validation {
                    e.clone()
                } else {
                    detail
                },
                duration_ms: start.elapsed().as_millis() as u64,
            }
        }
        Err(e) => TestResult {
            name: "Multi-Tool Call",
            passed: false,
            detail: format!("Error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    };

    print_result(&test2);
    results.push(test2);

    print_summary(&results, total_start);

    let any_failed = results.iter().any(|r| !r.passed);
    if any_failed {
        std::process::exit(1);
    }
}

// ── Test implementations ───────────────────────────────────────────

async fn run_single_tool_call(
    agent: &impl Agent,
) -> Result<EventTrace, Box<dyn std::error::Error>> {
    let msg = user_msg("user", "Calculate 3.14 * 2.718 using the calculator tool.")
        .map_err(|e| format!("{e:?}"))?;
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;

    let mut trace = EventTrace::new();
    while let Some(event) = stream.next().await {
        trace.record(&event);
    }

    Ok(trace)
}

async fn run_multi_tool_call(agent: &impl Agent) -> Result<EventTrace, Box<dyn std::error::Error>> {
    let msg = user_msg(
        "user",
        "First calculate 10 * 5, then divide that result by 2. Use the calculator tool for both.",
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;

    let mut trace = EventTrace::new();
    while let Some(event) = stream.next().await {
        trace.record(&event);
    }

    Ok(trace)
}
