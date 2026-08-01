# Event & Streaming

> One-liner: AgentScope's unified event protocol — 33 `AgentEvent` variants covering reply lifecycle, model calls, content-block streaming, tool execution, and session management; the single entry point for observing agent behavior (traces) and building streaming UIs.

## 1. Overview

This module covers the `agent_scope_event` crate, which sits at the foundation layer (depending only on `agent_scope_types` and `agent_scope_message`) and defines every event type emitted while an agent runs. Streaming semantics (emission order, cancellation, backpressure) are implemented by the Agent/model layers; this module is their shared vocabulary.

**When to use**: consuming agent event streams to render a terminal/UI, tracing a full reply via events, incrementally building a `Msg` from an event stream with `AppendEvent`, publishing custom service-layer events.

**Prerequisites**: [Message & Basic Types](./message-types.md) (the `Msg`/`ContentBlock`/`ToolResultState` payloads inside events).

## 2. Core Concepts & Main Public Types

### 2.1 The Event Bus: `AgentEvent` & `EventType`

`AgentEvent` is an enum discriminated by a `type` tag (serde tagged union) with **33 variants**; `EventType` is the corresponding plain discriminator enum. Serialization tags are SCREAMING_SNAKE_CASE (e.g., `"REPLY_START"`, `"TEXT_BLOCK_DELTA"`).

All events share the `EventBase { id, created_at, metadata }` base: `id` is an auto-generated UUID, `created_at` an RFC 3339 timestamp, and `metadata` an arbitrary key-value dictionary. Except for Session events, each event inlines the base fields via `#[serde(flatten)]`.

### 2.2 Event Groups

| Group | Events | Key payloads |
|-------|--------|--------------|
| Reply lifecycle | `ReplyStart` / `ReplyEnd` | `session_id`, `reply_id`, `name`, `role`; End carries `finished_reason` and optional `error: ErrorInfo` |
| Model calls | `ModelCallStart` / `ModelCallEnd` | `model_name`; End carries `input_tokens`/`output_tokens`/`finished_reason` |
| Text/thinking/data block streaming | `TextBlockStart/Delta/End`, `ThinkingBlockStart/Delta/End`, `DataBlockStart/Delta/End` | `reply_id` + `block_id`; Deltas carry incremental `delta`/`data`; Data blocks also carry `media_type` |
| Hint block (one-shot) | `HintBlock` | `hint: HintContent`, optional `source`; non-streaming, no Start/Delta/End sequence |
| Tool call streaming | `ToolCallStart/Delta/End` | `tool_call_id`, `tool_call_name`; Deltas accumulate the input JSON |
| Tool result streaming | `ToolResultStart`, `ToolResultTextDelta`, `ToolResultDataDelta`, `ToolResultEnd` | `tool_call_id`; End carries `state: ToolResultState` and `metadata` |
| Control & interaction | `ExceedMaxIters`, `RequireUserConfirm`, `UserConfirmResult`, `UserInterrupt`, `RequireExternalExecution`, `ExternalExecutionResult` | User confirmation (`ConfirmResult { confirmed, tool_call, rules }`), external execution (`Vec<ToolResultBlock>`), interruption |
| Session lifecycle | `SessionCreated/Closed/Saved/Loaded/Trimmed` | `session_id`; Closed carries `reason` (`explicit_close`/`drop`/`error`); Trimmed carries message counts before/after and optional token counts |
| Custom | `Custom` | `name` + `value: HashMap<String, Value>` for arbitrary service-layer notifications |

### 2.3 End Events Carry Full Accumulated Content (Feature 014 Semantics)

A streaming block's End event does more than mark lifecycle completion — it carries the **full accumulated content** from Start to End:

| Event | Full-content field | Semantics |
|-------|--------------------|-----------|
| `TextBlockEnd` | `text: Option<String>` | Complete text from all Deltas of this block |
| `ThinkingBlockEnd` | `thinking: Option<String>` | Complete thinking content of this block |
| `ToolCallEnd` | `input: Option<String>` | Complete input JSON accumulated from all `ToolCallDelta` events |
| `ToolResultEnd` | `output: Option<String>` | Complete observable output accumulated from all `ToolResultTextDelta` events |

Uniform convention: `Some("")` means **known empty**; `None` means **unknown/unavailable**. For `ToolResultEnd` in `error`/`interrupted` states, `output` must be `None` (unless the output is known complete) — consumers must not treat `None` as an empty string.

### 2.4 Event Emission Order (Trace Semantics)

The typical event sequence of one reply (Constitution Article VII: traces are core acceptance artifacts):

```text
ReplyStart
└─ ModelCallStart
   ├─ ThinkingBlockStart → ThinkingBlockDelta* → ThinkingBlockEnd   (optional)
   ├─ TextBlockStart → TextBlockDelta* → TextBlockEnd               (optional)
   └─ ToolCallStart → ToolCallDelta* → ToolCallEnd                  (optional, may repeat)
      └─ ToolResultStart → ToolResultTextDelta*/ToolResultDataDelta* → ToolResultEnd
└─ ModelCallEnd
(Model-call section repeats across reasoning-acting iterations)
ReplyEnd (finished_reason: completed / interrupted / exceed_max_iters / error)
```

