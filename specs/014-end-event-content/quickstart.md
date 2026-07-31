# Quickstart: End Event Content

**Feature**: 014-end-event-content | **Date**: 2026-07-31

## Purpose

Validate that EndEvent complete-content fields work in both streaming and non-streaming paths while preserving existing event lifecycle semantics.

## Prerequisites

- Rust workspace dependencies installed through Cargo.
- No external model provider is required for unit/contract validation; use existing mock/scripted tests where possible.
- Run commands from repository root.

## Validation Scenario 1: Event Protocol Serialization

**Goal**: Verify new optional fields are backward compatible.

**Command**:

```bash
rtk cargo test -p agent_scope_event event_serde
```

**Expected Outcomes**:

- EndEvent JSON with `text`, `thinking`, `input`, or `output` round-trips successfully.
- EndEvent JSON missing these fields deserializes successfully with `None` values.
- Empty string fields round-trip as `Some("")`, distinct from missing fields.
- Existing event constructors/tests are updated without changing event type names.

## Validation Scenario 2: Non-Streaming Text and Thinking EndEvent Content

**Goal**: Verify non-streaming model responses populate TextBlockEndEvent and ThinkingBlockEndEvent content.

**Command**:

```bash
rtk cargo test -p agent_scope_agent non_streaming_end_event_content
```

**Expected Outcomes**:

- For a complete text block, emitted sequence remains Start → Delta → End.
- TextBlockEndEvent.text equals the text delta content.
- For a complete thinking block, emitted sequence remains Start → Delta → End.
- ThinkingBlockEndEvent.thinking equals the thinking delta content.
- ReplyEnd behavior remains unchanged.

## Validation Scenario 3: Non-Streaming Tool Call and Tool Result EndEvent Content

**Goal**: Verify non-streaming tool call input and complete tool result output are copied to EndEvent fields.

**Command**:

```bash
rtk cargo test -p agent_scope_agent non_streaming_tool_end_event_content
```

**Expected Outcomes**:

- ToolCallEndEvent.input equals the ToolCallDeltaEvent.delta content for one-shot tool calls.
- ToolResultEndEvent.output equals the ToolResultTextDeltaEvent.delta content for successful complete tool output.
- Tool error paths preserve ToolResultState::Error and do not claim a successful complete output.

## Validation Scenario 4: Streaming Model Delta Accumulation

**Goal**: Verify streaming Text/Thinking/ToolCall EndEvent fields equal multi-chunk delta concatenation.

**Command**:

```bash
rtk cargo test -p agent_scope_agent streaming_end_event_content
```

**Expected Outcomes**:

- Multi-chunk text deltas concatenate into TextBlockEndEvent.text.
- Multi-chunk thinking deltas concatenate into ThinkingBlockEndEvent.thinking.
- Multi-chunk tool call input deltas concatenate into ToolCallEndEvent.input.
- Event type order and end event counts match pre-feature expectations.

## Validation Scenario 5: Streaming Tool Result Output

**Goal**: Verify tool result streams collect output and publish it on ToolResultEndEvent.

**Command**:

```bash
rtk cargo test -p agent_scope_agent streaming_tool_result_end_event_content
```

**Expected Outcomes**:

- ToolResultTextDeltaEvent chunks are emitted in order.
- ToolResultEndEvent.output equals concatenated text deltas for success.
- Interrupted tool result streams preserve `state = interrupted` and do not falsely claim complete success output.

## Validation Scenario 6: Interleaved Blocks

**Goal**: Verify active blocks do not leak content into each other.

**Command**:

```bash
rtk cargo test -p agent_scope_agent interleaved_end_event_content
```

**Expected Outcomes**:

- At least 10 interleaved block/tool ids accumulate independently.
- Each EndEvent contains only content for its own block/tool id.
- No late ToolCallDelta appears after ToolCallEnd.

## Full Workspace Regression

**Command**:

```bash
rtk cargo test
rtk cargo clippy --all-targets --all-features -- -D warnings
rtk cargo fmt --check
```

**Expected Outcomes**:

- All tests pass.
- No clippy warnings.
- Formatting check passes.
- Existing examples that listen to AgentEvent continue to compile after EndEvent constructors are updated.

## Manual Trace Review

After implementation, capture a trace from an agent run with:

1. Text response
2. Thinking content
3. Tool call input split across chunks
4. Tool result output

Confirm:

- EndEvent complete-content fields can reconstruct block-level final output.
- Reconstructing from DeltaEvents produces the same content.
- Event timestamps/order still show Start before Delta before End.
