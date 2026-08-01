# 事件与流式 / Event & Streaming

> 一句话定位：AgentScope 的统一事件协议——33 种 `AgentEvent` 覆盖回复生命周期、模型调用、内容块流式、工具执行与会话管理，是观察 Agent 行为（trace）与构建流式 UI 的唯一入口。

## 1. 模块概述 (Overview)

本模块对应 `agent_scope_event` crate，位于基础层（仅依赖 `agent_scope_types` 与 `agent_scope_message`），定义 Agent 运行期间发出的全部事件类型。流式语义（事件的产生顺序、取消、背压）由 Agent/模型层实现，本模块是它们的公共词汇表。

**适用场景**：消费 Agent 事件流渲染终端/UI、按事件追踪一次回复的完整 trace、用 `AppendEvent` 将事件流增量构建为 `Msg`、发布自定义服务层事件。

**前置阅读**：[消息与基础类型](./message-types.md)（事件载荷中的 `Msg`/`ContentBlock`/`ToolResultState`）。

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 事件总线：`AgentEvent` 与 `EventType`

`AgentEvent` 是带 `type` 判别标签的枚举（serde tagged union），共 **33 个变体**；`EventType` 为对应的纯判别枚举。序列化标签为 SCREAMING_SNAKE_CASE（如 `"REPLY_START"`、`"TEXT_BLOCK_DELTA"`）。

所有事件共享基座 `EventBase { id, created_at, metadata }`：`id` 自动生成 UUID，`created_at` 为 RFC 3339 时间戳，`metadata` 为任意键值字典。除 Session 事件外，各事件以 `#[serde(flatten)]` 内联基座字段。

### 2.2 事件分组

| 分组 | 事件 | 关键载荷 |
|------|------|----------|
| 回复生命周期 | `ReplyStart` / `ReplyEnd` | `session_id`、`reply_id`、`name`、`role`；End 含 `finished_reason` 与可选 `error: ErrorInfo` |
| 模型调用 | `ModelCallStart` / `ModelCallEnd` | `model_name`；End 含 `input_tokens`/`output_tokens`/`finished_reason` |
| 文本/思考/数据块流式 | `TextBlockStart/Delta/End`、`ThinkingBlockStart/Delta/End`、`DataBlockStart/Delta/End` | `reply_id` + `block_id`；Delta 携带增量 `delta`/`data`；Data 块另带 `media_type` |
| 提示块（一次性） | `HintBlock` | `hint: HintContent`、可选 `source`，非流式、无 Start/Delta/End 序列 |
| 工具调用流式 | `ToolCallStart/Delta/End` | `tool_call_id`、`tool_call_name`；Delta 累积输入 JSON |
| 工具结果流式 | `ToolResultStart`、`ToolResultTextDelta`、`ToolResultDataDelta`、`ToolResultEnd` | `tool_call_id`；End 含 `state: ToolResultState` 与 `metadata` |
| 控制与交互 | `ExceedMaxIters`、`RequireUserConfirm`、`UserConfirmResult`、`UserInterrupt`、`RequireExternalExecution`、`ExternalExecutionResult` | 用户确认（`ConfirmResult { confirmed, tool_call, rules }`）、外部执行（`Vec<ToolResultBlock>`）、中断 |
| 会话生命周期 | `SessionCreated/Closed/Saved/Loaded/Trimmed` | `session_id`；Closed 带 `reason`（`explicit_close`/`drop`/`error`）；Trimmed 带修剪前后消息数与可选 token 数 |
| 自定义 | `Custom` | `name` + `value: HashMap<String, Value>`，服务层任意通知 |

### 2.3 End 事件携带完整累积内容（Feature 014 语义）

流式块的 End 事件不仅表示生命周期结束，还携带从 Start 到 End 的**完整累积内容**：

| 事件 | 完整内容字段 | 语义 |
|------|-------------|------|
| `TextBlockEnd` | `text: Option<String>` | 该块全部 Delta 拼接的完整文本 |
| `ThinkingBlockEnd` | `thinking: Option<String>` | 该块完整推理内容 |
| `ToolCallEnd` | `input: Option<String>` | 全部 `ToolCallDelta` 累积的完整输入 JSON |
| `ToolResultEnd` | `output: Option<String>` | 全部 `ToolResultTextDelta` 累积的完整可观察输出 |

统一约定：`Some("")` 表示**已知为空**；`None` 表示**未知/不可用**。`ToolResultEnd` 在 `error`/`interrupted` 状态下 `output` 必须为 `None`（除非输出已知完整）——消费方不得把 `None` 当作空字符串。

### 2.4 事件发布顺序（trace 语义）

一次回复的典型事件序列（宪法第七条：trace 是核心验收产物）：

```text
ReplyStart
└─ ModelCallStart
   ├─ ThinkingBlockStart → ThinkingBlockDelta* → ThinkingBlockEnd   （可选）
   ├─ TextBlockStart → TextBlockDelta* → TextBlockEnd               （可选）
   └─ ToolCallStart → ToolCallDelta* → ToolCallEnd                  （可选，可多个）
      └─ ToolResultStart → ToolResultTextDelta*/ToolResultDataDelta* → ToolResultEnd
└─ ModelCallEnd
（多轮 reasoning-acting 时重复 Model 调用段）
ReplyEnd（finished_reason: completed / interrupted / exceed_max_iters / error）
```

`reply_id` 贯穿一次回复的全部事件；`block_id`/`tool_call_id` 贯穿单个块的生命周期——这两个 ID 是将事件流关联成结构化消息的锚点。

### 2.5 取消行为

