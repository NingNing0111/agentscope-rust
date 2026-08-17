---
title: "权限模式"
description: "根据智能体的部署方式选择合适的全局策略"
---

<Note>
**Rust 实现状态**: 已实现。五种 `PermissionMode` 在 AgentScope Rust 中可用。
</Note>

权限模式是每次工具调用决策背后的全局策略：它决定哪些决策点生效，以及没有任何规则命中时的兜底行为。AgentScope Rust 支持五种模式：

| 模式（Rust 变体） | 未命中规则时的兜底行为 | 说明 |
|-------------------|------------------------|------|
| `Default` | 允许 | 默认模式。显式 deny/ask 规则仍生效 |
| `AcceptEdits` | 允许 | 语义上默认接受编辑类操作；显式 deny/ask 规则仍生效 |
| `Explore` | 拒绝 | 只读规划模式，只放行命中 allow 规则的调用 |
| `Bypass` | 允许 | 完全信任模式；显式 deny/ask 规则仍生效 |
| `DontAsk` | 允许 | 无人值守模式；ask 决策转为 deny，永不返回确认 |

> 无论哪种模式，规则的优先级都固定不变：`deny` > `ask` > `allow`。模式只决定「无规则命中、且不是内置任务工具」时的兜底行为。`DontAsk` 额外把命中的 `ask` 规则就地转为拒绝。

## 设置模式

在构造权限上下文时设置：

```rust
use agent_scope_agent::{PermissionContext, PermissionMode, PermissionRule};

// DEFAULT：未命中规则时允许
let mut perm = PermissionContext::new(PermissionMode::Default);
perm.add_rule(PermissionRule::allow("Read*"));

// EXPLORE：只读规划，未命中规则拒绝
let explore = PermissionContext::new(PermissionMode::Explore);

// BYPASS：完全信任（deny/ask 规则仍生效）
let bypass = PermissionContext::new(PermissionMode::Bypass);
```

## 完整示例

见 [`examples/agent`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/agent/)，演示 `PermissionMode::Default` + allow 规则的组装。
