---
title: "概述"
description: "管理智能体的工作记忆，让长任务稳定推进"
---

<Note>
**Rust 实现状态**: 部分支持。已支持：上下文压缩（`ContextConfig`）、上下文卸载（`offload_context` / `offload_tool_result`）、环境感知（运行时状态注入，Feature 026）。尚未实现：更复杂的模型摘要压缩策略（当前为移除最旧消息 + 占位摘要）。
</Note>

上下文是智能体的工作记忆：大模型在每一步推理时看到的全部消息（用户输入、智能体回复、工具调用、工具结果）。上下文管理包含三种机制：

| 机制 | 作用 | 页面 |
|------|------|------|
| **上下文注入** | 把随对话变化的运行时状态（时间、任务、上下文用量）注入上下文 | [感知环境](environment-awareness) |
| **上下文压缩** | 把较早的消息移除/摘要，让长对话保持在模型窗口之内 | [压缩上下文](compress-context) |
| **上下文卸载** | 把被移除的内容持久化到外部存储，细节仍可找回 | [卸载上下文](offload-context) |

三种机制相互配合：注入补充模型当下需要知道的信息，压缩移除不再需要逐字保留的内容，卸载让被移除的内容只需一次文件读取即可找回。

## 组装上下文

每次模型调用前，智能体把系统提示词、摘要（如有压缩）与当前上下文拼成单次 API 输入。压缩与注入都发生在模型调用之前（`react_loop` / `streaming_reactor` 中）。

## 配置

`ContextConfig` 控制压缩与注入的开关与阈值：

```rust
use agent_scope_agent::ContextConfig;

let context_config = ContextConfig {
    enable: true,
    ..Default::default()
};
```
