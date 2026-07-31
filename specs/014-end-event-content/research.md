# Research Document: End Event Content

**Feature**: 014-end-event-content | **Date**: 2026-07-31

## Decision 1: EndEvent 新增可选完整内容字段

**Decision**: 在现有 EndEvent 结构上新增可选字段，而不是新增新的 CompleteEvent 类型或改变 DeltaEvent：

- `TextBlockEndEvent.text: Option<String>`
- `ThinkingBlockEndEvent.thinking: Option<String>`
- `ToolCallEndEvent.input: Option<String>`
- `ToolResultEndEvent.output: Option<String>`

字段使用“缺失/None = 内容未知或不可用；Some("") = 已知完整内容为空”的语义。

**Rationale**: Feature spec 明确要求 EndEvent 携带从 Start 到 End 期间流式收集的完整结果，同时考虑非流式场景。当前事件结构位于 `crates/agent_scope_event/src/block_events.rs` 和 `crates/agent_scope_event/src/tool_events.rs`，EndEvent 已经是所有消费者识别 block 生命周期结束的稳定信号。在现有结构增加可选字段可以保持事件类型、顺序、数量和生命周期语义不变，并让旧序列化数据缺失字段时仍可被解释为内容未知。

**Alternatives considered**:
- **新增 `TextBlockCompleteEvent` / `ToolCallCompleteEvent` 等事件**: 会增加事件序列复杂度，要求消费者同时理解 End 与 Complete 的相对顺序，不符合“EndEvent 携带完整内容”的需求。
- **让 DeltaEvent 的最后一个 delta 携带完整内容**: 会改变 Delta 的增量语义，导致重复内容和消费者拼接错误。
- **只在 Trace 层累积内容**: 只能解决调试展示，无法让普通事件消费者从 EndEvent 获取完整内容。

## Decision 2: 保持 DeltaEvent 发布，不让 EndEvent 替代流式增量

**Decision**: EndEvent 内容字段是便利快照，DeltaEvent 继续按原顺序发布，现有消费者仍可自行拼接 Delta。

**Rationale**: 宪法第七条要求 Trace 比较完整 Streaming chunks 和 Agent events；第十五条禁止为了优化改变事件发布顺序。Feature spec FR-015 也明确要求保留增量事件。EndEvent 的新增字段只降低消费者维护累积状态的成本，不改变原流式协议的主数据流。

**Alternatives considered**:
- **停止发送 Delta，只在 End 发送完整内容**: 破坏实时 UI、流式体验和现有消费者。
- **配置开关决定是否发送 Delta**: 增加协议分叉，不利于兼容性测试和 trace 比较。

## Decision 3: 流式模型输出在 `BlockTracker` 生命周期状态中累积

**Decision**: 流式模型路径使用 `crates/agent_scope_agent/src/streaming_reactor.rs` 中的 `BlockTracker` 保存每个 active block 的累积内容，并在 close helper 发布 EndEvent 时填入完整内容。

当前相关状态：

- `text_blocks: HashMap<String, (bool, Vec<String>)>`
- `thinking_blocks: HashMap<String, (bool, Vec<String>)>`
- `tool_blocks: HashMap<String, ToolCallBlock>`
- `completed_tool_ids: Vec<String>`

实现时应确保：

- `process_text_block_chunk()` 在发送 `TextBlockDeltaEvent` 后把 `tb.text` 追加到该 block 的 `Vec<String>`。
- `process_thinking_block_chunk()` 在发送 `ThinkingBlockDeltaEvent` 后把 `thb.thinking` 追加到该 block 的 `Vec<String>`。
- `ToolCallBlock.input` 已在 tool call chunk 路径中累积，可在 `close_active_tool_blocks()` 中填入 `ToolCallEndEvent.input`。
- `close_all_text_blocks()` / `close_all_thinking_blocks()` / `close_active_tool_blocks()` 发布 EndEvent 时从 tracker 取出并拼接内容。

**Rationale**: `BlockTracker` 已经是流式事件生命周期的来源。把累积状态放在同一结构中，可以保证 Start/Delta/End 的 block_id 关联一致，避免在另一个全局 accumulator 中重复推断事件边界。`StreamAccumulator` 位于 model crate，负责构建最终 `ChatResponse`；它不拥有事件发布上下文，不适合作为 EndEvent 字段填充的唯一来源。

**Alternatives considered**:
- **复用 `StreamAccumulator` 构建后的完整响应回填 EndEvent**: EndEvent 已在流式过程中发布，无法事后回填；若延迟发布会改变事件时序。
- **由消费者或 trace 层自行累积**: 与需求相反，不能让 EndEvent 自身携带内容。
- **在 event crate 内部实现累积器**: event crate 应保持纯协议层，不应依赖 agent streaming 生命周期状态。

