---
title: "消息与事件"
description: "智能体通信与流式数据传输"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用。
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

`AgentEvent`（`agent_scope_event` crate）是带标签的枚举，共 **33 种**事件，覆盖回复生命周期、模型调用、内容块流式增量、工具调用与结果、人工确认/中断/外部执行、会话事件与自定义事件。事件以 `#[serde(tag = "type")]` 序列化（每种事件对应一个 `type` 字符串标签），便于跨进程传输。

### 公共字段

每个事件结构体都通过 `#[serde(flatten)]` 内嵌 `EventBase`：

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `String` | 事件唯一标识符（默认 UUID） |
| `created_at` | `String` | 事件创建时间（ISO 8601 / RFC 3339） |
| `metadata` | `HashMap<String, Value>` | 任意键值元数据 |

### 事件目录

按类别列出全部 33 种事件。`type` 为序列化标签，变体为 `AgentEvent` 枚举成员，事件结构体为对应负载类型。

**回复生命周期**（2 种）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `REPLY_START` | `ReplyStart` | `ReplyStartEvent` | `session_id`、`reply_id`、`name`、`role` | 一次回复开始，标识本次回复所属会话、回复 ID 与发言方 |
| `REPLY_END` | `ReplyEnd` | `ReplyEndEvent` | `session_id`、`reply_id`、`finished_reason`、`error?` | 回复结束；`finished_reason` 取值 `completed` / `interrupted` / `exceed_max_iters` / `error`，`error` 携带失败详情 |

**模型调用**（2 种）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `MODEL_CALL_START` | `ModelCallStart` | `ModelCallStartEvent` | `reply_id`、`model_name` | 一次模型调用开始 |
| `MODEL_CALL_END` | `ModelCallEnd` | `ModelCallEndEvent` | `reply_id`、`input_tokens`、`output_tokens`、`finished_reason` | 模型调用结束，报告 token 用量与结束原因；多次调用会在 `Msg.usage` 上累计 |

**文本块**（3 种）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `TEXT_BLOCK_START` | `TextBlockStart` | `TextBlockStartEvent` | `reply_id`、`block_id` | 文本内容块开始 |
| `TEXT_BLOCK_DELTA` | `TextBlockDelta` | `TextBlockDeltaEvent` | `reply_id`、`block_id`、`delta` | 文本增量片段，按序拼接即得完整文本 |
| `TEXT_BLOCK_END` | `TextBlockEnd` | `TextBlockEndEvent` | `reply_id`、`block_id`、`text?` | 文本块结束；`text` 为从 Start 到 End 的完整文本（`None` 表示未知，`Some("")` 表示确认为空） |

**数据块**（3 种）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `DATA_BLOCK_START` | `DataBlockStart` | `DataBlockStartEvent` | `reply_id`、`block_id`、`media_type` | 二进制数据块开始（图片、音频、视频等），声明媒体类型 |
| `DATA_BLOCK_DELTA` | `DataBlockDelta` | `DataBlockDeltaEvent` | `reply_id`、`block_id`、`data`、`media_type` | Base64 编码的数据增量片段；片段可能未对齐 4 字节边界，需拼接后整体解码 |
| `DATA_BLOCK_END` | `DataBlockEnd` | `DataBlockEndEvent` | `reply_id`、`block_id` | 数据块结束，此刻 base64 流完整可解码 |

**思考块**（3 种）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `THINKING_BLOCK_START` | `ThinkingBlockStart` | `ThinkingBlockStartEvent` | `reply_id`、`block_id` | 思维链内容块开始 |
| `THINKING_BLOCK_DELTA` | `ThinkingBlockDelta` | `ThinkingBlockDeltaEvent` | `reply_id`、`block_id`、`delta` | 思考内容增量片段 |
| `THINKING_BLOCK_END` | `ThinkingBlockEnd` | `ThinkingBlockEndEvent` | `reply_id`、`block_id`、`thinking?` | 思考块结束，携带完整思考内容（语义同 `TEXT_BLOCK_END.text`） |

**提示块**（1 种，一次性、非流式）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `HINT_BLOCK` | `HintBlock` | `HintBlockEvent` | `reply_id`、`block_id`、`source?`、`hint` | 运行时状态注入的提示信息（见 [环境感知](context/environment-awareness)）；`hint` 为 `HintContent`（纯文本或块列表），`source` 标识提示来源 |

**工具调用**（3 种）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `TOOL_CALL_START` | `ToolCallStart` | `ToolCallStartEvent` | `reply_id`、`tool_call_id`、`tool_call_name` | 一次工具调用开始 |
| `TOOL_CALL_DELTA` | `ToolCallDelta` | `ToolCallDeltaEvent` | `reply_id`、`tool_call_id`、`delta` | 工具输入 JSON 的增量片段 |
| `TOOL_CALL_END` | `ToolCallEnd` | `ToolCallEndEvent` | `reply_id`、`tool_call_id`、`input?` | 工具输入完整接收；`input` 为累积后的完整 JSON 字符串 |

**工具结果**（4 种）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `TOOL_RESULT_START` | `ToolResultStart` | `ToolResultStartEvent` | `reply_id`、`tool_call_id`、`tool_call_name` | 工具执行结果开始流式输出 |
| `TOOL_RESULT_TEXT_DELTA` | `ToolResultTextDelta` | `ToolResultTextDeltaEvent` | `reply_id`、`tool_call_id`、`delta` | 工具结果文本增量片段 |
| `TOOL_RESULT_DATA_DELTA` | `ToolResultDataDelta` | `ToolResultDataDeltaEvent` | `reply_id`、`tool_call_id`、`block_id`、`media_type`、`data?`、`url?` | 工具结果二进制增量（`data` 或 `url` 二选一） |
| `TOOL_RESULT_END` | `ToolResultEnd` | `ToolResultEndEvent` | `reply_id`、`tool_call_id`、`state`、`metadata`、`output?` | 工具结果完成；`state` 取值 `running` / `success` / `error` / `interrupted` / `denied`，`output` 为完整可观察输出 |

