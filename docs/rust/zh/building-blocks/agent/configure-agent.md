---
title: "配置智能体"
description: "用模型、工具与配置对象组装一个智能体"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用，采用**构造期配置**：AgentScope Rust 在构造 `ReActAgent` 时完成全部装配；配置构造后不可变，无运行时动态改模型。
</Note>

智能体在初始化时完成全部装配：通过 `AgentConfig::builder()` 传入模型、工具包、权限与各项配置，`build()` 得到 `AgentConfig`，再构造 `ReActAgent`。这样做的好处是配置一经校验即固定，避免运行期被意外改动，行为可预测、可审计。

## 最简配置

```rust
use std::sync::Arc;
use agent_scope_agent::{AgentConfig, ReActAgent, ReActConfig, ContextConfig};
use agent_scope_rig::RigChatModel;

let model = Arc::new(RigChatModel::openai(&api_key, "qwen3.7-plus")?);
let agent_config = AgentConfig::builder()
    .name("my_agent")
    .system_prompt("你是一个有帮助的助手。")
    .model(model)
    .build()?;

let agent = ReActAgent::new(agent_config, ReActConfig::default(), ContextConfig::default(), vec![])?;
```

## 常用配置项

智能体的装配分三步：先用 `AgentConfig::builder()` 链式设置各项配置，`build()` 校验并产出不可变的 `AgentConfig`；再以 `ReActConfig`（循环行为）与 `ContextConfig`（上下文窗口）组装出 `ReActAgent`。`AgentConfigBuilder` 的完整方法如下：

| 方法 | 参数类型 | 说明 | 默认值 |
|------|----------|------|--------|
| `.name(...)` | `impl Into<String>` | 智能体名称（必填，空串报错） | 无 |
| `.system_prompt(...)` | `impl Into<String>` | 系统提示词，前置注入模型上下文 | 空串 |
| `.model(model)` | `Arc<dyn ChatModel>` | 推理所用模型（必填） | 无 |
| `.toolkit(toolkit)` | `ToolKit` | 注册的工具包（可选） | `None` |
| `.permission_context(ctx)` | `PermissionContext` | 权限上下文（模式 + 规则，见下文） | `PermissionContext::default()` |
| `.permission_mode(mode)` | `PermissionMode` | 仅设置权限模式，保留已有规则 | 保留默认 |
| `.task_tools_enabled(bool)` | `bool` | 是否注册内置任务规划工具并启用未完成任务注入 | `true` |
| `.session_store(store)` | `Arc<dyn SessionStore>` | 状态持久化后端；未设置时用 `sessions/` 下的 `JsonFileSessionStore` | `None` |
| `.session_id(id)` | `impl Into<String>` | 会话 ID，用于在进程间恢复状态 | `None`（自动生成） |
| `.auto_persist(bool)` | `bool` | 每次回复结束后自动持久化状态 | `true` |
| `.injection_config(cfg)` | `InjectionConfig` | 运行时状态注入配置（时间/任务/上下文长度） | `InjectionConfig::default()` |
| `.workspace(ws)` | `Arc<dyn WorkspaceBase>` | 绑定工作空间，自动注入内置工具 | `None` |
| `.workspace_tools_enabled(bool)` | `bool` | 绑定工作空间时是否注入内置工具 | `true` |
| `.with_stream_channel_capacity(cap)` | `Option<usize>` | 流式事件通道容量；`None`=无界（默认）、`Some(n)`=有界；`Some(0)` 会 panic | `None` |
| `.build()` | — | 校验并产出 `AgentConfig`（返回 `Result`） | — |

### AgentConfig 字段

`build()` 产出的 `AgentConfig` 是公开字段结构体，也可直接构造（但通常走 builder）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `String` | 智能体标识，用于消息与事件 |
| `system_prompt` | `String` | 系统提示词 |
| `model` | `Arc<dyn ChatModel>` | 推理模型 |
| `toolkit` | `Option<ToolKit>` | 注册的工具 |
| `stream_channel_capacity` | `Option<usize>` | 流式通道容量 |
| `permission_context` | `PermissionContext` | 权限上下文 |
| `task_tools_enabled` | `bool` | 是否启用内置任务工具 |
| `session_store` | `Option<Arc<dyn SessionStore>>` | 会话存储后端 |
| `session_id` | `Option<String>` | 会话 ID |
| `auto_persist` | `bool` | 是否自动持久化 |
| `injection_config` | `InjectionConfig` | 运行时注入配置 |
| `workspace` | `Option<Arc<dyn WorkspaceBase>>` | 绑定的工作空间 |
| `workspace_tools_enabled` | `bool` | 是否注入工作空间内置工具 |