## Decision 4: 非流式路径直接从完整 block 或工具输出填充 EndEvent

**Decision**: 非流式路径在发布 Start → Delta → End 的同一代码块中直接把完整内容写入 EndEvent 字段。

关键路径：

- `crates/agent_scope_agent/src/react_loop.rs`：非 streaming loop 中文本、thinking、工具调用和工具结果事件。
- `crates/agent_scope_agent/src/streaming_reactor.rs::process_response_and_continue()`：Complete model call 路径下工具调用事件。
- `crates/agent_scope_agent/src/streaming_reactor.rs::execute_tool_call()`：Complete/Stream/Error 工具结果事件。

**Rationale**: 非流式响应天然已有完整 block 内容，例如 `TextBlock.text`、`ThinkingBlock.thinking`、`ToolCallBlock.input` 和工具输出文本。直接填充 EndEvent 可以保证流式和非流式消费者体验一致，且不需要新增中间状态。

**Alternatives considered**:
- **非流式仍让消费者从 Delta 拼接**: 不满足“非流式场景也考虑”的需求。
- **将非流式伪装为多段流式后统一累积**: 增加不必要复杂度，且完整内容已经可得。

## Decision 5: ToolResultEndEvent 输出以消费者可观察文本为准

**Decision**: `ToolResultEndEvent.output` 使用已经通过 `ToolResultTextDeltaEvent.delta` 发布给消费者的文本内容：

- `ToolOutput::Text(t)` → `Some(t)` 或流式 collected 文本。
- `ToolOutput::Blocks(_)` → 与现有 Delta 行为一致，使用当前可观察占位文本（目前路径中为 `"[blocks]"`）作为 output。
- 工具执行错误路径 → 若没有发布 text delta，output 为 `None`；若实现选择发布错误文本 delta，则 EndEvent output 应与该 delta 一致。
- 被取消的工具流 → `state = Interrupted` 时不得宣称完整成功输出；output 应为 `None` 或只在文档中明确为“已收集的部分输出”时使用。Feature 014 默认选择 `None` 以避免误导。

**Rationale**: Feature spec 定义完整内容为“可观察内容累积结果”，而当前 tool result event 协议只有 `ToolResultTextDeltaEvent` 和 `ToolResultDataDeltaEvent`。EndEvent 的 output 必须与消费者实际看到的 delta 保持一致，不能从内部 `ToolOutput` 生成另一种格式。

**Alternatives considered**:
- **把 `ToolOutput::Blocks` 序列化为 JSON 放入 output**: 会与现有 `"[blocks]"` delta 不一致，改变消费者可观察内容。
- **错误路径总是填充 `format!("Error: {e}")`**: 如果没有对应 delta，则 EndEvent 会包含消费者未从流中看到的内容，破坏可观察一致性。

## Decision 6: 序列化兼容策略使用 serde default + skip_none

**Decision**: 新增可选字段在事件结构上使用兼容序列化策略：缺失字段可反序列化为 `None`，`None` 序列化时省略，`Some("")` 序列化为空字符串。

推荐字段属性：

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub text: Option<String>;
```

同理应用于 `thinking`、`input`、`output`。

**Rationale**: 宪法第十二条要求稳定数据协议和新增字段的向后兼容。旧事件 JSON 不包含这些字段时必须正常反序列化；新事件在无内容或未知内容时应避免输出误导性字段；空字符串又必须能和 `None` 区分。

**Alternatives considered**:
- **非可选 `String` + default 空字符串**: 无法区分旧事件缺失字段和已知空内容。
- **总是序列化 `null`**: 可行但增加输出噪音；项目现有 `Option` 字段常用 `skip_serializing_if`。

## Decision 7: 测试策略以事件协议回归和兼容性为中心

**Decision**: 测试分三层：

1. `agent_scope_event` 序列化/反序列化测试：验证新增字段存在、缺失字段兼容、空字符串与 None 区分。
2. `agent_scope_agent` 非流式路径测试：验证 Text/Thinking/ToolCall/ToolResult EndEvent 携带完整内容。
3. `agent_scope_agent` 流式路径测试：验证多 chunk 拼接、交错 block、工具调用输入累积、工具结果流式输出累积、取消/错误语义。

**Rationale**: Feature spec SC-001 到 SC-006 都是可观察事件协议要求。测试必须覆盖 event crate 的协议层和 agent crate 的事件生产层，否则只改结构不填值或只填值不兼容旧 JSON 都可能漏检。

**Alternatives considered**:
- **只做 snapshot 测试**: 可以覆盖输出形状，但对取消、空内容、交错 block 的行为定位不足。
- **只测试最终 ChatResponse**: 无法验证 EndEvent 字段和事件顺序。
