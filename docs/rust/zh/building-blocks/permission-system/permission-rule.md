---
title: "权限规则"
description: "为特定工具与调用编写允许、拒绝和询问规则"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L2）。`PermissionRule` 在 AgentScope Rust 中可用。兼容基线为 AgentScope Python v2.0.5。
</Note>

`PermissionRule` 把某个工具与具体的调用模式映射到三种行为之一：`Allow`、`Deny`、`Ask`。规则在每种[权限模式](permission-mode)下都以最高优先级评估。

## 构造规则

`PermissionRule` 提供三个构造方法，每个接收一个**匹配模式**：

| 方法 | 行为 | 说明 |
|------|------|------|
| `PermissionRule::allow(pattern)` | 允许 | 匹配该模式的调用直接执行 |
| `PermissionRule::deny(pattern)` | 拒绝 | 匹配该模式的调用被拒绝，错误结果喂给模型 |
| `PermissionRule::ask(pattern)` | 询问 | 匹配该模式的调用进入确认（`RequireConfirm`） |

```rust
use agent_scope_agent::{PermissionContext, PermissionRule};

let mut perm = PermissionContext::default();
perm.add_rule(PermissionRule::allow("Read*"));
perm.add_rule(PermissionRule::allow("Glob"));
perm.add_rule(PermissionRule::deny("Bash*"));
perm.add_rule(PermissionRule::ask("Write*"));
```

## 规则语义

- `pattern` 是通配匹配模式（工具名 / 前缀模式），用于对工具名进行通配匹配。
- 规则在 `PermissionContext` 中按加入顺序评估，命中最优先。
- 未命中任何规则时，由模式决定兜底行为（见 [权限模式](permission-mode)）。

## 完整示例

见 [`examples/agent`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/agent/)，演示 allow 规则 + 流式确认事件。
