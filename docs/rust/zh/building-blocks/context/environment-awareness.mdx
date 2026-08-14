---
title: "感知环境"
description: "让智能体感知时间、任务与上下文用量等运行时状态"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L2）。运行时状态注入（Feature 026）在 AgentScope Rust 中可用。兼容基线为 AgentScope Python v2.0.5。
</Note>

智能体通过**运行时状态注入**感知跨轮次变化的运行环境（时间、未完成任务、上下文长度），把提示注入上下文，让智能体持续保持方向感。

## 配置

通过 `InjectionConfig`（`agent_scope_agent`）配置注入的维度与格式：

| 配置 | 说明 |
|------|------|
| 时间维度 | 当前时间（支持 IANA 时区 `chrono-tz`）、时间格式 |
| 未完成任务 | 基于内置任务工具清单的未完成任务提醒 |
| 上下文长度 | 当前上下文 token 用量提示 |
| 模板与额外字段 | 自定义注入模板与额外字段 |

```rust
use agent_scope_agent::{AgentConfig, InjectionConfig};

let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .injection_config(InjectionConfig::default())  // 默认启用时间/任务/上下文三维度
    .build()?;
```

## 行为

- 在每次模型调用前，通过统一的 `_inject_runtime_state` 管线把当前状态注入为单个 `HintBlock`。
- 可配置是否发布 `HintBlockEvent` 事件（用于前端展示注入内容）。
- 未完成任务提醒在上下文较长时会提示智能体收敛未完成任务。

## 相关

- 注入内容在上下文中表现为 `HintBlock`（见 [消息与事件](../message-and-event)）。
- 任务清单由内置任务工具维护（见 [计划模式](../plan)）。
