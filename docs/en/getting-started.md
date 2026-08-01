# Getting Started

> From zero to your first AgentScope Rust agent in 30 minutes.

This guide is for developers who know Rust and are new to AgentScope. When you finish, you will have: configured your model-service credentials, run a terminal chat agent with streaming output, thinking mode, and tool calling, and learned how to use the crates in your own project.

---

## 1. Prerequisites

| Requirement | Details |
|-------------|---------|
| Rust toolchain | stable toolchain via [rustup](https://rustup.rs/) (the workspace uses the 2024 edition; 1.85+ recommended) |
| DashScope API key | an API key from Alibaba Cloud Model Studio (DashScope), starting with `sk-` |
| This repository | AgentScope Rust is currently distributed as source (not yet on crates.io); clone the repository to use it |

Verify your toolchain:

```bash
cargo --version
```

---

## 2. Project Layout at a Glance

```text
agentscope-rust/
├── crates/              # 14 functional crates (agent_scope_*)
├── examples/            # runnable examples (start with agent_demo)
└── docs/                # this documentation site
```

**Important**: the root package `agentscope` only hosts the examples — it is not a facade library. Depend on the concrete `agent_scope_*` crates directly (see Section 6).

---

## 3. Configuring Credentials

The examples load a `.env` file at the repository root via `dotenv`. Create it:

```bash
echo 'API_KEY=sk-your-real-key' > .env
```

`.env` is ignored by `.gitignore` (the `.env*` rule) and will not be committed.

**Examples read credentials as follows**:

| Example | Credential source |
|---------|-------------------|
| `agent_demo` | `--api-key` or `.env`/environment variable `API_KEY` |
| `chat.rs` | `--api-key` or `.env`/environment variable `API_KEY` |
| `session_test.rs` and other offline examples | no credentials needed |

Example binaries pass credentials explicitly to model constructors (for example, `DashScopeChatModel::new(api_key, model_name)`); the crates never read environment variables themselves.

---

## 4. Running Your First Agent

Start with the complete Agent Demo. It calls the real DashScope API and starts an interactive REPL with streaming output, tool calling, permission demonstration, and multi-turn context:

```bash
cargo run --example agent_demo
```

Show model, tool, permission, and reply lifecycle events:

```bash
cargo run --example agent_demo -- --model qwen-plus --show-events
```

Send one prompt and exit:

```bash
cargo run --example agent_demo -- --prompt "Use calculator to compute 23 * (17 + 5)"
```

You will see:

```text
AgentScope Rust Interactive Agent Demo
Model: qwen-plus
API key: [REDACTED:xxxx]
Tools: calculator, safe_time, demo_knowledge_lookup, dangerous_demo_action(denied)
Type /help for commands, /exit to quit. This demo calls the real DashScope API.
```

Try asking:

```text
> Use calculator to compute 15 * 27 + 3
```

You will observe the agent event stream: text deltas, model-call boundaries, tool calls, tool results, permission denial, and the final answer. Type `/help` for REPL commands and `/exit` to quit.

Other useful examples:

```bash
# Quick terminal chat experience
cargo run --example chat -- --model qwen-plus

# Offline example: session persistence (no API key needed)
cargo run --example session_test
```

---

## 5. Understanding the Code in Ten Minutes

The `agent_demo` example has a four-step main line (full code in `examples/agent-demo/main.rs`, `tools.rs`, and `render.rs`):

**① Create the model** — `DashScopeChatModel::new` takes the explicitly passed key and model name:

<!-- source: examples/agent-demo/main.rs -->
```rust
let model = Arc::new(DashScopeChatModel::new(&config.api_key, &config.model).with_stream(true));
```

**② Register a tool** — `FunctionTool::new` wraps an async handler function:

<!-- source: examples/agent-demo/tools.rs -->
```rust
let mut toolkit = ToolKit::new();
toolkit.register(FunctionTool::new("calculator", "...", calculator));
```

**③ Assemble the agent** — the `AgentConfig` builder + `ReActAgent::new`:

<!-- source: examples/agent-demo/main.rs -->
```rust
let config = AgentConfig::builder()
    .name("agent_demo")
    .system_prompt(system_prompt(false))
    .model(model)
    .toolkit(toolkit)
    .permission_context(permission_context)
    .build()?;
let agent = ReActAgent::new(config, react_config, ContextConfig::default(), vec![])?;
```

**④ Start a conversation** — non-streaming `reply` or streaming `reply_stream`:

<!-- source: examples/agent-demo/main.rs -->
```rust
let mut stream = agent.reply_stream(Some(vec![user_msg("user", input)?])).await?;
while let Some(event) = stream.next().await {
    renderer.render(&event)?;
}
```

User messages are built with the factory function `agent_scope_message::factory::user_msg(name, text)` (`crates/agent_scope_message/src/factory.rs:11`).

---

## 6. Using the Crates in Your Own Project

Add the crates you need as path dependencies in your `Cargo.toml`:

```toml
[dependencies]
agent_scope_agent = { path = "../agentscope-rust/crates/agent_scope_agent" }
agent_scope_dashscope = { path = "../agentscope-rust/crates/agent_scope_dashscope" }
agent_scope_tool = { path = "../agentscope-rust/crates/agent_scope_tool" }
agent_scope_message = { path = "../agentscope-rust/crates/agent_scope_message" }
tokio = { version = "1", features = ["full"] }
```

A minimal runnable program can follow `examples/agent-demo/main.rs`: build the model → register tools and permissions → assemble a `ReActAgent` → construct a message with `user_msg` → call `agent.reply_stream(...)` and consume `AgentEvent` values.

---

## 7. Troubleshooting

| Symptom | Cause and fix |
|---------|---------------|
| `agent_demo` reports a missing `API_KEY` | the repository-root `.env` file is missing, `API_KEY` is empty, or `--api-key` was not passed; create `.env` as shown in Section 3 |
| DashScope returns `invalid api key` | the `API_KEY` value is invalid or expired; output is redacted and will not print the raw key |
| Request timeouts / network errors | check connectivity and DashScope service status; this example calls the real API |
| "Agent is busy (already streaming)" | a new reply was requested before the previous streaming reply finished; the REPL waits for each reply serially |
| Show tool and permission events | run `cargo run --example agent_demo -- --show-events` |
| Show redacted JSON events | run `cargo run --example agent_demo -- --show-json-events` |

---

## 8. Next Steps

Explore the modules in the recommended reading order:

1. [Messages & Core Types](modules/message-types.md) — the Msg / ContentBlock data model
2. [Events & Streaming](modules/event-streaming.md) — every AgentEvent type and streaming semantics
3. [Model Abstraction](modules/model.md) → [DashScope Provider](modules/dashscope.md)
4. [Tool System](modules/tool.md) → [Agent System](modules/agent.md)
5. [Memory](modules/memory.md) → [Session Management](modules/session.md)
6. [RAG](modules/rag.md) → [Workspace](modules/workspace.md) → [Skill](modules/skill.md) → [Sandbox](modules/sandbox.md)

Other entry points:

- [Python → Rust Migration Guide](migration.md) — if you know the Python edition of AgentScope
- [Tutorial: RAG Knowledge-Base Chat](tutorials/rag-knowledge-chat.md) — an end-to-end walkthrough across modules
