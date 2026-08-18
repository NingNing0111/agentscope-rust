---
title: "概述"
description: "理解模型输入、上下文管理配置与子智能体共享边界"
---

<Note>
**Rust 实现状态**：ReAct 对话上下文、基于阈值的自动压缩、运行时状态注入、Workspace 显式卸载和 SubAgent 上下文策略均已实现。自动压缩目前只会移除旧消息并写入占位摘要；它不会自动调用 Workspace 卸载，也不会生成语义摘要。
</Note>

Context（上下文）不是单一配置项，而是智能体在一次模型调用中可见的信息，以及围绕这些信息执行的预算、注入、压缩、卸载和授权规则。

## 建立心智模型

建议把 Context 分成五层理解：

| 层次 | 负责什么 | 主要 API |
|------|----------|----------|
| **对话 Context** | 当前会话中的用户消息、智能体回复、工具调用、工具结果和 Hint | `AgentState::context`、`Msg` |
| **窗口管理** | 计算 token 预算，按阈值压缩旧消息 | `ContextConfig` |
| **运行时注入** | 注入时间、未完成任务、上下文用量和自定义字段 | `InjectionConfig` |
| **外部卸载** | 把消息或大型工具结果显式写入 Workspace，保留可读取路径 | `WorkspaceBase::offload_context`、`WorkspaceBase::offload_tool_result` |
| **SubAgent 共享** | 决定父智能体向子智能体传递哪些消息、引用和能力 | `ContextSharingPolicy`、`CapabilityScope` |

这些层次彼此相关，但不要混为一谈：

- `ContextConfig` 只管理 ReAct 模型窗口和自动压缩，不控制运行时 Hint。
- `InjectionConfig` 属于 `AgentConfig`，只控制运行时状态注入。
- Workspace offload 是显式持久化 API；当前自动压缩不会顺带卸载被移除的消息。
- SubAgent 不会自然继承父智能体的完整上下文或资源；共享内容和实际能力分别受策略约束。

## 一次模型调用看见什么

ReAct 循环构造模型输入时，信息来源包括：

1. **system prompt**：`AgentConfig::system_prompt`，放在调用消息最前面；
2. **tool schemas**：当前 `ToolKit` 暴露给模型的工具定义；
3. **历史消息**：`AgentState::context` 中保留的消息；
4. **摘要**：自动压缩后，当前实现会在上下文开头插入一条占位摘要消息；手动 `trim_context` 则把裁剪内容累积到 `AgentState::summary`，是否发送给模型由调用方负责；
5. **运行时注入**：符合条件时追加到上下文的单个 `HintBlock`；
6. **中间件调整**：`pre_reasoning` 中间件还可以在调用前修改消息和 tool schemas。

自动路径的关键顺序是：

1. 用「当前历史消息 + system prompt + tool schemas」估算输入 token；
2. 超过阈值且启用压缩时，先压缩持久状态中的旧消息；
3. 基于压缩后的状态重新判断并追加运行时注入；
4. 再从状态读取最新消息，执行 `pre_reasoning`，调用模型。

因此，压缩当轮发送的是**压缩后的**消息；注入也会在同一轮进入模型输入。详见[压缩上下文](compress-context)和[感知环境](environment-awareness)。

## 配置总览

下面的函数只依赖公开 API。`model` 由应用选择的模型实现提供。

```rust
use std::sync::Arc;

use agent_scope_agent::{
    AgentConfig, AgentError, ContextConfig, InjectionConfig, ReActAgent, ReActConfig,
};
use agent_scope_model::ChatModel;

fn build_agent(model: Arc<dyn ChatModel>) -> Result<ReActAgent, AgentError> {
    let injection_config = InjectionConfig {
        timezone: "Asia/Shanghai".into(),
        context_buffer_ratio: 0.15,
        ..Default::default()
    };

    let agent_config = AgentConfig::builder()
        .name("assistant")
        .system_prompt("回答时给出可验证的依据。")
        .model(model)
        .injection_config(injection_config)
        .build()?;

    let context_config = ContextConfig {
        enable: true,
        trigger_ratio: 0.8,
        reserve_ratio: 0.1,
        ..Default::default()
    };

    ReActAgent::new(
        agent_config,
        ReActConfig::default(),
        context_config,
        vec![],
    )
}
```

