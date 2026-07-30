# Data Model: Streaming Infrastructure

**Feature**: 008-streaming-infrastructure
**Date**: 2026-07-29

## Entities

### EventStream (NEW)

| Field | Type | Description |
|-------|------|-------------|
| `rx` | `tokio::sync::mpsc::Receiver<AgentEvent>` | Event receiver end of the channel |
| `cancel_tx` | `Option<tokio::sync::oneshot::Sender<()>>` | Cancel signal sender, fires on Drop |

**Lifecycle**:
1. Created by `ReActAgent::reply_stream()`
2. Yielded to caller as `Pin<Box<dyn Stream<Item = AgentEvent> + Send>>`
3. Each `poll_next()` dequeues from `rx`
4. On Drop: sends oneshot cancel signal, clears `is_streaming` flag on agent
5. After `ReplyEnd` event: fused, all subsequent polls return `None`

**Invariants**:
- `rx` is the exclusive consumer of the mpsc channel for this stream
- Drop MUST fire cancel signal before dropping `rx`
- After Drop, `is_streaming` flag on `AgentInner` MUST be cleared

---

### EventEmitter (MODIFIED)

**Current**: Uses `tokio::sync::broadcast::Sender<AgentEvent>`
**New**: Uses `tokio::sync::mpsc::Sender<AgentEvent>` (multi-producer, single-consumer per stream)

| Field | Type | Description |
|-------|------|-------------|
| `tx` | `mpsc::Sender<AgentEvent>` | Cloned for internal use; each `reply_stream()` creates its own channel |

**Key Changes**:
- `new(capacity: Option<usize>)`: `None` → unbounded channel, `Some(N)` → bounded channel with capacity N
- `emit(&self, event)`: `async fn` — awaits channel space when bounded and full (backpressure)
- `subscribe()` → REMOVED: each `reply_stream()` creates a new mpsc channel
- Clone remains — allows passing tx to spawned tasks

**Transition Insight**: `emit()` becomes async to support backpressure (`.send().await`). The caller (reactor loop) is already async.

---

### StreamHandle (NEW)

| Field | Type | Description |
|-------|------|-------------|
| `cancel_rx` | `tokio::sync::oneshot::Receiver<()>` | Receives cancel signal when stream is dropped |
| `is_streaming` | `Arc<AtomicBool>` | Reference to agent's streaming guard flag |

**Lifecycle**:
1. Created alongside `EventStream`
2. Passed to `run_streaming_loop()` — checked before each model call and tool execution
3. `cancel_rx` becoming ready (or closed) = stream was dropped → cancel processing
4. Dropped when streaming loop completes normally

---

### StreamingReactor (NEW — internal logic)

Pure function with no fields. Entry point:

```rust
pub(crate) async fn run_streaming_loop(
    ctx: ReactLoopContext<'_>,
    stream_handle: StreamHandle,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<(), AgentError>
```

Returns `Ok(())` on normal completion (events already sent). Returns `Err(...)` on failure after emitting error event.

---

### AgentInner (MODIFIED)

| Field | Type | Change |
|-------|------|--------|
| `event_emitter` | `EventEmitter` | **Type changed**: broadcast → mpsc |
| `interrupted` | `AtomicBool` | Unchanged |
| `is_streaming` | `AtomicBool` | **NEW**: guards against concurrent reply calls |
| `stream_channel_capacity` | `Option<usize>` | **NEW**: from AgentConfig, passed to EventEmitter::new() |

---

### AgentConfig (MODIFIED)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `stream_channel_capacity` | `Option<usize>` | `None` | Channel type: `None` = unbounded, `Some(N)` = bounded with N slots |

---

### AgentError (MODIFIED)

New variant:

```rust
/// A streaming reply is already in progress.
AlreadyStreaming,
```

---

### ToolExecOutput (UNCHANGED)

Already has `Stream(Pin<Box<dyn Stream<Item = Result<ToolResultBlock, ToolError>> + Send>>)` variant. No changes needed — the existing `Stream` variant is consumed progressively by `StreamingReactor`.

### ToolCallDeltaEvent (EXISTING — now used in streaming)

Already exists in `agent_scope_event::tool_events`. Previously unused in ReAct loop (only TextBlockDelta was emitted). Now emitted for each model stream chunk containing partial tool call arguments.

### ToolResultTextDeltaEvent (EXISTING — now used for streaming tool output)

Already exists in `agent_scope_event::tool_events`. Previously a single delta was emitted with the full tool result. Now emitted progressively for streaming tool output.

## Relationships

```text
ReActAgent
├── AgentInner
│   ├── EventEmitter (mpsc::Sender<AgentEvent>)
│   │   └── cloned for each reply_stream() call → paired with mpsc::Receiver
│   ├── AtomicBool (is_streaming) ← checked by reply()
│   └── AtomicBool (interrupted) ← set by interrupt()
│
└── reply_stream() creates:
    ├── EventStream (mpsc::Receiver + oneshot::Sender)
    └── StreamHandle (oneshot::Receiver + Arc<AtomicBool>)
        └── passed to run_streaming_loop()

run_streaming_loop()
├── receives: ReactLoopContext + StreamHandle + mpsc::Sender
├── on each model chunk:
│   └── emit events through mpsc::Sender
│   └── check StreamHandle for cancellation
├── on tool call completion:
│   └── execute tool (consuming ToolExecOutput::Stream if present)
│   └── emit ToolResultTextDelta events through mpsc::Sender
└── on completion:
    └── emit ReplyEnd through mpsc::Sender
    └── mpsc::Sender dropped → stream ends

EventStream::Drop
├── fires oneshot::Sender → StreamHandle receives cancel
└── clears is_streaming AtomicBool
```

## State Transitions

### Stream Lifecycle

```text
[Created] → [Streaming] → [ReplyEnd Sent] → [Fused]
                │
                └── [Dropped] → [Cancelled]
```

1. **Created**: `reply_stream()` called, mpsc channel created, task spawned
2. **Streaming**: Events flowing through channel
3. **ReplyEnd Sent**: Final event emitted, stream fused (all subsequent polls → `None`)
4. **Dropped**: Consumer drops stream → cancel signal sent → processing task terminates
5. **Cancelled**: Internal state after drop; agent ready for new reply

### is_streaming Guard

```text
[false] → reply() called → compare_exchange(false, true) → [true]
[true] → reply() called → compare_exchange fails → AlreadyStreaming error
[true] → stream ends/dropped → store(false) → [false]
```
