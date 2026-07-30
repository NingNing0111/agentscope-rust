# Contract: EventEmitter — Internal Event Channel

**Feature**: 008-streaming-infrastructure
**Contract Type**: Internal Module API (pub(crate))
**Stability**: Internal — may change without semver

## Current API (broadcast-based)

```rust
pub(crate) struct EventEmitter {
    tx: broadcast::Sender<AgentEvent>,
}

impl EventEmitter {
    pub(crate) fn new(capacity: usize) -> Self;
    pub(crate) fn emit(&self, event: impl Into<AgentEvent>);
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<AgentEvent>;
}
```

## New API (mpsc-based)

```rust
pub(crate) struct EventEmitter {
    tx: mpsc::Sender<AgentEvent>,
}

impl EventEmitter {
    /// Create a new emitter.
    /// `capacity`: None = unbounded channel, Some(N) = bounded with N slots.
    pub(crate) fn new(capacity: Option<usize>) -> Self;

    /// Publish an event.
    /// Awaits channel capacity when bounded and full (backpressure).
    /// Panics if the corresponding receiver has been dropped
    ///   (this is a bug — the reactor should stop before the channel closes).
    pub(crate) async fn emit(&self, event: impl Into<AgentEvent>);

    /// Clone the sender for use in spawned tasks.
    /// Typically called once per `reply_stream()` to create the tx/receiver pair.
    pub(crate) fn clone_sender(&self) -> mpsc::Sender<AgentEvent>;
}
```

## Key Behavioral Differences

| Aspect | broadcast | mpsc |
|--------|-----------|------|
| Multi-consumer | Yes (each subscriber gets all events) | No (single consumer) |
| On channel full | Oldest event dropped | Sender awaits (backpressure) |
| Clone | Sender cloned for broadcasting | Sender cloned for multi-producer |
| emit() | sync, non-blocking | async, may block |
| subscribe() | Returns Receiver | REMOVED — channels created per-stream |

## Usage Pattern

```rust
// In reply_stream():
let (tx, rx) = mpsc::channel::<AgentEvent>(capacity);
let emitter = EventEmitter { tx: tx.clone() };

// Spawn reactor task with tx:
tokio::spawn(async move {
    run_streaming_loop(ctx, stream_handle, tx).await;
});

// Return stream to caller:
EventStream { rx, cancel_tx: Some(cancel_tx) }
```
