# Feature Specification: Streaming Infrastructure

**Feature Branch**: `008-streaming-infrastructure`

**Created**: 2026-07-29

**Status**: Draft

**Input**: User description: "实现feature 8，streaming — 将现有的 accumulate-then-process 模式升级为真正的实时流式管道，使模型输出的 chunk 能够实时转发给调用方，支持流式工具调用检测、流式工具执行、背压控制和取消传播。"

## Clarifications

### Session 2026-07-29

- Q: ReAct 多轮迭代在流中如何呈现？ → A: 单一连续流 — `ReplyStart` → (ModelCallStart → chunks → ModelCallEnd) → (ToolCallStart → ... → ToolResultEnd) → (ModelCallStart → chunks → ModelCallEnd) → ... → `ReplyEnd`。所有迭代事件通过同一个 stream 实时推送
- Q: 有界 channel 满时的反压策略？ → A: 仅阻塞 (Block only)。移除 `DropOldest` 选项，因为丢弃事件违反事件完整性要求（FR-003）和宪法 Article 7 的 trace 完整性原则
- Q: 并发 reply_stream() 调用的行为？ → A: 返回错误 — 后续调用立即返回 `AgentError::AlreadyStreaming`，调用方必须先丢弃/消费完当前 stream 才能发起新调用

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Real-Time Event Streaming to Callers (Priority: P1)

A developer calls `agent.reply_stream(user_msg)` and wants to receive agent events (text deltas, tool call starts, etc.) in real-time as the model produces them, not after the entire model response has been accumulated. Currently, `reply_stream()` internally collects all events into a Vec and yields them only after the full reply completes — the developer sees all events at once rather than progressively.

**Why this priority**: Real-time streaming is the fundamental value proposition of this feature. Without it, `reply_stream()` offers no meaningful difference from `reply()` — both block until the full response is ready. This is the MVP.

**Independent Test**: Create a ReActAgent with a MockModel in streaming mode (3 chunks over time). Call `reply_stream()` and poll the stream. Verify that events arrive progressively across multiple poll points (not all at once at the end), and that the order matches the AgentScope protocol.

**Acceptance Scenarios**:

1. **Given** a ReActAgent with a mock model that streams text in 3 chunks ("Hel" → "lo " → "World"), **When** the developer calls `agent.reply_stream(user_msg)` and polls the stream, **Then** the stream yields `ReplyStart` immediately, then `ModelCallStart`, then `TextBlockStart` → `TextBlockDelta("Hel")` → `TextBlockDelta("lo ")` → `TextBlockDelta("World")` → `TextBlockEnd` → `ModelCallEnd` → `ReplyEnd`, with each event arriving as soon as the model chunk is processed, not after accumulation.
2. **Given** a ReActAgent with a non-streaming model, **When** `agent.reply_stream(user_msg)` is called, **Then** the stream still yields events progressively in the correct order (the entire text arrives as a single delta), and the behavior is indistinguishable from `reply()` in event content, only differing in delivery timing.
3. **Given** a streaming reply in progress, **When** the caller drops the stream, **Then** the underlying model call is cancelled and no further events are produced.

---

### User Story 2 - Streaming Tool Call Detection (Priority: P2)

When a model streams its response, tool calls may arrive in fragments (tool name in one chunk, partial JSON arguments in subsequent chunks). The agent needs to detect when a tool call is "complete" (all argument chunks received) and begin execution without waiting for the entire model response to finish — especially important when the model produces a tool call followed by text in the same stream.

**Why this priority**: Tool use is central to agent functionality. If the agent cannot detect and act on tool calls until the full response is accumulated, the streaming experience degrades to batch mode for tool-using agents. This builds on US1.

**Independent Test**: Use a mock model that streams: first `ToolCallStart(chunk1: name="calc")` → `ToolCallDelta(chunk2: args='{"a":1')` → `ToolCallDelta(chunk3: args=',"b":2}')` → `TextDelta(chunk4: "Now computing...")`. Verify the agent starts executing the tool call as soon as chunk3 arrives (tool call is complete), without waiting for chunk4.

