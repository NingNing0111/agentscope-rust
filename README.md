# AgentScope Rust（Beta阶段，生产暂时不建议使用）

用 Rust 实现的 **Agent 开发框架**（AgentScope 的 Rust 重构版）。以多 crate workspace 组织，核心能力位于 `crates/agent_scope_*`，每个 crate 是独立 package，可按需单独依赖。

当前可直接体验的完整交互式编码 Agent 位于 `examples/pi-rust`（ratatui TUI + ReActAgent + 工具 + 记忆 + Skills）。

## 文档

- [AgentScope Rust 中文文档](https://ningning0111.github.io/agentscope-rust/) — 中文文档站（VitePress，随 `master` 自动发布到 GitHub Pages）
- [本地文档维护说明](docs/rust/README.md) — 文档结构、状态标注与本地构建/检查命令

## 特性一览

- **Agent 编排**：`Agent` trait、`ReActAgent`（reasoning→acting 循环）、`Middleware` 管道、权限控制、`Planner`、`SubAgent`
- **模型抽象**：`ChatModel` trait，内置 DashScope/Qwen Provider（OpenAI 兼容端点），支持流式与 thinking 模式
- **工具系统**：`FunctionTool`（任意 async 函数自动生成 schema）、`ToolKit` 注册表、`Skill` 集成、lenient 容错反序列化
- **记忆与 RAG**：`FileMemory` / `TurbovecMemory`、`KnowledgeBase` + `RAGMiddleware`（Static / Agentic）
- **工作空间**：`LocalWorkspace` 隔离文件系统 + 内置编码工具（Read/Write/Edit/Bash/Grep/Glob/ListDir）
- **事件驱动**：33 种 `AgentEvent` 流式事件，适合 TUI / WebSocket / SSE / 日志
- **会话持久化**：agent state 自动落盘，支持 `--resume`

## 快速体验 pi-rust 编码 Agent

创建仓库根目录 `.env` 或设置环境变量：

```bash
echo 'API_KEY=sk-your-real-dashscope-key' > .env
```

一次性发送 prompt 后退出：

```bash
cargo run -p pi-rust -- --prompt "请用一句话说明你是什么。"
```

交互式 TUI（真实 TTY 中启用；管道/CI 或 `--no-tui` 时回退 line REPL）：

```bash
cargo run -p pi-rust -- \
  --workdir .pi-rust \
  --cwd . \
  --model qwen-plus \
  --mode coding
```

完整说明见 [`examples/pi-rust/README.md`](examples/pi-rust/README.md)。

## 最简单的 Agent 示例

以 DashScope/Qwen 为模型，注册一个计算器工具，四步创建 Agent 并发起对话。

### 1. 添加依赖

```toml
[dependencies]
agent_scope_agent = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_dashscope = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_tool = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_message = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_event = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }

tokio = { version = "1", features = ["full"] }
schemars = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
```

### 2. 完整代码

```rust
use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_dashscope::DashScopeChatModel;
use agent_scope_message::factory::user_msg;
use agent_scope_tool::{FunctionTool, ToolKit};
use schemars::JsonSchema;
use serde::Deserialize;

// 工具参数类型:只需实现 Deserialize + JsonSchema,schema 自动推导
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput {
    expression: String,
}

async fn calc(input: CalcInput) -> String {
    // 用简单求值代替,真实场景可接入 eval 库
    format!("calced: {}", input.expression)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 模型(凭据由应用显式传入;也可从环境变量读)
    let api_key = std::env::var("API_KEY").expect("set API_KEY");
    let model = Arc::new(DashScopeChatModel::new(&api_key, "qwen-plus").with_stream(true));

    // 2. 工具
    let mut toolkit = ToolKit::new();
    toolkit.register(FunctionTool::new("calculator", "Evaluate a math expression", calc));

    // 3. 组装 ReActAgent
    let config = AgentConfig::builder()
        .name("assistant")
        .system_prompt("You are a helpful assistant. Use the calculator tool for math questions.")
        .model(model)
        .toolkit(toolkit)
        .build()?;
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![], // middlewares:记忆 / RAG / 自定义
    )?;

    // 4. 发起对话
    let reply = agent
        .reply(Some(vec![user_msg("user", "请用 calculator 计算 15 * 27 + 3")
            .expect("valid user message")]))
        .await?;
    println!("{}", reply.get_text_content("\n").unwrap_or_default());

    Ok(())
}
```

> 运行前先 `export API_KEY="sk-your-key"`（DashScope / 阿里云百炼平台申请）。

### 流式版本

把 `reply()` 换成 `reply_stream()` 即可逐事件消费：

```rust
use futures::StreamExt;

let mut stream = agent
    .reply_stream(Some(vec![user_msg("user", "一步步计算 (2+3)*4")
        .expect("valid user message")]))
    .await?;
while let Some(event) = stream.next().await {
    match event {
        agent_scope_event::AgentEvent::TextBlockDelta(e) => print!("{}", e.delta),
        agent_scope_event::AgentEvent::ThinkingBlockDelta(e) => eprint!("[thinking] {}", e.delta),
        agent_scope_event::AgentEvent::ToolCallStart(e) => eprintln!("tool: {}", e.tool_call_name),
        agent_scope_event::AgentEvent::ReplyEnd(e) => eprintln!("\nfinished: {:?}", e.finished_reason),
        _ => {}
    }
}
```

更多进阶用法（记忆、RAG、权限、Planner/SubAgent/Workspace）见 [`skills/agentscope-guide/SKILL.md`](skills/agentscope-guide/SKILL.md)。

## 相关资源

- TurboVec: https://github.com/RyanCodrai/turbovec
- microsandbox: https://github.com/superradcompany/microsandbox

> 开发者提示：本仓库开发时可按 `CLAUDE.md` 使用 `rtk cargo ...` 获取更紧凑的构建/测试输出；普通用户直接使用上面的裸 `cargo` 命令即可。
