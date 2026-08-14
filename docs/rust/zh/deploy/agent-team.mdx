---
title: "智能体团队"
description: "多智能体团队编排"
---

<Note>
**Rust 实现状态**: 部分支持（兼容等级 L3）。已支持：**库级多智能体委托**（`SubAgent` / `SubAgentRegistry` / `delegate_*` / `MultiAgentConversation`，`agent_scope_agent::subagent`）。尚未实现：服务级的完整 team 编排框架（leader 通过内置 `TeamCreate` / `AgentCreate` 等团队工具 spawn worker 并协调、跨会话消息路由）。兼容基线为 AgentScope Python v2.0.5。
</Note>

# 智能体团队（部分支持）

服务级的智能体团队构建在 agent-service 之上，由 leader 通过内置团队工具派生并协调 worker。AgentScope Rust 未提供服务级团队编排，但在**库级**提供多智能体委托能力，可用于构建自管理的多智能体协作。

## 库级多智能体委托

| 能力 | Rust 状态 |
|------|-----------|
| 派生 SubAgent 会话 | ✅ `SubAgent` / `SubAgentRegistry` |
| 委托任务并聚合结果 | ✅ `delegate_task` / `delegate_reasoning` 等 `delegate_*` |
| 多智能体对话 | ✅ `MultiAgentConversation` |
| leader 内置团队工具（TeamCreate/AgentCreate） | ❌ 未实现 |
| 跨会话消息路由（HintBlock team-message） | ❌ 未实现 |
| 团队成员关系与 UI 渲染 | ❌ 未实现 |

库级 SubAgent 委托的用法见 [Agent 概述](../building-blocks/agent/overview)（delegation 模块）。

## 缺失范围

- 无服务级团队编排框架：`deploy/agent-service` 为「计划中」，因此依赖服务的团队能力无从落地。
- 无内置团队工具：`agent_scope_agent` 不包含 `TeamCreate` / `AgentCreate` 等工具，需由应用自行实现。

## 替代能力

在 Rust 侧可用 `SubAgent` + `ReActAgent` 自行组装多智能体协作（leader 按需 spawn worker 并汇总结果），但消息路由与生命周期协调需应用层实现。

## 相关

- 库级基础能力见对应的 building-blocks 页面。
- 逐页状态对照见 `mirror-map.md`。
