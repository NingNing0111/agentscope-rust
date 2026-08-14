---
title: "权限模式"
description: "根据智能体的部署方式选择合适的全局策略"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L2）。五种 `PermissionMode` 在 AgentScope Rust 中可用。兼容基线为 AgentScope Python v2.0.5。
</Note>

权限模式是每次工具调用决策背后的全局策略：它决定哪些决策点生效，以及无人裁决的调用如何收尾。AgentScope Rust 支持五种模式：

| 模式（Rust 变体） | 行为 | 适用场景 |
|-------------------|------|----------|
| `Default` | 默认：未命中规则时允许（MVP 行为以规则为准） | 最安全，推荐默认值 |
| `AcceptEdits` | 默认允许编辑操作；显式 deny/ask 规则仍生效 | 用户在场的活跃开发 |
| `Explore` | 只读规划模式：未命中规则时拒绝调用 | 代码探索、规划 |
| `Bypass` | 完全信任：显式 deny/ask 规则仍生效 | 沙箱环境或完全可信的运行 |
| `DontAsk` | 无人值守：任何 ask 决策转为 deny，永不返回 ASK | 无人值守 / 计划任务 |

> 注意：Rust 版的模式行为以「未命中规则时允许/拒绝」为语义核心；更细粒度的「只读命令自动放行」「工作目录内编辑自动放行」「危险路径安全 ASK」等内建检查为部分覆盖（见 [工具内置检查](tool-check)）。

## 设置模式

在构造权限上下文时设置：

```rust
use agent_scope_agent::{PermissionContext, PermissionMode, PermissionRule};

// DEFAULT：显式规则裁决
let mut perm = PermissionContext::new(PermissionMode::Default);
perm.add_rule(PermissionRule::allow("Read*"));

// EXPLORE：只读规划，未命中规则拒绝
let explore = PermissionContext::new(PermissionMode::Explore);

// BYPASS：完全信任（deny/ask 规则仍生效）
let bypass = PermissionContext::new(PermissionMode::Bypass);
```

## 完整示例

见 [`examples/agent`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/agent/)，演示 `PermissionMode::Default` + allow 规则的组装。