`AgentConfigBuilder::build()` 会先验证 `InjectionConfig`；`ReActAgent::new()` / `ReActAgent::build()` 随后验证 `ContextConfig`，并执行 `context_buffer_ratio < trigger_ratio` 的跨配置检查。无效配置会在构造阶段返回 `AgentError::InvalidConfig`，不会等到第一次模型调用才失败。

## SubAgent 的 Context sharing

父智能体向 SubAgent 委派任务时，需要区分两类对象：

- `ContextSharingPolicy` 描述**允许共享什么**，并用于构建或清理 `SharedContext`；
- `CapabilityScope` 描述 SubAgent **实际可以使用什么**，包括工具、memory、session、workspace、sandbox、模型和副作用范围。

### 消息共享策略

`MessageContextPolicy` 提供四种模式：

| Variant | 行为 |
|---------|------|
| `None` | 不共享父上下文消息，默认值 |
| `SummaryOnly` | 仅共享调用方提供的摘要，并生成一条 system 摘要消息 |
| `Selected { message_ids }` | 仅共享指定消息 ID |
| `Full { explicit }` | 共享全部消息；只有 `explicit: true` 才获准 |

```rust
use agent_scope_agent::{
    ContextSharingPolicy, MessageContextPolicy, ResourceSharingPolicy, SharedContext,
    SubAgentError,
};
use agent_scope_message::Msg;

fn share_for_researcher(messages: &[Msg]) -> Result<SharedContext, SubAgentError> {
    let policy = ContextSharingPolicy {
        message_policy: MessageContextPolicy::Full { explicit: true },
        workspace_policy: ResourceSharingPolicy::Scoped {
            refs: vec!["reports/input.md".into()],
        },
        ..Default::default()
    };

    let requested = SharedContext {
        messages: messages.to_vec(),
        workspace_refs: vec![
            "reports/input.md".into(),
            "private/secrets.txt".into(),
        ],
        ..SharedContext::empty()
    };

    policy.sanitize_shared_context(&requested)
}
```

`Scoped { refs }` 中的 `refs` 是授权允许列表，不是待共享引用。`build_shared_context(messages, summary)` 只根据消息策略构造消息和摘要，不会自动填充 `workspace_refs`。资源引用应由调用方放入 `SharedContext`，再经 `sanitize_shared_context` 过滤；上例最终只保留 `reports/input.md`。`Full { explicit: false }` 会返回权限错误。这个布尔值不是备注，而是防止无意泄露全部父上下文的授权闸门。

### 资源共享与能力范围

`ResourceSharingPolicy` 可用于 memory、session、workspace 等资源：

| Variant | 行为 |
|---------|------|
| `None` | 不共享，默认值 |
| `ReadOnly` | 允许只读共享 |
| `Scoped { refs }` | 只允许列出的引用 |
| `Inherited { explicit }` | 继承父资源范围；只有 `explicit: true` 才获准 |

```rust
use agent_scope_agent::{CapabilityScope, ResourceSharingPolicy};

let scope = CapabilityScope {
    tools: vec!["Read".into(), "Grep".into()],
    workspace: ResourceSharingPolicy::Inherited { explicit: true },
    memory: ResourceSharingPolicy::ReadOnly,
    ..Default::default()
};

assert!(scope.allows_tool("Read"));
assert!(scope.require_workspace().is_ok());
```

`SharedContext` 是实际传递的数据容器，包含 `messages`、可选 `summary`、各类资源引用和 `redaction_notes`。即使调用方自行构造了 `SharedContext`，委派路径仍应通过策略清理，避免绕过 `Selected` / `Scoped` 等限制。

::: warning
`MessageContextPolicy::Full` 和 `ResourceSharingPolicy::Inherited` 都要求显式授权。不要用 `explicit: true` 作为“让示例通过”的固定写法；只有确认子智能体确实需要完整消息或继承资源时才开启。
:::

## 选择机制

- 对话逐渐接近模型窗口：启用[压缩上下文](compress-context)。
- 模型需要知道当前时间、任务状态或窗口压力：配置[感知环境](environment-awareness)。
- 内容体积大、需要保留原文或二进制数据：显式调用[卸载上下文](offload-context)。
- 委派任务但要限制数据暴露：使用 `ContextSharingPolicy` 和 `CapabilityScope`。
