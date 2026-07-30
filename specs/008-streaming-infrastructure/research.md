# Research: Streaming Infrastructure

**Feature**: 008-streaming-infrastructure
**Date**: 2026-07-29

## Decision 1: Channel Type — mpsc vs broadcast

**Context**: 当前 `EventEmitter` 使用 `tokio::sync::broadcast` channel。broadcast 在被慢消费者追上时会丢弃旧事件（lagged），这违反了 FR-018（不丢事件）和宪法 Article 7（trace 完整性）。此外，broadcast 不提供反压机制——如果 channel 满，emit 方不会阻塞。

**Decision**: 将 `EventEmitter` 的内部 channel 从 `tokio::sync::broadcast` 替换为 `tokio::sync::mpsc`。

**Rationale**:
- mpsc 支持有界容量和阻塞反压（`send().await` 在 channel 满时暂停）
- mpsc 永不会丢弃事件——每条消息都被传递
- 与 `futures::Stream` 有顶级的 `UnboundedReceiverStream` / `ReceiverStream` 适配器
- 每个 `reply_stream()` 调用创建自己的 mpsc channel，而非共享一个 broadcast

**Alternatives considered**:
- `broadcast` + 大容量：不能根本解决丢事件问题，lagged 时仍会丢
- `flume`：第三方 crate，无 tokio 原生优势，功能与 mpsc 类似
- `tokio::sync::watch`：仅保持最新值，不适合事件序列

**Impact**: `EventEmitter` API 从 `subscribe() -> broadcast::Receiver` 变为 `subscribe() -> mpsc::Receiver`。`reply_stream()` 直接消费 mpsc::Receiver。不再支持多个 `reply_stream()` 同时订阅同一 reply（这本来就不允许，见 FR-023）。

---

## Decision 2: Progressive Stream Processing Architecture

**Context**: 当前 `react_loop.rs` 的 `run_react_loop()` 函数在获取模型响应时使用 accumulate-then-process 模式：
1. 调用 `model.call()` 获取 `ModelCallResult`
2. 如果是 Stream，用 `StreamAccumulator` 累积所有 chunk
3. 调用 `classify_response()` 和 `match outcome` 决定下一步

在流式模式下，需要在每个 chunk 到达时立即处理并 emit 事件，同时检测工具调用完成度。

**Decision**: 创建 `streaming_reactor.rs` 模块，实现 `run_streaming_loop()` 函数。核心逻辑：

```text
for each model stream chunk:
  1. emit ModelCallStart (first chunk only)
  2. for each content block in chunk:
     a. accumulate into per-block buffer (StreamAccumulator per block id)
     b. emit corresponding AgentEvent (TextBlockDelta, ToolCallDelta, etc.)
     c. detect block completion or transition
  3. on stream end OR block-type transition:
     a. if tool call blocks are complete → execute them immediately
     b. feed tool results back to model → continue stream loop
     c. if text blocks → emit ReplyEnd and finish
```

保留现有 `run_react_loop()` 用于 `reply()` 路径（它内部调用 `streaming_reactor` 但累积所有事件再返回最终 Msg）。

**Rationale**:
- 分离关注点：旧 `react_loop.rs` 保持稳定（向后兼容），新 `streaming_reactor.rs` 承担渐进式处理的复杂性
- 复用 `StreamAccumulator` 的逐 block 累积能力（已在 Feature 003 实现）
- 工具调用完成检测使用 FR-011 的简单启发式（block type 切换或流结束）

**Alternatives considered**:
- 修改现有 `run_react_loop` 添加 if/else 分支：复杂度爆炸，维护困难
- 完全替换 `run_react_loop`：风险高，破坏向后兼容
- 使用状态机 trait：过度设计，ReAct 循环结构已固定

---

## Decision 3: Tool Call Completion Detection Heuristic

**Context**: 在流式模式下，模型可能在一个 chunk 中发送部分工具调用参数。例如：
- Chunk 1: `ToolCallBlock { id: "tc1", name: "calc", input: '{"a":1' }`
- Chunk 2: `ToolCallBlock { id: "tc1", name: "", input: ',"b":2}' }`
- Chunk 3: `TextBlock { id: "t1", text: "Now computing..." }`

需要判断工具调用何时完成（参数接收完毕），才能执行工具。

**Decision**: 使用以下两个条件之一判定工具调用完成：
1. 模型流结束（`is_last == true`）
2. 同一模型响应中，流开始发送不同类型的 content block（如从 ToolCallBlock 切换到 TextBlock）

**Rationale**:
- 简单、无状态、不依赖 JSON 解析（参数可能是 incomplete JSON）
- 与 OpenAI streaming API 行为一致（同一 tool call 的所有片段共享相同 block id，跨 block type 切换表示完成）
- 不引入额外解析开销

