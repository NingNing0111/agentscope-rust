---
title: "中断智能体"
description: "干净地停止运行中的智能体，并从一致状态恢复"
---

<Note>
**Rust 实现状态**: 已实现。`ReActAgent::interrupt()` 置位中断标志 + 取消 token，并发出 `UserInterruptEvent`；同时支持以 `reply_stream_event(EventInput::Interrupt)` 注入中断事件。
</Note>

`ReActAgent` 基于显式中断标志 + 取消 token 实现中断机制，支持在模型推理或工具执行的任意阶段停止执行。中断之后，智能体的上下文保持一致状态，会话可以立即通过新的输入消息继续。

## 中断运行中的智能体

```rust
let agent = Arc::new(ReActAgent::new(config, ReActConfig::default(), ContextConfig::default(), vec![])?);

// 在另一任务上延迟触发中断。
let agent2 = Arc::clone(&agent);
tokio::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    agent2.interrupt();
});

let mut stream = agent.reply_stream(Some(vec![msg])).await?;
while let Some(event) = stream.next().await {
    match &event {
        AgentEvent::UserInterrupt(_) => println!("[interrupted]"),
        AgentEvent::ReplyEnd(e) => println!("[end] {:?}", e.finished_reason),
        _ => {}
    }
}
```

## 中断语义

- **运行中中断**：`interrupt()` 置位中断标志并传播取消；当前推理-行动步骤干净展开，回复以「被中断」的结束原因结束。
- **一致性**：中断后上下文保持一致，可立即用新输入继续会话。
- **事件**：中断过程发出 `UserInterruptEvent`，最终 `ReplyEnd` 携带 `finished_reason`。

## 事件注入中断（Feature 032）

除 `interrupt()` 方法外，还可向 `reply_stream_event` 注入 `EventInput::Interrupt`，用于从事件流一侧触发中断：

```rust
use agent_scope_agent::event_input::EventInput;
use agent_scope_event::{EventBase, UserInterruptEvent};

let mut stream = agent.reply_stream_event(EventInput::Interrupt(UserInterruptEvent {
    base: EventBase::new(),
    reply_id: reply_id.to_string(),
})).await?;
while let Some(event) = stream.next().await {
    if let AgentEvent::ReplyEnd(e) = &event {
        println!("[end] {:?}", e.finished_reason);  // INTERRUPTED
    }
}
```

中断语义：

- **有进行中回复 / 等待确认**：注入 `UserInterruptEvent` 后回复以 `ReplyEnd(finished_reason=INTERRUPTED)` 结束。
- **无进行中回复**：静默 no-op（不产生任何副作用）。

<Note>
中断时返回给模型/上层的文本由 `ReActConfig.interruption_message` 决定，默认是 `"The execution was interrupted."`，可在构造时自定义。
</Note>

## 完整示例

见 [`examples/agent`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/agent/)，演示延迟 `interrupt()` + 流式事件响应。
