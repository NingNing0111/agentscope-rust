---
title: "卸载上下文"
description: "把被移除的内容持久化到外部存储，细节仍可找回"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L2）。上下文卸载在 AgentScope Rust 中可用（`WorkspaceBase::offload_context` / `offload_tool_result`）。兼容基线为 AgentScope Python v2.0.5。
</Note>

当上下文被压缩或工具结果被截断时，卸载机制把被移除的内容持久化到工作空间的外部存储，使细节在需要时仍可找回——只需一次文件读取。

## 卸载接口

`WorkspaceBase`（`agent_scope_workspace`）提供卸载能力：

| 方法 | 说明 |
|------|------|
| `offload_context(...)` | 卸载被压缩的上下文内容 |
| `offload_tool_result(...)` | 卸载被截断的过大工具结果 |

实现这些方法的 workspace（如 `LocalWorkspace`）会把内容写入工作空间内文件，并返回可检索的位置/摘要。

## 与压缩协作

压缩（[压缩上下文](compress-context)）决定「移除什么」，卸载决定「把移除的内容存到哪里」。两者配合，长对话既保持在模型窗口内，又不丢失早期细节。

## 边界

卸载是工作空间能力的一部分，因此**绑定工作空间**的 agent 才能使用卸载；未绑定 workspace 时仅依赖压缩的占位摘要。
