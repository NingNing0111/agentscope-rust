---
title: "控制台"
description: "在终端快速测试和验证智能体行为（计划中）"
---

<Note>
**Rust 实现状态**: 计划中。库级 `console` 模块（`launch_console` 交互式聊天 + `ConsoleRenderer` 事件渲染）在 AgentScope Rust 中尚未实现，预计在后续版本推出。
</Note>

库级控制台模块让你在终端直接与智能体对话并查看完整事件流，无需启动 web 服务或手写事件分发。

## Rust 侧现状

- 库级 `console` 模块未实现。
- 终端交互参考实现 `examples/pi-rust`（ratatui TUI + line REPL）已随示例体系重构移除。如需终端交互，可基于 `examples/chat`（流式对话 + 事件分发）与 `AgentEvent` 事件流自行构建。

## 缺失范围

| 能力 | Rust 现状 |
|------|-----------|
| 库级 console 模块（`launch_console` / `ConsoleRenderer`） | 未实现 |
| 终端交互 | 无仓库内置示例；需基于事件流自行构建 |