**Acceptance Scenarios**:

1. **Given** a model streaming a tool call across 3 chunks (tool name, partial args, final args) followed by a text delta, **When** the agent processes the stream, **Then** it emits `ToolCallStart` → `ToolCallDelta` → `ToolCallDelta` events in real-time, detects the tool call is complete when the final args chunk arrives, executes the tool, and then processes the subsequent text delta — all without accumulating the full model response first.
2. **Given** a model that streams multiple tool calls interleaved with text ("I'll search for that" → `ToolCall("search", "{q:...}")` → "Found results"), **When** the agent processes the stream, **Then** all content blocks are emitted in order, and each tool call is executed as soon as its argument stream completes.
3. **Given** a model streaming a tool call with malformed JSON in the arguments, **When** the tool call's argument stream completes, **Then** the agent emits `ToolCallEnd` with the raw arguments and execution fails with a clear error (not silently ignored).

---

### User Story 3 - Streaming Tool Execution (Priority: P3)

Some tools produce output incrementally (e.g., a search tool that streams result snippets, a code interpreter that streams execution output). The agent should support tools that return streaming output, forwarding deltas to the caller as they arrive.

**Why this priority**: Streaming tool output is important for responsiveness in tools with long execution times, but it builds on US1 and US2. A developer can always wrap a streaming tool as a batch tool if this feature isn't available.

**Independent Test**: Register a tool that returns `ToolExecOutput::Stream(...)` yielding 3 text chunks. When the agent executes this tool, verify that `ToolResultStart` → `ToolResultTextDelta` × 3 → `ToolResultEnd` events are emitted progressively through the stream.

**Acceptance Scenarios**:

1. **Given** a ReActAgent with a streaming tool that yields output in 3 chunks, **When** the agent executes this tool during a `reply_stream()` call, **Then** the caller receives `ToolResultStart` → `ToolResultTextDelta` → `ToolResultTextDelta` → `ToolResultTextDelta` → `ToolResultEnd` in real-time, with each delta arriving as the tool produces it.
2. **Given** a streaming tool that fails mid-execution after yielding 2 chunks, **When** the tool produces an error, **Then** the agent emits `ToolResultEnd` with state=error and an error message, and the error is fed back to the model if `max_iters` allows.
3. **Given** a streaming tool execution that the user interrupts via `UserInterruptEvent`, **When** the interrupt arrives, **Then** the tool stream is cancelled via the cancellation mechanism, and the agent emits `ToolResultEnd` with interrupted state.

---

### User Story 4 - Backpressure and Flow Control (Priority: P4)

When a caller of `reply_stream()` consumes events slowly, the streaming pipeline should apply backpressure back through the agent to the model provider — preventing unbounded memory growth from event buffering.

**Why this priority**: Production deployments need predictable resource usage. However, backpressure is a refinement that can be added after the core streaming pipeline is validated. The default unbounded channel approach is acceptable for MVP (US1-US3).

**Independent Test**: Create a slow consumer that takes 100ms between stream polls. Configure a bounded channel between the agent's event emitter and the stream consumer. Verify that when the channel is full, the agent pauses processing (does not consume more model chunks) until the consumer catches up.

**Acceptance Scenarios**:

1. **Given** a ReActAgent with a bounded event channel of capacity 16, **When** a slow consumer polls the stream with 1-second delays and the model produces chunks much faster, **Then** the agent's event emission is blocked/paused when the channel is full, and resumes when the consumer catches up — no events are lost and memory usage stays bounded.
2. **Given** a ReActAgent with an unbounded event channel (default), **When** the model produces events faster than the consumer polls, **Then** all events are buffered and eventually delivered in order — no events are lost, at the cost of potentially higher memory usage.

---

### Edge Cases

