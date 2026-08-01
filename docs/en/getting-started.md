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
├── examples/            # 7 runnable examples (this guide uses chat.rs)
└── docs/                # this documentation site
```

**Important**: the root package `agentscope` only hosts the examples — it is not a facade library. Depend on the concrete `agent_scope_*` crates directly (see Section 6).

---

## 3. Configuring Credentials

The examples load a `.env` file at the repository root via `dotenv` (`examples/chat.rs:388`). Create it:

```bash
echo 'API_KEY=sk-your-real-key' > .env
```

`.env` is ignored by `.gitignore` (the `.env*` rule) and will not be committed.

**Examples read credentials differently (pay attention)**:

| Example | Credential source |
|---------|-------------------|
| `chat.rs` | **only** the `-k` / `--api-key` CLI argument (chat.rs:40; it does not read the environment) |
| `verify_agent.rs`, `memory_test.rs`, `rag_test.rs`, `streaming_tool_test.rs` | the `API_KEY` environment variable (loaded from `.env`) or the `-k` argument |
| `session_test.rs` | no credentials needed (offline example, defaults to an empty key) |

Credentials are passed explicitly to model constructors (e.g. `DashScopeChatModel::new(api_key, model_name)`); the crates never read environment variables themselves.

---

## 4. Running Your First Agent

The `chat` example is a terminal chat agent: streaming output, thinking mode (on by default), and a built-in calculator tool.

```bash
cargo run --example chat -- -k sk-your-real-key
```

Or forward the environment variable (note: `chat` does not read `API_KEY` itself — pass it explicitly):

```bash
set -a; source .env; set +a
cargo run --example chat -- -k "$API_KEY"
```

You will see:

```text
╔══════════════════════════════════════════════╗
║   AgentScope Terminal Chat (Streaming)      ║
║   Model: qwen-plus                          ║
║   Tools: calculator                        ║
║   Thinking: on                             ║
╚══════════════════════════════════════════════╝
```

Try asking:

```text
> 帮我计算 15 * 27 + 3
```

You will observe the full agent event stream: thinking blocks → text blocks → the tool call (calculator) → the tool result → the final answer. Type `exit` to quit, `Ctrl+C` to interrupt the current reply.

Other useful examples:

```bash
# Six integration checks for ReActAgent capabilities (reads API_KEY from .env)
cargo run --example verify_agent

# Offline example: session persistence (no API key needed)
cargo run --example session_test
```

---

## 5. Understanding the Code in Ten Minutes

The `chat` example has a four-step main line (full code in `examples/chat.rs` and `examples/common.rs`):

**① Create the model** — `DashScopeChatModel::new` takes the explicitly passed key and model name:

<!-- source: examples/common.rs:L34-L36 -->
```rust
pub fn create_model(api_key: &str, model_name: &str) -> Arc<DashScopeChatModel> {
    Arc::new(DashScopeChatModel::new(api_key, model_name))
}
```

**② Register a tool** — `FunctionTool::new` wraps an async handler function:

<!-- source: examples/common.rs:L311-L317 -->
```rust
pub fn create_calculator_tool() -> FunctionTool {
    FunctionTool::new(
        "calculator",
        "Evaluate a mathematical expression. ...",
        calc_handler,
    )
}
```

**③ Assemble the agent** — the `AgentConfig` builder + `ReActAgent::new`:

<!-- source: examples/common.rs:L338-L356 -->
```rust
let mut builder = AgentConfig::builder()
    .name("assistant")
    .system_prompt(system_prompt)
    .model(model);
if let Some(tk) = toolkit {
    builder = builder.toolkit(tk);
}
let config = builder.build()?;
ReActAgent::new(config, ReActConfig::default(), ContextConfig::default(), vec![])
```

**④ Start a conversation** — non-streaming `reply` or streaming `reply_stream`:

<!-- source: examples/verify_agent.rs:L355 -->
```rust
let reply = agent.reply(Some(vec![msg])).await?;
```

<!-- source: examples/chat.rs:L479 -->
```rust
let mut stream = agent.reply_stream(Some(vec![msg])).await?;
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

A minimal runnable program follows the same pattern as `test_simple_chat` in `examples/verify_agent.rs` (L345-L365): build the model → assemble a `ReActAgent` → construct a message with `user_msg` → call `agent.reply(...)`. We recommend copying `examples/verify_agent.rs` and `examples/common.rs` as your starting templates.

---

## 7. Troubleshooting

| Symptom | Cause and fix |
|---------|---------------|
| clap reports a missing required argument `--api-key` when running `chat` | the `chat` example does not read the environment — pass `-k` explicitly (see the table in Section 3) |
| Other examples fail with a DashScope API error (invalid api key) | `.env` is missing, or `API_KEY` is empty/wrong; a missing key does not panic — the call returns an error |
| Request timeouts / network errors | check connectivity; the `chat` example retries network-class errors on the next input (chat.rs:492-497) |
| "Agent is busy (already streaming)" | a new reply was requested before the previous streaming reply finished; wait for it to complete (chat.rs:499-503) |
| Verify your setup without credentials | run the offline example `cargo run --example session_test` |
| Disable thinking mode | `cargo run --example chat -- -k ... --no-thinking` |

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