**控制与人工介入**（6 种）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `EXCEED_MAX_ITERS` | `ExceedMaxIters` | `ExceedMaxItersEvent` | `reply_id`、`name` | 推理-行动循环超过最大迭代次数，替代正常的 `REPLY_END` |
| `REQUIRE_USER_CONFIRM` | `RequireUserConfirm` | `RequireUserConfirmEvent` | `reply_id`、`tool_calls` | 需要用户确认的工具调用列表（`ToolCallBlock`，状态为 `asking`），回复暂停等待注入确认结果 |
| `USER_CONFIRM_RESULT` | `UserConfirmResult` | `UserConfirmResultEvent` | `reply_id`、`confirm_results` | 用户对确认请求的响应；`ConfirmResult` 含 `confirmed`、`tool_call` 与可选 `rules`（采纳的授权规则） |
| `USER_INTERRUPT` | `UserInterrupt` | `UserInterruptEvent` | `reply_id` | 用户中断当前回复（`agent.interrupt()` 触发），回复以 `interrupted` 结束 |
| `REQUIRE_EXTERNAL_EXECUTION` | `RequireExternalExecution` | `RequireExternalExecutionEvent` | `reply_id`、`tool_calls` | 需要宿主外部执行工具调用（状态为 `submitted`），回复暂停等待外部结果 |
| `EXTERNAL_EXECUTION_RESULT` | `ExternalExecutionResult` | `ExternalExecutionResultEvent` | `reply_id`、`execution_results` | 外部执行结果列表（`ToolResultBlock`），注入后恢复同一回复 |

**会话事件**（5 种，独立于回复流）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `SESSION_CREATED` | `SessionCreated` | `SessionCreatedEvent` | `session_id` | 新会话创建 |
| `SESSION_CLOSED` | `SessionClosed` | `SessionClosedEvent` | `session_id`、`reason` | 会话关闭；`reason` 取值 `explicit_close` / `drop` / `error` |
| `SESSION_SAVED` | `SessionSaved` | `SessionSavedEvent` | `session_id`、`message_count` | 会话持久化到存储 |
| `SESSION_LOADED` | `SessionLoaded` | `SessionLoadedEvent` | `session_id`、`message_count` | 会话从存储恢复 |
| `SESSION_TRIMMED` | `SessionTrimmed` | `SessionTrimmedEvent` | `session_id`、`messages_before`、`messages_after`、`tokens_before?`、`tokens_after?` | 上下文压缩裁剪后发出，报告前后消息数与 token 数 |

**自定义**（1 种）

| `type` | 变体 | 事件结构体 | 关键字段 | 说明 |
|--------|------|-----------|----------|------|
| `CUSTOM` | `Custom` | `CustomEvent` | `name`、`value` | 服务层自定义事件，携带任意 JSON 负载，可在任意时间点发出 |

### 生命周期约束

事件遵循有序的生命周期，消费方可按约束做流式重建：

| 序列规则 | 约束 |
|----------|------|
| `REPLY_START` → … → `REPLY_END` | 正常结束；`EXCEED_MAX_ITERS` 或 `USER_INTERRUPT` 会替代正常的 `REPLY_END` |
| `MODEL_CALL_START` → `MODEL_CALL_END` | 成对出现，可多次 |
| `TEXT/DATA/THINKING_BLOCK_START` → `*_DELTA`… → `*_END` | Delta 与 End 依赖对应的 Start |
| `TOOL_CALL_START` → `TOOL_CALL_DELTA`… → `TOOL_CALL_END` → `TOOL_RESULT_START` → … → `TOOL_RESULT_END` | 完整的工具生命周期 |
| `HINT_BLOCK` | 一次性事件，无 Start/Delta/End |
| `CUSTOM` / 会话事件 | 任意时间点，无约束 |

### 事件流恢复消息

`AppendEvent` trait（`agent_scope_event`）能把事件增量应用到 `Msg` 上：一次 `reply_stream` 产生的所有事件累积为一个完整的 assistant `Msg`——因此从事件流中总能恢复出完整的消息状态（文本、思考、工具调用与结果、token 用量、结束原因等）。

```rust
use agent_scope_event::{AgentEvent, AppendEvent};
use agent_scope_message::{Msg, Role};

let mut msg = Msg::new("assistant".into(), vec![], Role::Assistant)?;
let mut stream = agent.reply_stream(Some(vec![input_msg])).await?;
while let Some(event) = stream.next().await {
    msg.append_event(&event)?;   // 增量重建完整回复
}
```

### 消费事件流

```rust
let mut stream = agent.reply_stream(Some(vec![msg])).await?;
while let Some(event) = stream.next().await {
    match &event {
        AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
        AgentEvent::ThinkingBlockDelta(d) => print!("\x1b[2m{}\x1b[0m", d.delta),
        AgentEvent::ToolCallStart(s) => println!("[tool] {}", s.tool_call_name),
        AgentEvent::ReplyEnd(e) => println!("[end] {:?}", e.finished_reason),
        _ => {}
    }
}
```

完整的事件分发写法（逐类型消费一次 `reply_stream` 产生的所有事件）见 [`examples/chat/src/main.rs`](https://github.com/NingNing0111/agentscope-rust/blob/master/examples/chat/src/main.rs)。