**Alternatives considered**:
- JSON 完整性检测（尝试 `serde_json::from_str` 直到成功）：在参数就是非 JSON（如 Python 代码）时失败；且 JSON 可能在中间某步恰好合法（误报）
- 等待 N 个 chunk 无变化：引入不可靠的定时器
- 依赖 `is_last` 字段：延迟大，强制等待整个模型响应结束

---

## Decision 4: reply() 内部实现策略

**Context**: `reply()` 必须保持向后兼容——返回 `Result<Msg, AgentError>`，而非 stream。但在内部，为减少代码重复，可以让 `reply()` 也使用流式管道，只是最终累积成单个 `Msg`。

**Decision**: `reply()` 内部复用 `run_streaming_loop()` 但将事件收集到 `Vec<AgentEvent>` 中，最后从事件提取最终 `Msg`。

**Rationale**:
- 单一代码路径，降低维护成本
- `reply()` 的行为与 `reply_stream()` 的事件序列完全一致（仅交付方式不同）
- 保证 SC-007（事件类型和顺序一致）

**Alternatives considered**:
- 保留 `run_react_loop()` 作为 `reply()` 专用路径：代码重复，违反 DRY，可能导致行为分歧

---

## Decision 5: Cancellation on Stream Drop

**Context**: FR-004 要求消费者丢弃 stream 时取消底层模型调用和工具执行。FR-020 要求 drop 触发取消。

**Decision**: 使用 `tokio::sync::oneshot` channel 作为取消信号。`EventStream` 的 `Drop` 实现发送 oneshot 信号。`run_streaming_loop()` 在每次模型调用前和工具执行前检查 oneshot 是否已关闭。

```text
EventStream {
    rx: mpsc::Receiver<AgentEvent>,
    cancel_tx: Option<oneshot::Sender<()>>,  // fires on Drop
}
```

**Rationale**:
- oneshot 是 Rust 中最轻量的取消信号机制（单次触发、零开销检测）
- Drop 实现确保即使 stream 被 `mem::forget` 也能正常清理（oneshot sender 被 drop 时 receiver 侧 `.await` 返回 `Err(Closed)`）
- 与现有 `AtomicBool interrupted` 互补：UserInterruptEvent 通过 interrupt flag，stream drop 通过 oneshot

**Alternatives considered**:
- `CancellationToken`：功能完备但更重，对单次取消场景 overkill
- 仅依赖 `AtomicBool`：不能通过 drop 自动触发（需要显式调用 interrupt）

---

## Decision 6: Bounded Channel Configuration

**Context**: 用户需要一个地方配置 channel 容量。不应该引入新的全局配置对象。

**Decision**: 将 `stream_channel_capacity: Option<usize>` 添加到 `AgentConfig` 中（在 Feature 007 中已定义）。`None` = 无界（默认），`Some(N)` = 有界 channel 容量为 N。

**Rationale**:
- 最小化配置表面积——不新建配置类型
- 与 FR-017（可配置容量）和 FR-019（默认无界）一致
- `AgentConfig` 使用 builder 模式，容易向后兼容地添加新字段

---

## Decision 7: AlreadyStreaming Guard Implementation

**Context**: FR-023 禁止在活跃 stream 存在时发起新的 `reply()` 或 `reply_stream()` 调用。需要一个轻量级的并发保护机制。

**Decision**: 在 `AgentInner` 中添加 `AtomicBool` (`is_streaming`)。`reply()` 和 `reply_stream()` 在入口处检查并设置此标志。当 stream 被 drop（或自然结束）时，`EventStream::Drop` 清除标志。

不使用 `tokio::sync::Mutex` 或 `RwLock` 因为仅需保护一个 boolean。

**Rationale**:
- `AtomicBool` 是最快、最简单的并发原语
- `compare_exchange` 提供无锁 check-and-set
- 与现有 `interrupted: AtomicBool` 模式一致

**Alternatives considered**:
- `Mutex<()>` lock holder：过度，引入死锁风险
- 完全无保护（允许并发）：状态冲突风险（见 Edge Cases）

## Summary of All Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | mpsc 替代 broadcast | 反压支持 + 不丢事件 |
| 2 | 新 streaming_reactor 模块 | 关注点分离 + 向后兼容 |
| 3 | Block-type 切换 = 工具调用完成 | 简单、无 JSON 解析依赖 |
| 4 | reply() 复用流式管道 | 单一代码路径 + SC-007 |
| 5 | oneshot 用于 stream drop 取消 | 最轻量 + Drop 安全 |
| 6 | AgentConfig 添加 channel capacity | 最小配置表面积 |
| 7 | AtomicBool 保护并发 | 最快最简单 |