- What happens when a streaming model produces an error mid-stream (e.g., API connection drops after 2 of 5 chunks)?
- What happens when a tool call's arguments span across more chunks than expected (e.g., very large JSON argument)?
- What happens when the model streams a ContentBlock type the agent doesn't recognize?
- What happens when the caller polls the stream after the stream has already ended (fused stream behavior)?
- What happens when context compression is triggered mid-stream?
- What happens when a tool call is detected as complete but the model then sends additional argument chunks for the same tool call (model hallucination in streaming)?
- How does the system handle a model that sends `is_last=true` mid-stream then sends more chunks?
- What happens when a second `reply()` or `reply_stream()` call is made while a previous `reply_stream()` is still active (stream not dropped or fully consumed)?

## Requirements *(mandatory)*

### Functional Requirements

**Real-time event streaming**:

- **FR-001**: `reply_stream()` MUST yield events to the caller progressively as the model produces chunks, not after the full response is accumulated.
- **FR-002**: The stream returned by `reply_stream()` MUST implement `futures::Stream<Item = AgentEvent>` and be fused (yield `None` after `ReplyEnd`).
- **FR-003**: Event emission order in streaming mode MUST match the AgentScope protocol: `ReplyStart` → ((`ModelCallStart` → content block events* → `ModelCallEnd`) → (`ToolCallStart` → ... → `ToolResultEnd`)*)⁺ → `ReplyEnd`. All iterations of the ReAct loop (model calls + tool executions) are emitted through a single continuous stream without interruption.
- **FR-004**: The consumer dropping the stream MUST trigger cancellation of the underlying model call and any in-flight tool executions.
- **FR-005**: `reply_stream()` with a non-streaming model MUST produce the same sequence of events as with a streaming model, just delivered in a single burst.

**Streaming model integration**:

- **FR-006**: When the `ChatModel` returns `ModelCallResult::Stream`, the agent MUST forward each chunk's content blocks as real-time events without waiting for the stream to complete.
- **FR-007**: The agent MUST handle `ChatResponse` chunks where `is_last` indicates stream termination, and emit `ModelCallEnd` only after the final chunk.
- **FR-008**: If a model stream produces an error, the agent MUST emit an error event (not crash) and terminate the stream with `ReplyEnd(finished_reason=error)`.
- **FR-009**: The agent MUST preserve the `ChatUsage` from the final streaming chunk (typically the last chunk carries the token counts).

**Streaming tool call handling**:

- **FR-010**: The agent MUST accumulate partial `ToolCallBlock` chunks within a single model stream, tracking tool call completion by the end of argument streaming.
- **FR-011**: A tool call MUST be considered complete and ready for execution when either: (a) the model stream ends, or (b) the model starts streaming a different content block type after the tool call's arguments.
- **FR-012**: The agent MUST emit `ToolCallStart` when a tool call block is first seen, `ToolCallDelta` events for argument chunks, and `ToolCallEnd` when the tool call is complete.
- **FR-013**: The agent MUST NOT wait for the entire model response to finish before executing tool calls — tool execution begins as soon as individual tool calls are detected as complete.

**Streaming tool execution**:

- **FR-014**: When a tool returns `ToolExecOutput::Stream`, the agent MUST forward the tool's output deltas as `ToolResultTextDelta` events in real-time.
- **FR-015**: Tools that return `ToolExecOutput::Complete` MUST produce a single `ToolResultTextDelta` followed by `ToolResultEnd` (backward compatible).
- **FR-016**: On tool execution error during streaming, the agent MUST emit `ToolResultEnd` with an error state and feed the error back to the model context.

**Backpressure and flow control**:

- **FR-017**: The event channel between agent processing and stream consumer MUST support configurable capacity (bounded or unbounded).
- **FR-018**: When a bounded channel is full, event emission MUST block (applying backpressure through to model consumption) until space is available. Event dropping is NOT permitted — every event MUST be delivered to the consumer to preserve trace completeness.
- **FR-019**: The default configuration MUST use an unbounded channel (preserving backward compatibility with existing behavior) but allow users to opt into bounded channels for memory-constrained environments.

**Cancellation and cleanup**:

