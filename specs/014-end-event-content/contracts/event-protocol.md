# Event Protocol Contract: End Event Content

**Feature**: 014-end-event-content | **Date**: 2026-07-31

## Contract Scope

This contract defines the externally observable event protocol extension for EndEvent complete-content fields. It applies to AgentEvent producers and consumers for streaming and non-streaming execution paths.

## Compatibility Rules

1. Existing event types remain unchanged.
2. Existing Start → Delta → End ordering remains unchanged.
3. Existing DeltaEvent payloads remain unchanged and continue to be emitted.
4. New complete-content fields are optional.
5. Consumers must accept EndEvent JSON that omits these fields.
6. Producers must not emit EndEvent content that contradicts the DeltaEvent sequence observed for the same block/tool id.

## Text Block Events

### TextBlockEndEvent

```json
{
  "event_type": "text_block_end",
  "reply_id": "reply-001",
  "block_id": "text-001",
  "text": "Hello world"
}
```

**Field Semantics**:

| Field | Semantics |
|-------|-----------|
| `text` omitted | Complete text is unknown or unavailable |
| `"text": ""` | Complete text is known and empty |
| `"text": "..."` | Complete text for this block |

**Sequence Contract**:

```text
TextBlockStart(block_id=A)
TextBlockDelta(block_id=A, delta="Hel")
TextBlockDelta(block_id=A, delta="lo")
TextBlockEnd(block_id=A, text="Hello")
```

The `text` value must equal `"Hel" + "lo"`.

## Thinking Block Events

### ThinkingBlockEndEvent

```json
{
  "event_type": "thinking_block_end",
  "reply_id": "reply-001",
  "block_id": "thinking-001",
  "thinking": "I should calculate first."
}
```

**Field Semantics**:

| Field | Semantics |
|-------|-----------|
| `thinking` omitted | Complete thinking content is unknown or unavailable |
| `"thinking": ""` | Complete thinking content is known and empty |
| `"thinking": "..."` | Complete thinking content for this block |

**Sequence Contract**:

```text
ThinkingBlockStart(block_id=T)
ThinkingBlockDelta(block_id=T, delta="I should ")
ThinkingBlockDelta(block_id=T, delta="calculate first.")
ThinkingBlockEnd(block_id=T, thinking="I should calculate first.")
```

## Tool Call Events

### ToolCallEndEvent

```json
{
  "event_type": "tool_call_end",
  "reply_id": "reply-001",
  "tool_call_id": "call-001",
  "input": "{\"expression\":\"5678 * 345\"}"
}
```

**Field Semantics**:

| Field | Semantics |
|-------|-----------|
| `input` omitted | Complete tool input is unknown or unavailable |
| `"input": ""` | Complete tool input is known and empty |
| `"input": "..."` | Complete tool input for this call |

**Sequence Contract**:

```text
ToolCallStart(tool_call_id=C, tool_call_name="calculator")
ToolCallDelta(tool_call_id=C, delta="{\"expression\":")
ToolCallDelta(tool_call_id=C, delta="\"5678 * 345\"}")
ToolCallEnd(tool_call_id=C, input="{\"expression\":\"5678 * 345\"}")
```

No ToolCallDelta for the same `tool_call_id` may appear after ToolCallEnd.

## Tool Result Events

### ToolResultEndEvent

```json
{
  "event_type": "tool_result_end",
  "reply_id": "reply-001",
  "tool_call_id": "call-001",
  "state": "success",
  "metadata": {},
  "output": "1958910"
}
```

**Field Semantics**:

| Field | Semantics |
|-------|-----------|
| `output` omitted | Complete output is unknown, unavailable, or not safe to claim as complete |
| `"output": ""` | Complete output is known and empty |
| `"output": "..."` | Complete observable output for this tool result |

**Sequence Contract**:

```text
ToolResultStart(tool_call_id=C, tool_call_name="calculator")
ToolResultTextDelta(tool_call_id=C, delta="195")
ToolResultTextDelta(tool_call_id=C, delta="8910")
ToolResultEnd(tool_call_id=C, state=success, output="1958910")
```

For interrupted or error states, output must not misrepresent partial content as a complete successful result.

## Non-Streaming Contract

Non-streaming producers that emit synthetic Start → Delta → End sequences must populate EndEvent fields from the same complete block content used to produce DeltaEvent.

Example:

```text
TextBlockStart(block_id=A)
TextBlockDelta(block_id=A, delta="full response")
TextBlockEnd(block_id=A, text="full response")
```

Tool calls in non-streaming model responses follow the same rule:

```text
ToolCallStart(tool_call_id=C)
ToolCallDelta(tool_call_id=C, delta="{...}")
ToolCallEnd(tool_call_id=C, input="{...}")
```

## Streaming Contract

Streaming producers must accumulate only deltas observed between the corresponding Start and End for that id.

For interleaved blocks:

```text
TextBlockStart(A)
TextBlockDelta(A, "a1")
ThinkingBlockStart(B)
ThinkingBlockDelta(B, "b1")
TextBlockDelta(A, "a2")
TextBlockEnd(A, text="a1a2")
ThinkingBlockEnd(B, thinking="b1")
```

Each EndEvent must contain only its own id's content.

## Error and Cancellation Contract

- Existing ReplyEnd and ToolResultState error/interrupted semantics remain authoritative.
- If a block/tool lifecycle did not complete normally, EndEvent content must be omitted unless the producer can prove the content is complete for that lifecycle.
- Producers must not emit content fields to hide or soften errors.

## Consumer Guidance

Consumers can choose one of two valid strategies:

1. **Snapshot strategy**: Read EndEvent complete-content fields when present; fall back to DeltaEvent accumulation when absent.
2. **Streaming strategy**: Ignore EndEvent complete-content fields and continue to process DeltaEvent streams exactly as before.

Both strategies must observe the same final block content for normal completed lifecycles.
