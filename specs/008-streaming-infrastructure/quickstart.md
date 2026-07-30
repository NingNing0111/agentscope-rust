# Quickstart: Streaming Infrastructure Validation

**Feature**: 008-streaming-infrastructure
**Date**: 2026-07-29

## Prerequisites

- Rust toolchain (stable 1.75+)
- All existing tests pass: `cargo test -p agent_scope_agent`
- All existing tests pass: `cargo test` (workspace-wide)

## Validation Scenarios

### Scenario 1: Real-Time Text Streaming (US1)

**Goal**: Verify that `reply_stream()` yields text events progressively, not after accumulation.

```bash
# Run the real-time streaming tests
cargo test -p agent_scope_agent --test streaming_tests test_streaming_mock_model_produces_correct_text
cargo test -p agent_scope_agent --test streaming_tests test_streaming_progressive_events
```

**Expected outcome**: First event (ReplyStart) arrives before model completes. TextBlockDelta events arrive interleaved with model chunk arrival, not all at once at the end.

**Manual validation snippet** (conceptual):
```rust
// Create agent with MockModel that streams 3 chunks
let model = Arc::new(MockModel::new("mock", "Hello World").with_stream(3));
let agent = ReActAgent::new(config, ReActConfig::default(), ContextConfig::default(), vec![])?;

let mut stream = agent.reply_stream(Some(vec![user_msg("user", "hi")?])).await?;

// First event should arrive quickly (ReplyStart)
let first = stream.next().await.unwrap();
assert!(matches!(first, AgentEvent::ReplyStart(_)));

// Subsequent events arrive as model chunks are processed
// ... TextBlockStart, TextBlockDelta("Hel"), TextBlockDelta("lo"), ...
```

### Scenario 2: Streaming Tool Call Detection (US2)

**Goal**: Verify tool calls are detected and executed mid-stream, not after full response.

```bash
# Run tool call detection tests
cargo test -p agent_scope_agent --test streaming_tests test_streaming_tool_call_detection
```

**Expected outcome**: ToolCallStart → ToolCallDelta → ToolCallEnd events appear before the subsequent TextBlockDelta, and tool execution begins before the full model stream ends.

### Scenario 3: Streaming Tool Execution (US3)

**Goal**: Verify tools with `ToolExecOutput::Stream` output progressive deltas.

```bash
cargo test -p agent_scope_agent --test streaming_tests test_streaming_tool_execution
```

**Expected outcome**: ToolResultStart → ToolResultTextDelta × N → ToolResultEnd events emitted as tool produces output.

### Scenario 4: Backpressure (US4)

**Goal**: Verify bounded channel blocks emitter when full.

```bash
cargo test -p agent_scope_agent --test streaming_tests test_bounded_channel_backpressure
```

**Expected outcome**: When consumer is slow, emission blocks (not drops events). No events lost. Memory bounded.

### Scenario 5: Backward Compatibility

**Goal**: Verify all existing tests pass without modification.

```bash
# All existing agent tests must pass
cargo test -p agent_scope_agent

# All workspace tests must pass
cargo test
```

**Expected outcome**: All 47 pre-existing agent tests pass. 0 failures. 0 test modifications needed.

### Scenario 6: Cancellation on Stream Drop

**Goal**: Verify dropping stream cancels underlying work.

```bash
cargo test -p agent_scope_agent --test streaming_tests test_stream_drop_cancellation
```

**Expected outcome**: After stream drop, is_streaming flag is cleared within 50ms. New `reply()` call succeeds immediately after.

### Scenario 7: Concurrent Call Protection

**Goal**: Verify AlreadyStreaming error when calling reply() during active stream.

```bash
cargo test -p agent_scope_agent --test streaming_tests test_concurrent_reply_streaming_guard
```

**Expected outcome**: Second `reply_stream()` call returns `Err(AgentError::AlreadyStreaming)`.

### Scenario 8: Clippy & Format

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

**Expected outcome**: 0 warnings, 0 format errors.

## Full Integration Check

```bash
# Complete validation sequence
cargo test -p agent_scope_agent                 # Unit + integration tests for agent crate
cargo clippy --all-targets -- -D warnings       # Lint check
cargo fmt --all -- --check                      # Format check
```
