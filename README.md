# AgentScope Rust（Beta阶段，生产暂时不建议使用）

用 Rust 实现的 **Agent 开发框架**（AgentScope 的 Rust 重构版）。以多 crate workspace 组织，核心能力位于 `crates/agent_scope_*`，每个 crate 是独立 package，可按需单独依赖。

## 文档

- [AgentScope Rust 中文文档](https://ningning0111.github.io/agentscope-rust/) — 中文文档站（VitePress，随 `master` 自动发布到 GitHub Pages）
- [本地文档维护说明](docs/rust/README.md) — 文档结构、状态标注与本地构建/检查命令

## 特性一览

- **Agent 编排**：`Agent` trait、`ReActAgent`（reasoning→acting 循环）、`Middleware` 管道、权限控制、`Planner`、`SubAgent`
- **模型抽象**：`ChatModel` trait，内置 rig-backed Provider（OpenAI / Anthropic / DeepSeek），支持流式、工具调用与 thinking 模式
- **工具系统**：`FunctionTool`（任意 async 函数自动生成 schema）、`ToolKit` 注册表、`Skill` 集成、lenient 容错反序列化
- **记忆与 RAG**：`FileMemory` / `TurbovecMemory`、`KnowledgeBase` + `RAGMiddleware`（Static / Agentic）
- **工作空间**：`LocalWorkspace` 隔离文件系统 + 内置编码工具（Read/Write/Edit/Bash/Grep/Glob/ListDir）
- **事件驱动**：33 种 `AgentEvent` 流式事件，适合 TUI / WebSocket / SSE / 日志
- **会话持久化**：agent state 自动落盘，支持 `--resume`

## 配置凭据

创建仓库根目录 `.env` 或设置环境变量：

```bash
cp .env.example .env   # 变量名见 .env.example（DEFAULT_API_KEY / DEFAULT_CHAT_MODEL / DEFAULT_URL）
# 然后编辑 .env，把 DEFAULT_API_KEY=sk-xxx 换成你的真实 Key
```

各示例从这些变量构建模型（模型名 `DEFAULT_CHAT_MODEL`、端点 `DEFAULT_URL` 均可覆盖）。最简示例见下方「最简单的 Agent 示例」。

## 最简单的 Agent 示例

以 rig-backed OpenAI 模型为默认（支持 Anthropic/DeepSeek），注册一个计算器工具，四步创建 Agent 并发起对话。

### 1. 添加依赖

```toml
[dependencies]
agent_scope_agent = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_rig = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
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
use agent_scope_message::factory::user_msg;
use agent_scope_rig::RigChatModel;
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
    // 1. 模型(凭据与模型名从环境变量/.env 读取,变量名见 .env.example)
    let api_key = std::env::var("DEFAULT_API_KEY").expect("set DEFAULT_API_KEY");
    let model_name =
        std::env::var("DEFAULT_CHAT_MODEL").unwrap_or_else(|_| "qwen3.7-plus".to_string());
    let mut model = RigChatModel::openai(&api_key, &model_name)?;
    if let Ok(base_url) = std::env::var("DEFAULT_URL") {
        model = model.with_base_url(base_url);
    }
    let model = Arc::new(model.with_stream(true));

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

> 运行前先 `export DEFAULT_API_KEY="sk-xxx"`（默认 `DEFAULT_URL` 指向 DashScope 百炼兼容端点，完整变量见 `.env.example`；也可用 Anthropic/DeepSeek 构造器，见 [`docs/rust/zh/building-blocks/model/llm.md`](docs/rust/zh/building-blocks/model/llm.md)）。

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