`reply_id` spans all events of one reply; `block_id`/`tool_call_id` span the lifecycle of a single block — these two IDs are the anchors for correlating an event stream into a structured message.

### 2.5 Cancellation Behavior

Cancellation works through two cooperating paths (Feature 008 semantics):

1. **`CancellationToken`** (tokio-util): the Agent layer checks the cancellation signal at each iteration and stops further model calls and tool execution once cancelled;
2. **`UserInterrupt` event**: emitted when cancellation occurs, followed by `ReplyEnd` with `finished_reason: interrupted` — this is how consumers distinguish a normal completion from an interruption.

### 2.6 `AppendEvent`: Event Stream → Msg

The `AppendEvent` trait incrementally applies an event stream onto a `Msg`, building the complete message step by step (Deltas append text, Ends finalize blocks, `ToolResultEnd` writes state). Failures return `AppendEventError`:

| Error | Trigger condition |
|-------|-------------------|
| `ReplyIdMismatch` | Event `reply_id` does not match the target message `id` |
| `BlockNotFound` | A Delta/End event references a `block_id` with no corresponding Start |
| `UnknownEventType` | Unrecognized event type |

## 3. Quick Example

The terminal chat example tracks per-block state with a `BlockTracker` to summarize accumulated content at End events:

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

See `examples/chat.rs` for the full event-rendering loop (`render_event()`, starting at L86) — it covers every `AgentEvent` variant and is the authoritative reference for "how to consume an event stream".

## 4. Usage Patterns

### 4.1 Consuming the Event Stream (Match by Group)

Events are matched as Rust enum variants; organize match arms by group (see `render_event` in `examples/chat.rs`): print boundary markers for Reply/Model lifecycle, emit increments immediately for Deltas, and print accumulated summaries while resetting block state at Ends.

### 4.2 Using End-Event Full Content for Summaries

When an End event arrives, read its full-content field directly — no need to concatenate Deltas yourself (Feature 014):

```rust
AgentEvent::TextBlockEnd(e) => {
    if let Some(full_text) = &e.text {
        // full_text is the complete Start→End text of this block
    }
}
AgentEvent::ToolResultEnd(e) => {
    // output is None in error/interrupted states — never treat it as empty
    match (&e.state, &e.output) { /* ... */ }
}
```

### 4.3 Incremental Msg Building (AppendEvent)

Hold an assistant `Msg` and call `append_event(&event)` for each incoming event; when the reply ends you hold the complete structured message. Validate event ownership by `reply_id` to avoid cross-talk (`ReplyIdMismatch`).

### 4.4 Publishing Custom Events

Service layers (e.g., progress notifications) can use the `Custom` event without polluting the protocol event space:

```rust
let event = CustomEvent {
    base: EventBase::new(),
    name: "ingest-progress".into(),
    value: HashMap::from([("done".into(), serde_json::json!(3))]),
};
```

## 5. Errors & Unsupported Capabilities

| Error type | Trigger condition |
|------------|-------------------|
| `AppendEventError::ReplyIdMismatch` | An event is applied to a mismatched `Msg` |
| `AppendEventError::BlockNotFound` | A Delta/End arrives without a corresponding Start (block not found) |
| `AppendEventError::UnknownEventType` | Unrecognized event type |

**Unsupported capabilities**: none. This module is a pure event protocol with no `UnsupportedFeature` paths.

**FAQ**:

- *Why are End-event content fields `Option`?*: `Some("")` and `None` differ in meaning — the former is known empty, the latter unknown/unavailable (e.g., an interrupted tool result).
- *Received a Delta without a Start?*: the protocol guarantees Start→Delta*→End order; a missing Start means the stream was truncated or `reply_id` cross-talk occurred, and `AppendEvent` reports `BlockNotFound`.

## 6. Compatibility

- **Compatibility level**: **L1** (field-by-field event structure and serialization protocol compatibility, 5 entries); **L2** (behaviorally equivalent emission order/streaming semantics, 29 entries)
- **Authoritative source**: `specs/001-compatibility-baseline/capability-matrix.json`
- **Known deviations**:
  - The matrix `status` field is currently `NOT_ANALYZED` for all entries (not backfilled after Features 001-017). Levels on this page are cross-verified against matrix `target_level` (event category: L1×5/L2×29) + `specs/008-streaming-infrastructure`, `specs/014-end-event-content` + actual code state.
  - End events carrying full accumulated content (`text`/`thinking`/`input`/`output` fields) is a Rust-side enhancement from Feature 014; Python End events only mark lifecycle completion.
  - `EventType` actually has 33 variants (the crate doc comment saying "27" is outdated; the enum is authoritative).
  - Session events serialize `base` as a nested field (not flattened), unlike all other events which inline the base.
- **Unsupported capabilities**: none.

## 7. See Also

- [Message & Basic Types](./message-types.md) — event payloads and the target structure of `AppendEvent`
- [Agent System](./agent.md) — the producer of event streams and the cancellation mechanism
- [Model Abstraction](./model.md) — streaming-chunk-to-event conversion (StreamAccumulator)
- [Session Management](./session.md) — the emitter of session lifecycle events
