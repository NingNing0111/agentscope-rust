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
├── examples/            # 可运行示例（推荐先运行 agent_demo）
└── docs/                # 本文档站点
```

**重要**：仓库根 package `agentscope` 仅承载示例，不提供 facade 库。使用时直接依赖具体的 `agent_scope_*` crate（见第 6 节）。

---

## 3. 配置模型服务凭据

示例通过 `dotenv` 加载仓库根目录的 `.env` 文件。创建 `.env`：

```bash
echo 'API_KEY=sk-your-real-key' > .env
```

`.env` 已被 `.gitignore`（`.env*` 规则）忽略，不会进入版本控制。

**各示例读取凭据的方式如下**：

| 示例 | 凭据来源 |
|------|----------|
| `agent_demo` | `--api-key` 或 `.env`/环境变量 `API_KEY` |
| `chat.rs` | `--api-key` 或 `.env`/环境变量 `API_KEY` |
| `session_test.rs` 等离线示例 | 无需凭据 |

凭据由示例入口显式传入模型构造函数（如 `DashScopeChatModel::new(api_key, model_name)`），crate 内部不读取环境变量。

---

## 4. 运行第一个 Agent

推荐先运行完整 Agent Demo。它会调用真实 DashScope API，并启动一个支持流式输出、工具调用、权限演示和多轮上下文的交互式 REPL：

```bash
cargo run --example agent_demo
```

显示模型、工具、权限和回复生命周期事件：

```bash
cargo run --example agent_demo -- --model qwen-plus --show-events
```

发送一次性 prompt 后退出：

```bash
cargo run --example agent_demo -- --prompt "请用 calculator 计算 23 * (17 + 5)"
```

你会看到：

```text
AgentScope Rust Interactive Agent Demo
Model: qwen-plus
API key: [REDACTED:xxxx]
Tools: calculator, safe_time, demo_knowledge_lookup, dangerous_demo_action(denied)
Type /help for commands, /exit to quit. This demo calls the real DashScope API.
```

尝试输入：

```text
> 请用 calculator 计算 15 * 27 + 3
```

你将观察到完整的 Agent 事件流：文本增量、模型调用边界、工具调用、工具结果、权限拒绝与最终回答。输入 `/help` 查看 REPL 命令，输入 `/exit` 退出。

其他常用示例：

```bash
# 只体验快速终端聊天
cargo run --example chat -- --model qwen-plus

# 离线示例：会话持久化（无需 API Key）
cargo run --example session_test
```

---

## 5. 十分钟看懂代码

`agent_demo` 示例的主线只有四步（完整代码见 `examples/agent-demo/main.rs`、`tools.rs` 与 `render.rs`）：

**① 创建模型** — `DashScopeChatModel::new` 接收显式传入的凭据与模型名：

<!-- source: examples/agent-demo/main.rs -->
```rust
let model = Arc::new(DashScopeChatModel::new(&config.api_key, &config.model).with_stream(true));
```

**② 注册工具** — `FunctionTool::new` 封装一个异步处理函数：

<!-- source: examples/agent-demo/tools.rs -->
```rust
let mut toolkit = ToolKit::new();
toolkit.register(FunctionTool::new("calculator", "...", calculator));
```

**③ 组装 Agent** — `AgentConfig` builder + `ReActAgent::new`：

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

**④ 发起对话** — 非流式 `reply` 或流式 `reply_stream`：

<!-- source: examples/agent-demo/main.rs -->
```rust
let mut stream = agent.reply_stream(Some(vec![user_msg("user", input)?])).await?;
while let Some(event) = stream.next().await {
    renderer.render(&event)?;
}
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

最小可运行程序可参考 `examples/agent-demo/main.rs`：构造模型 → 注册工具与权限 → 组装 `ReActAgent` → 用 `user_msg` 构造消息 → 调用 `agent.reply_stream(...)` 并消费 `AgentEvent`。

---

## 7. 常见问题排查

| 现象 | 原因与解决 |
|------|-----------|
| 运行 `agent_demo` 时提示缺少 `API_KEY` | 仓库根 `.env` 缺失、`API_KEY` 为空，或没有传 `--api-key`；按第 3 节创建 `.env` |
| DashScope API 返回 `invalid api key` | `API_KEY` 值无效或已过期；输出会脱敏，不会打印原始 key |
| 请求超时 / 网络错误 | 检查网络连通性和 DashScope 服务状态；这是调用真实 API 的示例 |
| 提示 "Agent is busy (already streaming)" | 上一次流式回复未结束又发起了新回复；REPL 会串行等待当前回复完成 |
| 想查看工具与权限事件 | 运行 `cargo run --example agent_demo -- --show-events` |
| 想查看脱敏后的 JSON 事件 | 运行 `cargo run --example agent_demo -- --show-json-events` |

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