### 循环与上下文配置

`ReActConfig` 控制推理-行动循环的行为：

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `max_iters` | `u32` | `20` | 单次回复的最大推理-行动迭代数 |
| `stop_on_reject` | `bool` | `false` | 权限拒绝时是否停止（而非等待确认） |
| `interruption_message` | `String` | `"The execution was interrupted."` | 回复被中断时返回的消息 |
| `structured_output_grace_iters` | `u32` | `3` | 结构化输出解析失败时额外允许的迭代数 |

`ContextConfig` 控制上下文窗口管理：

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `enable` | `bool` | `false` | 是否启用上下文压缩 |
| `trigger_ratio` | `f64` | `0.8` | 触发压缩的上下文占比阈值（0~1） |
| `reserve_ratio` | `f64` | `0.1` | 为模型回复预留的上下文占比 |
| `compression_prompt` | `String` | `"<STD_CP_PROMPT>"` | 压缩所用的系统提示词 |
| `tool_result_limit` | `usize` | `4096` | 工具结果内容的截断上限（字符） |

### 权限配置

权限系统决定每个工具调用是否允许、拒绝或需要确认（详见 [权限系统](../permission-system/overview)）。`PermissionMode` 有五种模式：

| 模式 | 说明 |
|------|------|
| `Default` | 按显式规则询问，否则放行（Rust 默认） |
| `AcceptEdits` | 默认允许编辑，显式 deny/ask 规则仍生效 |
| `Explore` | 只读规划模式，未分类调用默认拒绝 |
| `Bypass` | 完全信任模式，显式 deny/ask 规则仍生效 |
| `DontAsk` | 无人值守模式，任何 ask 决定转为 deny |

规则用 `PermissionRule::allow(pattern)` / `deny(pattern)` / `ask(pattern)` 构造，可链式 `.with_rule_content(...)`（匹配工具输入子串）与 `.with_source(...)`（标注来源），再通过 `PermissionContext::new(mode)` + `add_rule(rule)` 组装。

### 运行时状态注入

`InjectionConfig` 控制在每次迭代时向上下文注入时间、未完成任务与上下文长度信息：

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `inject_runtime_state` | `bool` | `true` | 总开关，`false` 时不做任何注入 |
| `timezone` | `String` | `"UTC"` | 注入时间的时区（无法解析时回退 UTC） |
| `time_format` | `String` | `"%Y-%m-%dT%H:%M:%S"` | 注入时间的格式（须能携带日期部分） |
| `time_interval` | `f64` | `0.5` | 距上次注入的最小间隔（小时） |
| `context_buffer_ratio` | `f64` | `0.2` | 早于压缩阈值的上下文长度注入缓冲（0~1） |
| `template` | `String` | 内置模板 | 注入包装模板，须含 `{runtime_state}` 占位符 |
| `task_tool_names` | `Vec<String>` | 四个任务工具名 | 出现这些工具调用时抑制任务注入 |
| `emit_hint_event` | `bool` | `true` | 注入发生时是否发出 `HintBlockEvent` |

<Note>
**内置任务工具输出协议（Feature 033）**：4 个内置任务工具（`TaskCreate` / `TaskList` / `TaskGet` / `TaskUpdate`）的成功结果文本以换行符 `\n` 结尾；`TaskUpdate` 报告实际应用的字段值（如 `Updated task (id=1): status=in_progress; add_blocked_by=[4]`）；`TaskGet` 对超过 200 字符的描述截断为 `{前 200 字符}… (truncated, {len} chars total)`。工具名、输入 Schema 与行为语义均不变。
</Note>

## 完整示例

见 [`examples/agent`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/agent/)，演示模型 + 工具 + 权限规则 + 中断的组装。