- **FR-020**: Dropping the stream returned by `reply_stream()` MUST cancel the in-progress model call and tool executions, and drop the underlying task.
- **FR-021**: After stream cancellation, the agent MUST be able to accept new `reply()` or `reply_stream()` calls (clean state recovery).
- **FR-022**: A `UserInterruptEvent` during streaming MUST produce the same behavior as in non-streaming mode: emit `UserInterruptEvent`, cancel in-flight operations, emit `ReplyEnd(interrupted)`.
- **FR-023**: Calling `reply()` or `reply_stream()` while a previous `reply_stream()` is still active (stream not fully consumed or dropped) MUST return `AgentError::AlreadyStreaming`. The caller MUST finish or cancel the active stream before initiating a new reply.

### Key Entities

- **EventStream**: The async stream object returned by `reply_stream()`. Wraps the internal channel receiver and implements `futures::Stream<Item = AgentEvent>`. Supports cancellation via Drop.
- **StreamingReactor**: The internal streaming variant of the React loop. Replaces the accumulate-then-classify pattern with progressive processing: model chunks arrive → events emitted immediately → tool calls detected on-the-fly → tool execution interleaved with model streaming.
- **StreamChannelConfig**: Configuration for the event channel: `capacity` (None = unbounded, Some(N) = bounded with N slots). Only Block backpressure strategy is supported — events are never dropped to preserve trace integrity.
- **ToolExecOutput** (extended): Currently has `Complete(ToolExecChunk)`; this feature adds a `Stream(Pin<Box<dyn Stream<Item = Result<ToolExecChunk, ToolError>> + Send>>)` variant for tools that produce output incrementally.
- **StreamHandle**: A cancellation handle tied to the stream's lifetime. When the stream is dropped, the handle signals cancellation to the agent's background processing task.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A caller polling `reply_stream()` receives the first event (ReplyStart) within 5ms of invocation, before any model API call completes.
- **SC-002**: Text delta events arrive at the caller within 10ms of the model chunk being received by the agent (measured with mock models producing chunks at 1ms intervals).
- **SC-003**: A tool call spanning 3 argument chunks is detected as complete and begins execution within 5ms of the final argument chunk arriving — no waiting for additional model chunks.
- **SC-004**: Dropping the stream triggers cancellation of the underlying model call within 50ms.
- **SC-005**: Memory usage in a long-running stream with a slow consumer stays under 2× the bounded channel capacity when bounded mode is configured.
- **SC-006**: All existing Agent tests (47 tests in agent_scope_agent) continue to pass without modification after the streaming refactor (API backward compatibility).
- **SC-007**: The full ReAct agent event sequence in streaming mode (event types and ordering) matches the non-streaming mode exactly — only timing and delivery mechanism differ.

## Assumptions

- The `StreamAccumulator` in `agent_scope_model` will be reused internally for accumulating tool call arguments across streaming chunks, but the top-level agent loop will not accumulate the entire model response before acting on it.
- `MockModel` already supports `with_stream(chunks)` for testing. This feature may extend MockModel to support more granular streaming scenarios (e.g., streaming tool calls) but MockModel enhancements are test infrastructure, not product deliverables.
- The `ToolKit` and tool execution infrastructure in `agent_scope_tool` will need a minor extension to support `ToolExecOutput::Stream`, but the core tool system design remains unchanged.
- The existing `reply()` method (non-streaming) MUST continue to work identically — internally it uses the same streaming infrastructure but accumulates all events before returning the final `Msg`.
- Backward compatibility with Feature 007's `reply_stream()` signature: the existing `Agent::reply_stream()` trait method signature (`async fn reply_stream(...) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>`) is preserved, but the internal implementation changes from "collect-then-yield" to "progressive yield."
- Streaming structured output (via `ChatModel::generate_structured_output`) is deferred to a future feature. The current limitation of returning an error for streaming structured output remains.
- The Python AgentScope reference implementation streams events in the same order as defined by the event protocol. This feature maintains compatibility with that ordering.
- All streaming events are delivered over an in-process channel (tokio mpsc). Cross-process or network streaming is out of scope.
