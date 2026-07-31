# Data Model: End Event Content

**Feature**: 014-end-event-content | **Date**: 2026-07-31

## Overview

This feature extends existing block lifecycle end events with optional complete-content snapshots. The model preserves the existing Start → Delta → End lifecycle while allowing event consumers to read final block content directly from EndEvent.

## Entities

### TextBlockEndEvent

**Purpose**: Marks the end of a text block lifecycle and optionally carries the complete text accumulated for that block.

**Fields**:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `base` | EventBase | Yes | Common event metadata |
| `reply_id` | String | Yes | Reply this block belongs to |
| `block_id` | String | Yes | Text block identifier |
| `text` | Option<String> | No | Complete text content for this block |

**Validation Rules**:

- `block_id` MUST match the preceding TextBlockStartEvent and TextBlockDeltaEvent block id.
- `text = Some(value)` MUST equal the concatenation of all TextBlockDeltaEvent.delta values for the same block in observable order.
- `text = Some("")` means the block is known to have complete empty text.
- `text = None` means complete text is unknown or unavailable.

### ThinkingBlockEndEvent

**Purpose**: Marks the end of a thinking block lifecycle and optionally carries the complete thinking text accumulated for that block.

**Fields**:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `base` | EventBase | Yes | Common event metadata |
| `reply_id` | String | Yes | Reply this block belongs to |
| `block_id` | String | Yes | Thinking block identifier |
| `thinking` | Option<String> | No | Complete thinking content for this block |

**Validation Rules**:

- `block_id` MUST match the preceding ThinkingBlockStartEvent and ThinkingBlockDeltaEvent block id.
- `thinking = Some(value)` MUST equal the concatenation of all ThinkingBlockDeltaEvent.delta values for the same block in observable order.
- `thinking = Some("")` means the block is known to have complete empty thinking content.
- `thinking = None` means complete thinking content is unknown or unavailable.

### ToolCallEndEvent

**Purpose**: Marks the end of a tool call input lifecycle and optionally carries the complete input accumulated for that tool call.

**Fields**:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `base` | EventBase | Yes | Common event metadata |
| `reply_id` | String | Yes | Reply this tool call belongs to |
| `tool_call_id` | String | Yes | Tool call identifier |
| `input` | Option<String> | No | Complete tool input for this call |

**Validation Rules**:

- `tool_call_id` MUST match the preceding ToolCallStartEvent and ToolCallDeltaEvent tool_call_id.
- `input = Some(value)` MUST equal the concatenation of all ToolCallDeltaEvent.delta values for the same tool call in observable order.
- `input = Some("")` means the tool input is known to be complete and empty.
- `input = None` means complete input is unknown or unavailable.
- Late deltas after ToolCallEndEvent remain invalid and must not be introduced by this feature.

### ToolResultEndEvent

**Purpose**: Marks the end of a tool result lifecycle and optionally carries the complete observable output accumulated for that tool result.

**Fields**:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `base` | EventBase | Yes | Common event metadata |
| `reply_id` | String | Yes | Reply this tool result belongs to |
| `tool_call_id` | String | Yes | Tool call whose result this is |
| `state` | ToolResultState | Yes | Result terminal state |
| `metadata` | HashMap<String, Value> | Yes (default empty) | Tool result metadata |
| `output` | Option<String> | No | Complete observable output for this tool result |

**Validation Rules**:

- `tool_call_id` MUST match the preceding ToolResultStartEvent and ToolResult*DeltaEvent tool_call_id.
- `output = Some(value)` MUST equal the concatenation of all observable text output deltas for the same tool result in order.
- `output = Some("")` means the tool result is known to have complete empty output.
- `output = None` means complete output is unknown, unavailable, or intentionally withheld because the result did not complete successfully.
- For `state = Interrupted`, output MUST NOT falsely represent a successful complete result.

### BlockContentAccumulator

**Purpose**: Transient lifecycle state used by event producers to populate EndEvent complete-content fields.

**Fields**:

| Field | Type | Description |
|-------|------|-------------|
| `reply_id` | String | Reply containing the active blocks |
| `text_blocks` | Map<BlockId, Vec<String>> | Text deltas by block |
| `thinking_blocks` | Map<BlockId, Vec<String>> | Thinking deltas by block |
| `tool_inputs` | Map<ToolCallId, String> | Tool input accumulated by tool call |
| `tool_outputs` | Map<ToolCallId, String> | Tool result output accumulated by tool call |

**Validation Rules**:

- Accumulators are scoped to one reply lifecycle.
- Each active block/tool id accumulates independently.
- Content is appended only after the corresponding delta is emitted or in the same producer operation that emits the delta.
- Accumulator state is cleared when the corresponding EndEvent is emitted.

### EventConsumer

**Purpose**: Downstream subscriber that reads AgentEvent streams.

**Rules**:

- Consumers MAY read complete content from EndEvent fields when present.
- Consumers MAY continue to read DeltaEvent streams and concatenate them manually.
- Consumers MUST treat missing EndEvent content fields as unknown/unavailable, not as protocol failure.
- Consumers MUST NOT assume EndEvent content replaces DeltaEvent ordering semantics.

## State Transitions

### Normal streaming text/thinking block

```text
NoActiveBlock
  -> StartEmitted(block_id)
  -> DeltasAccumulating(block_id, content*)
  -> EndEmitted(block_id, complete_content)
  -> Closed
```

### Normal streaming tool call

```text
NoActiveToolCall
  -> ToolCallStartEmitted(tool_call_id)
  -> ToolInputAccumulating(tool_call_id, input*)
  -> ToolCallEndEmitted(tool_call_id, complete_input)
  -> ToolReadyForExecution
```

### Normal tool result

```text
NoActiveToolResult
  -> ToolResultStartEmitted(tool_call_id)
  -> ToolOutputAccumulating(tool_call_id, output*)
  -> ToolResultEndEmitted(tool_call_id, state, complete_output)
  -> Closed
```

### Cancellation/error path

```text
ActiveBlockOrTool
  -> CancellationOrErrorObserved
  -> Existing ReplyEnd/error/tool state emitted
  -> EndEvent content omitted or explicitly unknown unless complete content is known
```

## Serialization Semantics

- New complete-content fields are optional.
- Missing fields deserialize as `None`.
- `None` fields are omitted when serializing.
- Empty strings serialize as `""` and must remain distinguishable from missing fields.
- Existing JSON without the new fields remains valid.
