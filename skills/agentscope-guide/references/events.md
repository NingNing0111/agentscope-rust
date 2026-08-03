# 参考:事件与流式(`agent_scope_event`)

> 详细 API 参考:`AgentEvent` 全部事件分组、流式语义、End 事件累积内容、`AppendEvent` 增量构建、取消行为。

## 1. `AgentEvent` 与 `EventType`

`AgentEvent` 是带 `type` 判别标签的枚举(serde tagged union),共 **33 个变体**;`EventType` 为对应纯判别枚举。序列化标签为 SCREAMING_SNAKE_CASE(如 `"REPLY_START"`)。

所有事件共享 `EventBase { id, created_at, metadata }`:`id` 自动 UUID,`created_at` 为 RFC 3339,`metadata` 任意键值。除 Session 事件外,其余以 `#[serde(flatten)]` 内联基座。

## 2. 事件分组总览

| 分组 | 事件 | 关键载荷 |
|------|------|----------|
| 回复生命周期 | `ReplyStart` / `ReplyEnd` | `session_id`、`reply_id`、`name`、`role`;End 含 `finished_reason` 与可选 `error: ErrorInfo` |
| 模型调用 | `ModelCallStart` / `ModelCallEnd` | `model_name`;End 含 `input_tokens`/`output_tokens`/`finished_reason` |
| 文本/思考/数据块流式 | `TextBlockStart/Delta/End`、`ThinkingBlockStart/Delta/End`、`DataBlockStart/Delta/End` | `reply_id` + `block_id`;Delta 携带增量;Data 块另带 `media_type` |
| 提示块(一次性) | `HintBlock` | `hint: HintContent`、可选 `source`,非流式 |
| 工具调用流式 | `ToolCallStart/Delta/End` | `tool_call_id`、`tool_call_name`;Delta 累积输入 JSON |
| 工具结果流式 | `ToolResultStart`、`ToolResultTextDelta`、`ToolResultDataDelta`、`ToolResultEnd` | `tool_call_id`;End 含 `state: ToolResultState` 与 `metadata` |
| 控制与交互 | `ExceedMaxIters`、`RequireUserConfirm`、`UserConfirmResult`、`UserInterrupt`、`RequireExternalExecution`、`ExternalExecutionResult` | 用户确认、外部执行、中断 |
| 会话生命周期 | `SessionCreated/Closed/Saved/Loaded/Trimmed` | `session_id`;Closed 带 `reason`;Trimmed 带修剪前后消息数与 token 数 |
| 自定义 | `Custom` | `name` + `value: HashMap<String, Value>` |

## 3. End 事件携带完整累积内容(Feature 014)

流式块的 End 事件不仅标记生命周期结束,还携带从 Start 到 End 的**完整累积内容**:

| 事件 | 完整内容字段 | 语义 |
|------|-------------|------|
| `TextBlockEnd` | `text: Option<String>` | 该块全部 Delta 拼接的完整文本 |
| `ThinkingBlockEnd` | `thinking: Option<String>` | 该块完整推理内容 |
| `ToolCallEnd` | `input: Option<String>` | 全部 Delta 累积的完整输入 JSON |
| `ToolResultEnd` | `output: Option<String>` | 全部 TextDelta 累积的完整可观察输出 |

统一约定:`Some("")` = **已知为空**;`None` = **未知/不可用**。`ToolResultEnd` 在 `error`/`interrupted` 状态下 `output` 必须为 `None`。

## 4. 事件发布顺序(trace 语义)

```text
ReplyStart
└─ ModelCallStart
   ├─ ThinkingBlockStart → ThinkingBlockDelta* → ThinkingBlockEnd   (可选)
   ├─ TextBlockStart → TextBlockDelta* → TextBlockEnd               (可选)
   └─ ToolCallStart → ToolCallDelta* → ToolCallEnd                  (可选,可多个)
      └─ ToolResultStart → ToolResultTextDelta*/ToolResultDataDelta* → ToolResultEnd
└─ ModelCallEnd
(多轮 reasoning-acting 时重复 Model 调用段)
ReplyEnd(finished_reason: completed / interrupted / exceed_max_iters / error)
```

`reply_id` 贯穿一次回复全部事件;`block_id`/`tool_call_id` 贯穿单个块生命周期——这两个 ID 是把事件流关联成结构化消息的锚点。

## 5. 消费事件流

```rust
use futures::StreamExt;
use agent_scope_event::AgentEvent;

let mut stream = agent
    .reply_stream(Some(vec![user_msg("user", "一步步计算 (2+3)*4")?]))
    .await?;

while let Some(event) = stream.next().await {
    match event {
        AgentEvent::ReplyStart(e) => eprintln!("reply {} started", e.reply_id),
        AgentEvent::ModelCallStart(e) => eprintln!("model: {}", e.model_name),
        AgentEvent::TextBlockDelta(e) => print!("{}", e.delta),
        AgentEvent::ThinkingBlockDelta(e) => eprint!("[thinking] {}", e.delta),
        AgentEvent::ToolCallStart(e) => eprintln!("tool: {}", e.tool_call_name),
        AgentEvent::ToolCallEnd(e) => eprintln!("tool input: {:?}", e.input),
        AgentEvent::ToolResultEnd(e) => eprintln!("tool result: {:?} {:?}", e.state, e.output),
        AgentEvent::ReplyEnd(e) => eprintln!("finished: {:?}", e.finished_reason),
        _ => {}
    }
}
```

应至少处理:`ReplyStart`/`ReplyEnd`、`ModelCallStart`/`ModelCallEnd`、`TextBlock*`/`ThinkingBlock*`、`ToolCall*`/`ToolResult*`、`UserInterrupt`/`ExceedMaxIters`。

## 6. 用 End 事件完整内容做汇总

```rust
AgentEvent::TextBlockEnd(e) => {
    if let Some(full_text) = &e.text {
        // full_text 是该块 Start→End 的完整文本,无需自行拼接 Delta
    }
}
AgentEvent::ToolResultEnd(e) => {
    // error/interrupted 状态下 output 为 None——不得当作空串
    match (&e.state, &e.output) { /* ... */ }
}
```

## 7. `AppendEvent`:事件流 → Msg

`AppendEvent` trait 把事件流增量应用到 `Msg` 上(Delta 追加文本、End 定稿块、`ToolResultEnd` 写入状态):

| 错误 | 触发条件 |
|------|----------|
| `AppendEventError::ReplyIdMismatch` | 事件 `reply_id` 与目标消息 `id` 不一致 |
| `AppendEventError::BlockNotFound` | Delta/End 引用的 `block_id` 不存在(缺 Start) |
| `AppendEventError::UnknownEventType` | 无法识别的事件类型 |

## 8. 取消行为

1. `CancellationToken`(tokio-util):Agent 层每轮迭代检查取消信号。
2. `UserInterrupt` 事件:取消时发出,`ReplyEnd` 以 `finished_reason: interrupted` 收尾。

消费方以此区分正常结束与被中断。

## 9. 发布自定义事件

```rust
use agent_scope_event::{AgentEvent, EventBase};

let event = AgentEvent::Custom(/* name + value */);
// 服务层可用 Custom 事件发进度通知,不污染协议事件空间
```

## 10. 常见问题

- **End 事件内容字段为什么是 `Option`**:`Some("")` 已知为空,`None` 未知/不可用(如被中断的工具结果)。
- **收到 Delta 却没有对应 Start**:协议保证 Start→Delta*→End 顺序;缺 Start 说明流被截断或 `reply_id` 串话,`AppendEvent` 会报 `BlockNotFound`。