取消通过两条路径协作（Feature 008 语义）：

1. **`CancellationToken`**（tokio-util）：Agent 层在每轮迭代检查取消信号，取消后停止后续模型调用与工具执行；
2. **`UserInterrupt` 事件**：取消发生时发出，`ReplyEnd` 随之以 `finished_reason: interrupted` 收尾——消费方以此区分正常结束与被中断。

### 2.6 `AppendEvent`：事件流 → Msg

`AppendEvent` trait 将事件流增量应用到 `Msg` 上，逐步构建完整消息（Delta 追加文本、End 定稿块、`ToolResultEnd` 写入状态）。应用失败返回 `AppendEventError`：

| 错误 | 触发条件 |
|------|----------|
| `ReplyIdMismatch` | 事件 `reply_id` 与目标消息 `id` 不一致 |
| `BlockNotFound` | Delta/End 事件引用的 `block_id` 不存在（缺少对应 Start） |
| `UnknownEventType` | 无法识别的事件类型 |

## 3. 快速示例 (Quick Example)

终端聊天示例用 `BlockTracker` 按块追踪状态并为 End 事件汇总累积内容：

<!-- source: examples/chat.rs:L56-L78 -->
```rust
#[derive(Default)]
#[allow(dead_code)]
struct BlockTracker {
    current_text_id: Option<String>,
    current_thinking_id: Option<String>,
    current_tool_call_id: Option<String>,
    current_tool_result_id: Option<String>,
    current_data_id: Option<String>,
    /// Accumulated content per block type.
    text_buf: String,
    thinking_buf: String,
    tool_call_buf: String,
    tool_result_text_buf: String,
    data_len: usize,
}
```

完整的事件渲染循环见 `examples/chat.rs`（`render_event()`，L86 起），覆盖全部 `AgentEvent` 变体的终端渲染——它是"如何消费事件流"的权威参考实现。

## 4. 关键用法模式 (Usage Patterns)

### 4.1 消费事件流（match 全部分组）

事件以 Rust 枚举变体匹配，建议按分组组织 match 分支（参考 `examples/chat.rs` 的 `render_event`）：Reply/Model 生命周期打印边界标记，Delta 事件即时输出增量，End 事件输出累积汇总并重置块状态。

### 4.2 用 End 事件的完整内容做汇总

End 事件到达时，可直接读取其完整内容字段，无需自行拼接 Delta（Feature 014）：

```rust
AgentEvent::TextBlockEnd(e) => {
    if let Some(full_text) = &e.text {
        // full_text 是该块 Start→End 的完整文本
    }
}
AgentEvent::ToolResultEnd(e) => {
    // error/interrupted 状态下 output 为 None——不得当作空串
    match (&e.state, &e.output) { /* ... */ }
}
```

### 4.3 增量构建 Msg（AppendEvent）

持有一个 assistant `Msg`，对每个到来的事件调用 `append_event(&event)`，回复结束时得到完整结构化消息。注意按 `reply_id` 校验事件归属，避免串话（`ReplyIdMismatch`）。

### 4.4 发布自定义事件

服务层（如进度通知）可用 `Custom` 事件，不污染协议事件空间：

```rust
let event = CustomEvent {
    base: EventBase::new(),
    name: "ingest-progress".into(),
    value: HashMap::from([("done".into(), serde_json::json!(3))]),
};
```

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误类型 | 触发条件 |
|----------|----------|
| `AppendEventError::ReplyIdMismatch` | 事件应用到不匹配的 `Msg` |
| `AppendEventError::BlockNotFound` | Delta/End 缺少对应 Start（块未找到） |
| `AppendEventError::UnknownEventType` | 无法识别的事件类型 |

**不支持的能力**：无。本模块为纯事件协议，无返回 `UnsupportedFeature` 的路径。

**常见问题**：

- *End 事件的内容字段为什么是 `Option`*：`Some("")` 与 `None` 语义不同——前者已知为空，后者未知/不可用（如被中断的工具结果）。
- *为什么收到 Delta 却没有对应 Start*：协议保证 Start→Delta*→End 顺序；缺失 Start 说明事件流被截断或 `reply_id` 串话，`AppendEvent` 会报 `BlockNotFound`。

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L1**（事件结构与序列化协议逐字段兼容，5 条）；**L2**（事件发布顺序/流式语义行为等价，29 条）
- **权威来源**: `specs/001-compatibility-baseline/capability-matrix.json`
- **已知偏差**:
  - 矩阵 `status` 字段当前全部为 `NOT_ANALYZED`（未随 Feature 001-017 回填）；本页等级以矩阵 `target_level`（event 类目 L1×5/L2×29）+ `specs/008-streaming-infrastructure`、`specs/014-end-event-content` + 代码实际状态交叉核实为准。
  - End 事件携带完整累积内容（`text`/`thinking`/`input`/`output` 字段）为 Feature 014 的 Rust 侧增强，Python 侧 End 事件仅标记生命周期结束。
  - `EventType` 实际共 33 个变体（crate 文档注释中的"27"为过时描述，以枚举为准）。
  - Session 事件的 `base` 字段为嵌套序列化（未 flatten），与其余事件的内联基座不同。
- **不支持的能力**: 无。

## 7. 相关模块 (See Also)

- [消息与基础类型 / message-types](./message-types.md) — 事件载荷与 `AppendEvent` 的目标结构
- [Agent 系统 / agent](./agent.md) — 事件流的产生方与取消机制
- [模型抽象 / model](./model.md) — 流式 chunk 到事件的转换（StreamAccumulator）
- [会话管理 / session](./session.md) — Session 生命周期事件的发出方
