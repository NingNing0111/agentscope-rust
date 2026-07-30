# Contract: StreamHandle — Stream Lifetime Cancellation

**Feature**: 008-streaming-infrastructure
**Contract Type**: Internal Module API (pub(crate))
**Stability**: Internal

## Purpose

将 `EventStream` 的生命周期绑定到后台处理任务的取消。当消费者丢弃 stream 时，`StreamHandle` 接收取消信号并传播到 reactor 循环。

## API

```rust
pub(crate) struct StreamHandle {
    cancel_rx: oneshot::Receiver<()>,
    is_streaming: Arc<AtomicBool>,
}

impl StreamHandle {
    /// Create a new handle. Returns (StreamHandle, oneshot::Sender).
    /// The sender is stored in EventStream — when EventStream is dropped,
    /// the sender is dropped, which closes the oneshot channel.
    pub(crate) fn new(is_streaming: Arc<AtomicBool>) -> (Self, oneshot::Sender<()>);

    /// Check if cancellation has been requested.
    /// Returns true if the stream has been dropped.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel_rx.try_recv().is_err()
    }

    /// Await cancellation (for use in select! loops).
    pub(crate) async fn cancelled(&mut self) {
        let _ = &mut self.cancel_rx;
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        self.is_streaming.store(false, Ordering::SeqCst);
    }
}
```

## EventStream Drop Contract

```rust
impl Drop for EventStream {
    fn drop(&mut self) {
        // 1. Drop the oneshot sender → closes channel → StreamHandle receives cancel
        drop(self.cancel_tx.take());
        // 2. Drop the mpsc receiver → sender in reactor will fail on next emit
        //    (reactor should already be stopping due to cancel signal)
    }
}
```

## Invariants

1. `StreamHandle::is_cancelled()` 在 `EventStream::Drop` 执行后的第一次检查必须返回 `true`
2. `is_streaming` flag 在 `StreamHandle::Drop` 时被清除（允许新 reply 调用）
3. reactor 任务在检测到取消后 MUST 停止发送事件并尽快终止
4. 如果 reactor 在 stream 被 drop 前正常完成，它先 drop mpsc sender（导致 stream 的 `poll_next` 返回 `None`），然后 drop StreamHandle
