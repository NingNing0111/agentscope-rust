---
title: "消息与事件"
description: "智能体通信与流式数据传输"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用（兼容等级 L1），兼容基线为 AgentScope Python v2.0.5。
</Note>

消息（Message）与事件（Event）是 AgentScope Rust 中两种基础数据结构。

- **消息** — 智能体间通信与上下文持久化的基本单元。
- **事件** — 前后端交互与流式传输的基本单元，支持人工介入（Human-in-the-loop）场景。

运行示例见 [`examples/chat`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/chat/)（`cargo run -p chat -- --prompt "..."`），它逐类型消费一次 `reply_stream` 产生的所有事件。

## 消息

`Msg` 结构体（`agent_scope_message` crate）的一个实例容纳一次完整的对话信息——一次用户输入或一次完整的智能体回复，信息以不同类型的内容块（`ContentBlock`）进行组织。

<Tip>
1. 智能体运行一次 `reply` 产生一个完整的 `Msg` 实例，包含多轮思考、工具调用、运行结果等所有信息。
2. 渲染时，一个 `Msg` 实例即对应一个完整的消息气泡。
</Tip>

### 结构

`Msg` 的核心字段如下：

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `String` | 唯一消息标识符 |
| `name` | `String` | 发送方名称 |
| `role` | `Role` | 发送方角色（`user`/`assistant`/`system`） |
| `content` | `Vec<ContentBlock>` | 有序内容块列表 |
| `metadata` | `serde_json::Map` | 任意键值元数据 |
| `created_at` | `String` | 创建时间（ISO 8601） |
| `finished_at` | `Option<String>` | 消息完成时间（ISO 8601） |

### 内容块

消息内容由类型化的块组成，每种块代表一类独立信息。`ContentBlock` 是一个带标签的枚举：

| 块类型 | 说明 |
|--------|------|
| `Text`（`TextBlock`） | 纯文本内容 |
| `Data`（`DataBlock`） | 二进制数据（图片、音频、视频等），可通过 `Base64Source` 或 `URLSource` 引用 |
| `Thinking`（`ThinkingBlock`） | 模型推理过程（思维链） |
| `ToolCall`（`ToolCallBlock`） | 工具调用，包含名称、输入和状态 |
| `ToolResult`（`ToolResultBlock`） | 工具执行结果 |
| `Hint`（`HintBlock`） | 提示信息（运行时状态注入等），使用 `source` 标识来源 |
| `Unknown` | 未识别的扩展块类型（保持 JSON 原样，向前兼容上游新增块） |

<Note>
角色约束在构造时强制执行：
- `role=="user"` 的消息只能包含 `Text` 和 `Data` 块；
- `role=="system"` 的消息只能包含 `Text` 块；
- `role=="assistant"` 的消息可包含所有块类型。
</Note>

### 工厂函数

推荐通过工厂函数构造消息（`agent_scope_message::factory`）：

| 函数 | 说明 |
|------|------|
| `user_msg(name, text)` | 构造 user 角色文本消息 |
| `assistant_msg(name, text)` | 构造 assistant 角色文本消息 |
| `system_msg(name, text)` | 构造 system 角色文本消息 |
| `*_with_blocks(name, blocks)` | 带自定义内容块构造 |

## 事件

`AgentEvent`（`agent_scope_event` crate）是带标签的枚举，共 **33 种**事件，覆盖回复生命周期、模型调用、内容块流式增量、工具调用与结果、人工确认/中断/外部执行、会话事件与自定义事件。事件以 `#[serde(tag = "type")]` 序列化，便于跨进程传输。

主要事件类别：

| 类别 | 事件示例 |
|------|----------|
| 回复生命周期 | `ReplyStart` / `ReplyEnd`（含 `finished_reason`） |
| 模型调用 | `ModelCallStart` / `ModelCallEnd` |
| 内容块流式增量 | `TextBlockDelta` / `ThinkingBlockDelta` / `DataBlockDelta` / `ToolCallDelta` |
| 工具执行 | `ToolCallStart` / `ToolCallEnd` / `ToolResultStart` / `ToolResultTextDelta` / `ToolResultEnd` |
| 人工介入 | `RequireUserConfirm` / `UserConfirmResult` / `UserInterrupt` |
| 外部执行 | `RequireExternalExecution` / `ExternalExecutionResult` |
| 会话 | `SessionCreated` / `SessionSaved` / `SessionTrimmed` 等 |
| 控制 | `ExceedMaxIters` / `Custom` |

一次 `reply_stream` 产生的所有事件累积为一个完整的 assistant `Msg`——因此从事件流中总能恢复出完整的消息状态。

```rust
let mut stream = agent.reply_stream(Some(vec![msg])).await?;
while let Some(event) = stream.next().await {
    match &event {
        AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
        AgentEvent::ToolCallStart(s) => println!("[tool] {}", s.tool_call_name),
        AgentEvent::ReplyEnd(e) => println!("[end] {:?}", e.finished_reason),
        _ => {}
    }
}
```

完整的事件分发写法见 [`examples/chat/src/main.rs`](https://github.com/NingNing0111/agentscope-rust/blob/master/examples/chat/src/main.rs)。
