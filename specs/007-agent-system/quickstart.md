# Quickstart: Agent System

**Feature**: 007-agent-system | **Date**: 2026-07-29

## Prerequisites

- Rust toolchain (stable, edition 2024)
- Workspace built: `cargo build` from repo root
- All foundation crates compiled (agent_scope_model, agent_scope_tool, agent_scope_state, agent_scope_message, agent_scope_event, agent_scope_types)

## Setup

Add `agent_scope_agent` to your dependency (when published) or use workspace path:

```toml
[dependencies]
agent_scope_agent = { path = "crates/agent_scope_agent" }
agent_scope_model = { path = "crates/agent_scope_model" }
agent_scope_message = { path = "crates/agent_scope_message" }
```

## Scenario 1: Basic Text Agent (US1)

Create a simple conversational agent that echoes user input.

```rust
use agent_scope_agent::{Agent, AgentConfig, ReActAgent, ReActConfig, ContextConfig};
use agent_scope_message::factory::user_msg;
use agent_scope_model::ChatModel;  // your mock/test model
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a mock model that echoes input
    // (MockModel comes from agent_scope_agent test utilities)
    
    // 2. Configure the agent
    let config = AgentConfig::builder()
        .name("echo-bot")
        .system_prompt("You are a helpful assistant.")
        .model(my_model)
        .build()?;

    // 3. Create the agent
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],  // no middleware
    )?;

    // 4. Send a message and get a reply
    let input = user_msg("user", "Hello!")?;
    let reply = agent.reply(Some(vec![input])).await?;

    // 5. Verify
    assert_eq!(reply.role, agent_scope_message::Role::Assistant);
    println!("Agent replied: {:?}", reply.get_text_content());

    Ok(())
}
```

**Expected outcome**: 
- Agent emits events in the order: ReplyStart → ModelCallStart → ModelCallEnd → TextBlockStart → TextBlockDelta → TextBlockEnd → ReplyEnd
- Returns a `Msg` with `role = Assistant`
- Agent state context contains both the user message and the assistant reply

## Scenario 2: ReAct Agent with Tools (US2)

Create an agent that can use a calculator tool.

```rust
use agent_scope_agent::{Agent, AgentConfig, ReActAgent, ReActConfig, ContextConfig};
use agent_scope_tool::{FunctionTool, ToolKit};
use agent_scope_message::factory::user_msg;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput {
    expression: String,
}

async fn calculate(input: CalcInput) -> String {
    // Simple eval — in production, use a safe expression parser
    format!("Result of '{}' = 42", input.expression)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create tool
    let calc_tool = FunctionTool::new("calculator", "Evaluate math expressions", calculate);
    let mut toolkit = ToolKit::new();
    toolkit.register(calc_tool);

    // 2. Configure agent with tools
    let config = AgentConfig::builder()
        .name("math-bot")
        .system_prompt("You are a math assistant. Use tools when needed.")
        .model(my_reasoning_model)
        .toolkit(toolkit)
        .build()?;

    let agent = ReActAgent::new(
        config,
        ReActConfig { max_iters: 5, ..Default::default() },
        ContextConfig::default(),
        vec![],
    )?;

    // 3. Send a math question
    let reply = agent.reply(Some(vec![user_msg("user", "What is 2+2?")?])).await?;

    println!("Final answer: {}", reply.get_text_content());

    Ok(())
}
```

**Expected outcome**:
- If model returns tool_call for "calculator" → agent executes tool → feeds result back → model gives final answer
- Full event trace includes tool lifecycle: ToolCallStart → ToolCallEnd → ToolResultStart → ToolResultEnd
- Max 5 iterations (will stop early if model returns text directly)

## Scenario 3: Middleware Integration (US3)

Add logging and content moderation to an agent.

```rust
use agent_scope_agent::{Middleware, ReActAgent, AgentError, Agent};
use agent_scope_message::{Msg, ContentBlock};
use async_trait::async_trait;
use std::sync::Arc;

// --- Logging Middleware ---
struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn pre_reply(&self, agent: &ReActAgent, input: &mut Option<Vec<Msg>>) -> Result<(), AgentError> {
        println!("[{}] Reply started with {} messages", 
            agent.name(), 
            input.as_ref().map(|m| m.len()).unwrap_or(0));
        Ok(())
    }

    async fn post_reply(&self, agent: &ReActAgent, result: &Result<Msg, AgentError>) -> Result<(), AgentError> {
        match result {
            Ok(_) => println!("[{}] Reply completed successfully", agent.name()),
            Err(e) => println!("[{}] Reply failed: {}", agent.name(), e),
        }
        Ok(())
    }
}

// --- Usage ---
let middlewares: Vec<Arc<dyn Middleware>> = vec![
    Arc::new(LoggingMiddleware),
];

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    middlewares,
)?;
```

**Expected outcome**:
- `pre_reply` fires before model call
- `post_reply` fires after reply completes (success or error)
- Each middleware hook can be implemented independently

## Scenario 4: Interruption (US4)

Interrupt a long-running agent and verify clean shutdown.

```rust
use tokio::time::{timeout, Duration};

// Start a reply in background
let agent_clone = agent.clone();
let handle = tokio::spawn(async move {
    agent_clone.reply(Some(vec![user_msg("user", "Long task...")?])).await
});

// Interrupt after 1 second
tokio::time::sleep(Duration::from_secs(1)).await;
agent.interrupt();

// Wait for result with timeout
match timeout(Duration::from_secs(5), handle).await {
    Ok(Ok(Ok(msg))) => {
        assert_eq!(msg.get_text_content(), "The execution was interrupted.");
    }
    _ => panic!("Expected clean interruption"),
}
```

**Expected outcome**:
- Agent returns control within configurable grace period
- ReplyEnd has `finished_reason = Interrupted`
- Returned Msg contains `interruption_message`
- After interruption, agent can accept new `reply()` calls normally

## Running Tests

```bash
# Run all agent tests
cargo test -p agent_scope_agent

# Run specific test categories
cargo test -p agent_scope_agent -- react_agent    # ReActAgent tests
cargo test -p agent_scope_agent -- middleware      # Middleware tests
cargo test -p agent_scope_agent -- event_sequence  # Event trace tests

# Run with trace output
RUST_LOG=debug cargo test -p agent_scope_agent -- --nocapture
```

## Validation Checklist

After completing implementation, verify:

- [ ] `cargo build -p agent_scope_agent` compiles without errors
- [ ] `cargo test -p agent_scope_agent` — all tests pass
- [ ] `cargo clippy -p agent_scope_agent` — no warnings
- [ ] `cargo fmt --check` — formatting clean
- [ ] Scenario 1: Basic agent replies with correct event sequence
- [ ] Scenario 2: Tool call → execution → result → final answer
- [ ] Scenario 3: All middleware hooks fire correctly
- [ ] Scenario 4: Interruption returns cleanly; agent resumes
- [ ] Edge case: Empty response from model
- [ ] Edge case: Tool call with no tools registered
- [ ] Edge case: Context compression triggered
- [ ] Edge case: `max_iters` exceeded
