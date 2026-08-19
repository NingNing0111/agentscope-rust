---
title: "常见问题"
description: "关于 AgentScope Rust 的常见问题"
---

<Note>
**Rust 实现状态**: 已实现。面向 AgentScope Rust 的 FAQ。
</Note>

<AccordionGroup>

  <Accordion title="AgentScope Rust 是什么？">
    AgentScope Rust（`agent_scope_*` crates）是 AgentScope 的 Rust 重构版，以 Rust 多 crate workspace 组织，各能力真实可用性以每页顶部「Rust 实现状态」标注为准（已实现 / 部分支持 / 计划中）。
  </Accordion>

  <Accordion title="支持哪些模型提供商？">
    当前内置 provider 为 **rig-backed**（`agent_scope_rig`），支持 OpenAI / Anthropic / DeepSeek 三套后端（`RigChatModel`、`RigEmbeddingModel`，示例默认 OpenAI）。`ChatModel` / `EmbeddingModel` trait 支持接入自定义 provider。
  </Accordion>

  <Accordion title="支持 TTS / 语音合成吗？">
    不支持。Rust 尚无 TTS 类型；消息层具备音频数据块能力（`DataBlock`）但无 TTS 生成。见 [TTS](../building-blocks/model/tts)。
  </Accordion>

  <Accordion title="支持沙箱隔离执行吗？">
    Rust 提供本地隔离沙箱（`LocalSandboxSession`：本地进程 + 临时根目录 + 路径越界防护 + 命令超时），**非 Docker**；cpu/memory 资源限制在本地后端不可强制。见 [工作空间](../building-blocks/workspace/overview)。
  </Accordion>

  <Accordion title="如何接入 MCP 服务？">
    Rust 提供 MCP **客户端**接入（`McpClient` / `McpTool` / `McpExt`，基于官方 SDK `rmcp`），可连接任意 stdio/HTTP MCP server 并接入其工具。尚无服务端 gateway。见 [MCP](../building-blocks/tool/mcp)。
  </Accordion>

  <Accordion title="有控制台 / 前端吗？">
    库级 console 模块未实现；终端交互参考实现已随示例体系重构移除。见 [控制台](../building-blocks/console)。
  </Accordion>

  <Accordion title="如何快速上手？">
    按 [快速开始](../quickstart) 操作：设置 `DEFAULT_API_KEY`，运行 `examples/quickstart`，30 分钟跑起第一个对话 Agent。
  </Accordion>

  <Accordion title="需要启用什么 feature 开关？">
    默认构建不需要 feature。多格式文档解析（PDF / Office / Excel / HTML）需要启用 `agent_scope_rag` 的 `xberg` feature，例如 `agent_scope_rag = { version = "...", features = ["xberg"] }`。其余能力按需引入对应 `agent_scope_*` crate 即可。
  </Accordion>

</AccordionGroup>
