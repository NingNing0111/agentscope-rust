# Contract: ToolExecOutput::Stream — Streaming Tool Execution

**Feature**: 008-streaming-infrastructure (US3)
**Contract Type**: Existing Enum Extension (behavioral only)
**Stability**: Stable (enum variant already exists)

## Current State

`ToolExecOutput::Stream(Pin<Box<dyn Stream<Item = Result<ToolResultBlock, ToolError>> + Send>>)` 已在 Feature 006 中定义，但在 Feature 007 的 ReAct 循环中未被使用——实际只消费 `Complete` 变体。

## New Behavior (Feature 008)

### Consumer Side (StreamingReactor)

当工具返回 `ToolExecOutput::Stream(mut stream)` 时：

```text
1. emit ToolResultStart
2. while let Some(chunk_result) = stream.next().await:
   a. on Ok(chunk): emit ToolResultTextDelta with chunk's text
   b. on Err(e): emit ToolResultEnd with state=Error
   c. check StreamHandle for cancellation
3. emit ToolResultEnd with state=Success
```

### Critical Streaming Tool Output Events

- `ToolResultStart` — tool execution begins
- `ToolResultTextDelta` × N — progressive output chunks
- `ToolResultEnd` — tool execution complete (state = Success | Error | Interrupted)

### Backward Compatibility

- `ToolExecOutput::Complete` 产生相同的事件序列：`ToolResultStart` → 单个 `ToolResultTextDelta` → `ToolResultEnd`
- 现有工具实现无需修改
- 工具 trait 签名不变

### Cancellation

- 检查 `StreamHandle` 的 cancel signal
- 如果 stream 被 drop，中止 `while let` 循环，emit `ToolResultEnd(state=Interrupted)`

## Tool Implementor Contract

流式工具 MUST:
- 产出标记 `is_last: true` 的 `ToolResultBlock` 作为最终 chunk
- 响应 cancellation（stream 的 `next()` 在 reactor 停止 polling 后不会再被调用）
- 不产出超过预期数量的 chunk（无硬限制，但超大流会阻塞事件管道）

流式工具 MAY:
- 在中间 chunk 设置 `is_last: false`
- 使用 `ToolResultBlock::state` 在最终 chunk 标记结果状态
