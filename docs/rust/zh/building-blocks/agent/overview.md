---
title: "概述"
description: "AgentScope Rust 的核心抽象：推理-行动循环引擎"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用。
</Note>

`Agent` 是 AgentScope Rust 的核心抽象：一个**推理-行动循环**引擎，将模型、工具、权限系统、人机交互、上下文管理、中间件、状态管理和事件系统整合到一个统一接口中。

其主要职责包括：

- 接收输入消息，调用工具完成任务
- 流式产出 `AgentEvent`，供 UI / TUI / WebSocket / SSE 消费
- 管理上下文（压缩、运行时状态注入）
- 在生命周期的关键节点运行中间件
- 处理用户中断，并从暂停状态继续运行（`reply_stream_event` 事件恢复）
- 通过状态持久化在进程间恢复会话

## 核心接口

`Agent` trait（`agent_scope_agent`）的主要方法：

| 方法 | 签名 | 说明 |
|------|------|------|
| `reply` | `async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError>` | 运行一轮推理-行动循环，返回最终助手消息；空上下文返回 `NoContentToReply` 错误 |
| `reply_stream` | `async fn reply_stream(&self, input: Option<Vec<Msg>>) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>` | 同 `reply`，但实时逐一产出 `AgentEvent`，以 `ReplyEnd` 结束 |
| `reply_stream_event` | `async fn reply_stream_event(&self, input: EventInput) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>` | 以 HITL 事件（确认 / 外部执行 / 中断）恢复暂停的同一回复；默认实现返回错误 |
| `observe` | `async fn observe(&self, input: Option<Vec<Msg>>) -> Result<(), AgentError>` | 将消息追加到上下文，不触发推理；`observe(None)` 为 no-op |
| `name` | `fn name(&self) -> &str` | 智能体名称 |
| `state` | `fn state(&self) -> std::sync::RwLockReadGuard<'_, AgentState>` | 智能体运行时状态（`AgentState`）的读锁；调用方不得跨 `.await` 持有该锁 |

内置实现为 `ReActAgent`（reasoning → acting 循环），由 `AgentConfig`（含 `ReActConfig`、`ContextConfig`）组装：

```rust
let agent_config = AgentConfig::builder()
    .name("assistant")
    .system_prompt("你是一个乐于助人的助手。")
    .model(model)
    .toolkit(toolkit)
    .build()?;

let agent = ReActAgent::new(
    agent_config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![],  // middlewares
)?;
```

## 事件驱动

`reply_stream` 产出 33 种 `AgentEvent`，完整覆盖回复生命周期、模型调用、文本/思考增量、工具调用与结果、人工确认/中断。完整分发见 [`examples/chat`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/chat) 与 [`examples/agent`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/agent)。

## 下一步

<CardGroup :cols="2">
  <Card title="配置智能体" icon="gear" href="/building-blocks/agent/configure-agent">
    如何设置模型、工具、权限与配置。
  </Card>
  <Card title="运行智能体" icon="play" href="/building-blocks/agent/run-agent">
    如何回复、流式、观察与持久化状态。
  </Card>
  <Card title="人机交互" icon="user-check" href="/building-blocks/agent/human-in-the-loop">
    如何暂停等待用户确认或外部执行。
  </Card>
  <Card title="中断智能体" icon="hand" href="/building-blocks/agent/interrupt-agent">
    如何干净地停止运行中或暂停中的智能体。
  </Card>
  <Card title="子智能体" icon="users" href="/building-blocks/agent/subagent">
    库级多智能体委托：SubAgent / SubAgentRegistry / delegate_*。
  </Card>
</CardGroup>
