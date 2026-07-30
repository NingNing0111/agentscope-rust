# Contract: Agent::reply_stream() — Streaming Reply API

**Feature**: 008-streaming-infrastructure
**Contract Type**: Public Trait Method
**Stability**: Stable (backward compatible with Feature 007)

## Signature (unchanged)

```rust
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    async fn reply_stream(
        &self,
        input: Option<Vec<Msg>>,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>;
}
```

## Behavioral Changes (Feature 008)

### Before (Feature 007)
- Events collected into a Vec during processing, yielded as a batch after reply completes
- First event available only after full reply (including all model calls and tool executions)
- Internally uses `broadcast` channel (events could be dropped on lag)

### After (Feature 008)
- Events yielded progressively as the model produces chunks and tools execute
- `ReplyStart` event available within 5ms of invocation
- Text delta events within 10ms of model chunk arrival
- Tool call execution begins as soon as tool call arguments are complete (not after full model response)
- Internally uses `mpsc` channel (events never dropped; backpressure on bounded channels)

## Event Sequence Guarantee

```
ReplyStart →
  (ModelCallStart → [TextBlockStart → TextBlockDelta* → TextBlockEnd]*
                 → [ToolCallStart → ToolCallDelta* → ToolCallEnd]*
                 → ModelCallEnd) →
  (ToolCallStart → ToolCallEnd → ToolResultStart → ToolResultTextDelta* → ToolResultEnd)* →
  (ModelCallStart → ... → ModelCallEnd)* →
  ... →
ReplyEnd
```

All iterations of the ReAct loop are emitted through a single continuous stream.

## Error Conditions

| Error | When |
|-------|------|
| `AgentError::NoContentToReply` | `input` is `None` and state context is empty |
| `AgentError::AlreadyStreaming` | **(NEW)** Another `reply_stream()` is still active on this agent |
| `AgentError::ModelError` | Model call fails (including mid-stream errors) |
| `AgentError::ToolError` | Tool execution fails |
| `AgentError::CancellationError` | Stream was dropped or interrupted externally |

## Stream Behavior

### Normal Completion
- Stream yields events until `ReplyEnd` is emitted
- After `ReplyEnd`, stream is fused: all subsequent polls return `Poll::Ready(None)`

### Consumer-Initiated Cancellation (Drop)
- When the stream `Pin<Box<dyn Stream>>` is dropped:
  1. Cancel signal sent to processing task
  2. Model call and tool executions are cancelled
  3. Agent's `is_streaming` flag is cleared
  4. Agent ready for new `reply()`/`reply_stream()` calls

### External Interruption (UserInterruptEvent)
- Same as Feature 007 behavior:
  1. `UserInterruptEvent` emitted
  2. In-flight operations cancelled
  3. `ReplyEnd(finished_reason=interrupted)` emitted
  4. Stream completes normally (not via Drop)

## Backward Compatibility

- Method signature unchanged
- Return type identical (`Pin<Box<dyn Stream<Item = AgentEvent> + Send>>`)
- All 47 existing agent tests pass without modification
- Existing callers that `.collect()` the stream see the same event sequence (only timing differs)
