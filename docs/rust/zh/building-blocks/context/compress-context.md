---
title: "压缩上下文"
description: "把上下文长度控制在模型窗口之内"
---

<Note>
**Rust 实现状态**: 已实现。`ContextConfig` + `compress_context` 在 AgentScope Rust 中可用。**注意**：当前策略为「移除最旧消息 + 占位摘要」；更复杂的模型摘要策略（调用模型生成摘要）为 deferred。
</Note>

当上下文 token 数超过阈值时，智能体会压缩上下文，使长对话保持在模型窗口之内。

## 配置

`ContextConfig` 控制压缩：

| 字段 | 说明 |
|------|------|
| `enable` | 是否启用压缩 |
| `trigger_ratio` | 触发压缩的阈值比例 |
| `reserve_ratio` | 压缩后保留的比例 |
| `tool_result_limit` | 工具结果截断限制 |

```rust
use agent_scope_agent::ContextConfig;

let context_config = ContextConfig {
    enable: true,
    trigger_ratio: 0.8,
    reserve_ratio: 0.3,
    ..Default::default()
};
```

## 行为

- 在每次推理步骤前检查上下文长度，超过阈值即触发压缩。
- 当前实现移除最旧消息并放入占位摘要，同时处理 tool-call / tool-result 配对边界，避免截断破坏工具调用生命周期。
- 被压缩移除的内容可交给卸载机制持久化（见 [卸载上下文](offload-context)）。
