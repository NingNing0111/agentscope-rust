---
title: "概述"
description: "规则、模式与工具检查如何共同决定每一次工具调用"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的权限引擎（`PermissionEngine` / `PermissionMode` / `PermissionRule` / `PermissionDecision` / `PermissionResult`）在 AgentScope Rust 中可用。
</Note>

权限系统拦截智能体发起的每一次工具调用，并给出三种决策之一：**允许**（Allow）执行、**拒绝**（Deny）执行，或**询问用户**（RequireConfirm）确认。智能体不能自己越过这个裁决——即使模型产生了工具调用意图，也必须先经过权限引擎判定，得到 `Allow` 才会真正执行。

决策由三个组件共同驱动（`agent_scope_agent::permission`）：

| 组件 | 作用 |
|------|------|
| **权限规则**（`PermissionRule`） | 针对特定工具与调用的显式允许/拒绝/询问规则，以最高优先级评估 |
| **权限模式**（`PermissionMode`） | 全局策略，决定没有任何规则命中时的兜底行为 |
| **工具检查** | 工具自身在运行时基于真实输入做动态分析（只读判定、危险路径保护） |

## 决策流程

每一次工具调用按固定优先级走下面的决策点，命中的第一个即生效：

| 优先级 | 决策点 | 结果 |
|--------|--------|------|
| 1 | `deny` 规则匹配 | 拒绝，最高优先级 |
| 2 | `ask` 规则匹配 | 询问（`DontAsk` 模式下转为拒绝） |
| 3 | `allow` 规则匹配 | 允许 |
| 4 | 内置任务工具 | 自动放行（显式 deny 规则仍优先） |
| 5 | 模式兜底 | `Explore` 拒绝；其余模式允许 |

最终产生 `PermissionResult`：`Allow` / `Deny { reason }` / `RequireConfirm`。`RequireConfirm` 在事件流中体现为 `RequireUserConfirmEvent`：引擎发出该事件后**暂停** reply_stream（不喂 denied、无 ReplyEnd），宿主收集确认后以 `UserConfirmResultEvent` 通过 `reply_stream_event` **恢复同一 agent**（见 [人机交互](../agent/human-in-the-loop)）。

## 核心类型

| 类型 | 说明 |
|------|------|
| `PermissionContext` | 权限上下文，持有模式与规则集合。`new(mode)` 创建，`add_rule(rule)` 添加规则 |
| `PermissionRule` | 单条规则。`allow(pattern)` / `deny(pattern)` / `ask(pattern)` 构造 |
| `PermissionMode` | 五种全局策略（见 [权限模式](permission-mode)） |
| `PermissionEngine` | 决策引擎。`check(tool_name, input)` 返回 `PermissionResult`，`check_decision(...)` 返回更详细的 `PermissionDecision` |
| `PermissionDecision` | 详细决策结果，含 `behavior` / `message` / `decision_reason` / `suggested_rules` 等字段 |
| `PermissionResult` | 精简结果：`Allow` / `Deny { reason }` / `RequireConfirm` |

## 使用

```rust
use agent_scope_agent::{PermissionContext, PermissionMode, PermissionRule};

let mut perm = PermissionContext::new(PermissionMode::Default);
// allow/deny/ask 各接收一个匹配模式（通配符；规则语义见 permission-rule 页）。
perm.add_rule(PermissionRule::allow("Read*"));

// 在 AgentConfig 中注入权限上下文
let config = agent_scope_agent::AgentConfig::builder()
    .permission_context(perm)
    // ...其他配置
    .build()?;
```

## 下一步

<CardGroup :cols="2">
  <Card title="权限模式" icon="toggle" href="/building-blocks/permission-system/permission-mode">
    五种全局策略的适用场景。
  </Card>
  <Card title="权限规则" icon="rule" href="/building-blocks/permission-system/permission-rule">
    为特定工具与调用编写 allow/deny/ask 规则。
  </Card>
  <Card title="工具内置检查" icon="shield" href="/building-blocks/permission-system/tool-check">
    工具运行时的只读判定与危险路径保护。
  </Card>
</CardGroup>
