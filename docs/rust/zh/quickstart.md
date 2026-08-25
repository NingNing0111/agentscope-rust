---
title: "快速开始"
description: "快速上手 AgentScope Rust"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用。
</Note>

## 环境准备

AgentScope Rust 需要 Rust 工具链（edition 2024，stable 即可）。同时需要一个模型服务的 API Key（当前内置 provider 为 rig：OpenAI / Anthropic / DeepSeek，示例默认 OpenAI）。

<Tip>
如果你只是想确认项目能编译，可先跳过凭据配置，直接运行下面的示例命令。未设置 `DEFAULT_API_KEY` 时，程序会输出明确错误并退出，不会产生隐藏的网络调用或 panic。
</Tip>

## 最短路径

1. 确认在仓库根目录，并安装依赖：

```bash
npm install
cargo --version
```

2. 设置模型凭据：

```bash
export DEFAULT_API_KEY="sk-your-key"
```

3. 运行 quickstart 示例：

```bash
cargo run -p quickstart -- --prompt "你好，请用一句话介绍你自己。"
```

### 引入依赖

在项目的 `Cargo.toml` 中加入所需 crate（各 crate 都是独立 package，可按需引入）。外部项目请使用 GitHub git 依赖，不要复制仓库内部的 `path` 依赖：

```toml
[dependencies]
agent_scope_agent = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_rig = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_tool = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_message = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_event = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
tokio = { version = "1", features = ["full"] }
```

如果是在 `agentscope-rust` 仓库本身开发，可直接运行文档对应的示例 crate（见下文）。

### 配置凭据

设置环境变量（或将 `DEFAULT_API_KEY` 等变量写入仓库根目录 `.env`，变量名见仓库根目录 `.env.example`；示例会通过 dotenv 自动加载）：

```bash
export DEFAULT_API_KEY="sk-your-key"
```

可选变量：

| 变量 | 作用 |
| --- | --- |
| `DEFAULT_API_KEY` | 必填。模型服务 API Key。 |
| `DEFAULT_CHAT_MODEL` | 可选。聊天模型名，未设置时使用示例默认值。 |
| `DEFAULT_URL` | 可选。覆盖 OpenAI 兼容端点。 |

## 第一个智能体

完整可运行的示例位于 [`examples/quickstart`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/quickstart/)，运行方式：

```bash
cargo run -p quickstart -- --prompt "你好，请用一句话介绍你自己。"
```

示例构建了一个最简智能体：一个 OpenAI 凭据、对应的聊天模型、一个空工具集，以及一个 `ReActAgent`。智能体提供两个调用入口 —— `reply` 返回最终消息，`reply_stream` 以流式方式逐步产出事件，适合展示推理和工具调用的中间过程。

```rust
use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_rig::RigChatModel;
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use agent_scope_tool::ToolKit;
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 凭据 —— 从环境变量读取（变量名见 .env.example）。
    let api_key = std::env::var("DEFAULT_API_KEY")
        .map_err(|_| anyhow::anyhow!("error: 缺少环境变量 DEFAULT_API_KEY"))?;

    // 2. 聊天模型 —— 模型名从 DEFAULT_CHAT_MODEL 读取（fallback qwen3.7-plus）；
    //    DEFAULT_URL 可选覆盖端点（默认指向百炼 DashScope 的 OpenAI 兼容端点）。
    let model_name =
        std::env::var("DEFAULT_CHAT_MODEL").unwrap_or_else(|_| "qwen3.7-plus".to_string());
    let mut model = RigChatModel::openai(&api_key, &model_name)?;
    if let Ok(base_url) = std::env::var("DEFAULT_URL") {
        model = model.with_base_url(base_url);
    }
    let model = Arc::new(model.with_stream(true));

    // 3. 工具集 —— 这里为空；注册 Bash/Read/Write 等即可启用工具调用。
    let toolkit = ToolKit::new();

    // 4. 组装 ReActAgent。
    let agent_config = AgentConfig::builder()
        .name("assistant")
        .system_prompt("你是一个乐于助人的助手。")
        .model(model)
        .toolkit(toolkit)
        .build()?;

    let agent = ReActAgent::new(
        agent_config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?;

    let msg = user_msg("user", "你好，请用一句话介绍你自己。")
        .map_err(|e| anyhow::anyhow!("无效的 user 消息: {e:?}"))?;

    // 方式一：等待最终的助手消息。
    let reply = agent.reply(Some(vec![msg.clone()])).await?;
    if let Some(text) = reply.get_text_content("") {
        println!("assistant: {text}");
    }

    // 方式二：流式获取增量事件（文本片段、工具调用等）。
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;
    while let Some(event) = stream.next().await {
        match event {
            AgentEvent::TextBlockDelta(delta) => print!("{}", delta.delta),
            AgentEvent::ToolCallStart(start) => {
                println!("\n[tool call] {} ({})", start.tool_call_name, start.tool_call_id);
            }
            AgentEvent::ReplyEnd(_) => println!("\n[reply end]"),
            _ => {}
        }
    }

    Ok(())
}
```

<Tip>
运行前在环境变量中设置 `DEFAULT_API_KEY`。当前内置模型 provider 为 rig-backed（`agent_scope_rig`），支持 OpenAI / Anthropic / DeepSeek 三套后端（`RigChatModel::openai/anthropic/deepseek`）；模型能力与支持范围见 [模型概览](/building-blocks/model/overview)。
</Tip>

### 预期输出

有凭据时，程序先打印 `reply()` 的最终助手消息，再以 `reply_stream()` 流式输出文本增量并以 `[reply end]` 结束。未设置 `DEFAULT_API_KEY` 时，程序输出明确的缺凭据错误并退出（不会静默失败或 panic）。

### 常见问题

| 现象 | 处理方式 |
| --- | --- |
| `缺少环境变量 DEFAULT_API_KEY` | 设置环境变量，或在仓库根目录 `.env` 中写入同名变量。 |
| 模型端点不可达 | 检查 `DEFAULT_URL` 是否指向 OpenAI 兼容接口，或移除该变量使用默认端点。 |
| 输出中没有工具调用 | quickstart 示例使用空 `ToolKit`，这是预期行为；接入工具见 [工具系统](/building-blocks/tool/overview)。 |

## 按需使用其他能力

AgentScope Rust 按能力分 crate，按需引入即可：

- **工具系统**：`agent_scope_tool`（FunctionTool、ToolKit、内置工具、Skill）
- **记忆**：`agent_scope_memory`（FileMemory、TurbovecMemory）
- **RAG**：`agent_scope_rag`（KnowledgeBase、RAGMiddleware）
- **工作空间**：`agent_scope_workspace`（LocalWorkspace）
- **沙箱**：`agent_scope_sandbox`（LocalSandboxSession）
- **MCP**：`agent_scope_mcp`（McpClient、McpTool）

更多用法见各模块文档（`building-blocks/` 目录）。
