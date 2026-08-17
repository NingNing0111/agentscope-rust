---
title: "配置智能体"
description: "用模型、工具与配置对象组装一个智能体"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L3）——**构造期配置**。AgentScope Rust 在构造 `ReActAgent` 时完成全部装配；配置构造后不可变，无运行时动态改模型。兼容基线为 AgentScope Python v2.0.5。
</Note>

智能体在初始化时完成全部装配：通过 `AgentConfig::builder()` 传入模型、工具包、权限与各项配置，`build()` 得到 `AgentConfig`，再构造 `ReActAgent`。

## 最简配置

```rust
use std::sync::Arc;
use agent_scope_agent::{AgentConfig, ReActAgent, ReActConfig, ContextConfig};
use agent_scope_dashscope::DashScopeChatModel;

let model = Arc::new(DashScopeChatModel::new(&api_key, "qwen-plus"));
let agent_config = AgentConfig::builder()
    .name("my_agent")
    .system_prompt("你是一个有帮助的助手。")
    .model(model)
    .build()?;

let agent = ReActAgent::new(agent_config, ReActConfig::default(), ContextConfig::default(), vec![])?;
```

## 常用配置项

`AgentConfigBuilder` 支持以下关键配置：

| 方法 | 说明 |
|------|------|
| `.name(...)` | 智能体名称 |
| `.system_prompt(...)` | 系统提示词 |
| `.model(model)` | `Arc<dyn ChatModel>` |
| `.toolkit(toolkit)` | `ToolKit`（含内置任务工具自动注册） |
| `.permission_context(ctx)` | 权限上下文（模式 + 规则） |
| `.workspace(ws)` | 绑定 `Arc<dyn WorkspaceBase>`（自动注入内置工具） |
| `.session_id(id)` | 会话 ID（用于状态恢复） |
| `.auto_persist(bool)` | 回复后自动持久化会话状态 |
| `.injection_config(cfg)` | 运行时状态注入配置（时间/任务/上下文长度） |
| `.task_tools_enabled(bool)` | 是否注册内置任务规划工具（默认 `true`） |

<Note>
**内置任务工具输出协议（Feature 033）**：4 个内置任务工具（`TaskCreate` / `TaskList` / `TaskGet` / `TaskUpdate`）的成功结果文本以换行符 `\n` 结尾；`TaskUpdate` 报告实际应用的字段值（如 `Updated task (id=1): status=in_progress; add_blocked_by=[4]`）；`TaskGet` 对超过 200 字符的描述截断为 `{前 200 字符}… (truncated, {len} chars total)`。工具名、输入 Schema 与行为语义均不变。
</Note>

## 完整示例

见 [`examples/agent`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/agent/)，演示模型 + 工具 + 权限规则 + 中断的组装。
