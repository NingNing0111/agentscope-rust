# 快速上手

> 30 分钟内，从零运行你的第一个 AgentScope Rust Agent。

本文面向具备 Rust 基础、首次接触 AgentScope 的开发者。完成后你将：配置好模型服务凭据，运行一个支持流式输出、thinking 模式与工具调用的终端对话 Agent，并了解如何在自己的项目中使用各 crate。

---

## 1. 环境准备

| 前置条件 | 说明 |
|----------|------|
| Rust 工具链 | 通过 [rustup](https://rustup.rs/) 安装的 stable 工具链（workspace 使用 2024 edition，建议 1.85+） |
| DashScope API Key | 阿里云百炼平台的 API Key，以 `sk-` 开头 |
| 本仓库源码 | AgentScope Rust 当前以源码形式分发（未发布 crates.io），克隆仓库后即可使用 |

验证工具链：

```bash
cargo --version
```

---

## 2. 项目结构速览

```text
agentscope-rust/
├── crates/              # 14 个功能 crate（agent_scope_*）
├── examples/            # 7 个可运行示例（本指南使用 chat.rs）
└── docs/                # 本文档站点
```

**重要**：仓库根 package `agentscope` 仅承载示例，不提供 facade 库。使用时直接依赖具体的 `agent_scope_*` crate（见第 6 节）。

---

## 3. 配置模型服务凭据

示例通过 `dotenv` 加载仓库根目录的 `.env` 文件（`examples/chat.rs:388`）。创建 `.env`：

```bash
echo 'API_KEY=sk-your-real-key' > .env
```

`.env` 已被 `.gitignore`（`.env*` 规则）忽略，不会进入版本控制。

**各示例读取凭据的方式不同（务必注意）**：

| 示例 | 凭据来源 |
|------|----------|
| `chat.rs` | **仅** `-k` / `--api-key` 命令行参数（chat.rs:40，不读环境变量） |
| `verify_agent.rs`、`memory_test.rs`、`rag_test.rs`、`streaming_tool_test.rs` | 环境变量 `API_KEY`（经 `.env` 加载）或 `-k` 参数 |
| `session_test.rs` | 无需凭据（离线示例，默认空 key） |

凭据由调用方显式传入模型构造函数（如 `DashScopeChatModel::new(api_key, model_name)`），crate 内部不读取环境变量。

---

## 4. 运行第一个 Agent

`chat` 示例是一个终端对话 Agent：流式输出、thinking 模式（默认开启）、内置 calculator 工具。

```bash
cargo run --example chat -- -k sk-your-real-key
```

或使用环境变量传入（注意：`chat` 不读 `API_KEY`，需显式转发）：

```bash
set -a; source .env; set +a
cargo run --example chat -- -k "$API_KEY"
```

启动后将看到：

```text
╔══════════════════════════════════════════════╗
║   AgentScope Terminal Chat (Streaming)      ║
║   Model: qwen-plus                          ║
║   Tools: calculator                        ║
║   Thinking: on                             ║
╚══════════════════════════════════════════════╝
```

尝试输入：

```text
> 帮我计算 15 * 27 + 3
```

你将观察到完整的 Agent 事件流：thinking 块 → 文本块 → 工具调用（calculator）→ 工具结果 → 最终回答。输入 `exit` 退出，`Ctrl+C` 中断当前回复。

其他常用示例：

```bash
# 六项 ReActAgent 能力集成验证（读取 .env 中的 API_KEY）
cargo run --example verify_agent

# 离线示例：会话持久化（无需 API Key）
cargo run --example session_test
```

---

## 5. 十分钟看懂代码

`chat` 示例的主线只有四步（完整代码见 `examples/chat.rs` 与 `examples/common.rs`）：

**① 创建模型** — `DashScopeChatModel::new` 接收显式传入的凭据与模型名：

<!-- source: examples/common.rs:L34-L36 -->
```rust
pub fn create_model(api_key: &str, model_name: &str) -> Arc<DashScopeChatModel> {
    Arc::new(DashScopeChatModel::new(api_key, model_name))
}
```

**② 注册工具** — `FunctionTool::new` 封装一个异步处理函数：

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

**③ 组装 Agent** — `AgentConfig` builder + `ReActAgent::new`：

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

**④ 发起对话** — 非流式 `reply` 或流式 `reply_stream`：

<!-- source: examples/verify_agent.rs:L355 -->
```rust
let reply = agent.reply(Some(vec![msg])).await?;
```

<!-- source: examples/chat.rs:L479 -->
```rust
let mut stream = agent.reply_stream(Some(vec![msg])).await?;
```

其中用户消息由工厂函数构造：`agent_scope_message::factory::user_msg(name, text)`（`crates/agent_scope_message/src/factory.rs:11`）。

---

## 6. 在自己的项目中使用

在你的 `Cargo.toml` 中以路径依赖引用所需 crate（按能力选用）：

```toml
[dependencies]
agent_scope_agent = { path = "../agentscope-rust/crates/agent_scope_agent" }
agent_scope_dashscope = { path = "../agentscope-rust/crates/agent_scope_dashscope" }
agent_scope_tool = { path = "../agentscope-rust/crates/agent_scope_tool" }
agent_scope_message = { path = "../agentscope-rust/crates/agent_scope_message" }
tokio = { version = "1", features = ["full"] }
```

最小可运行程序的模式与 `examples/verify_agent.rs` 的 `test_simple_chat`（L345-L365）一致：构造模型 → 组装 `ReActAgent` → `user_msg` 构造消息 → `agent.reply(...)` 获取回复。建议直接以 `examples/verify_agent.rs` 与 `examples/common.rs` 为模板复制起步。

---

## 7. 常见问题排查

| 现象 | 原因与解决 |
|------|-----------|
| 运行 `chat` 时 clap 报错缺少必需参数 `--api-key` | `chat` 示例不读环境变量，必须显式 `-k`（见第 3 节表格） |
| 其他示例报 DashScope API 错误（invalid api key） | `.env` 缺失或 `API_KEY` 为空/错误；凭据缺失不会 panic，而是在调用时返回错误 |
| 请求超时 / 网络错误 | 检查网络连通性；`chat` 示例对网络类错误会自动重试下一轮输入（chat.rs:492-497） |
| 提示 "Agent is busy (already streaming)" | 上一次流式回复未结束又发起了新回复；等待当前回复完成（chat.rs:499-503） |
| 想无凭据验证环境 | 运行离线示例 `cargo run --example session_test` |
| 想关闭 thinking 模式 | `cargo run --example chat -- -k ... --no-thinking` |

---

## 8. 下一步

按推荐阅读顺序深入各模块：

1. [消息与基础类型](modules/message-types.md) — Msg / ContentBlock 数据模型
2. [事件与流式](modules/event-streaming.md) — AgentEvent 全类型与流式语义
3. [模型抽象](modules/model.md) → [DashScope Provider](modules/dashscope.md)
4. [工具系统](modules/tool.md) → [Agent 系统](modules/agent.md)
5. [记忆](modules/memory.md) → [会话管理](modules/session.md)
6. [RAG](modules/rag.md) → [工作空间](modules/workspace.md) → [技能](modules/skill.md) → [沙箱](modules/sandbox.md)

其他入口：

- [Python → Rust 迁移参考](migration.md) — 如果你熟悉 Python 版 AgentScope
- [场景教程：RAG 知识库问答](tutorials/rag-knowledge-chat.md) — 端到端串联多个模块
