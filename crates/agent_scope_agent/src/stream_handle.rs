//! StreamHandle — cancellation bridge between EventStream and the reactor.
//!
//! When the consumer drops [`EventStream`](super::react_agent::EventStream), the
//! oneshot sender is dropped, which closes the channel. The reactor checks
//! [`StreamHandle::is_cancelled()`] before each model call and tool execution.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tokio::sync::oneshot;

/// Handle passed to the reactor loop for cancellation detection.
pub(crate) struct StreamHandle {
    cancel_rx: Mutex<oneshot::Receiver<()>>,
    is_streaming: Arc<AtomicBool>,
}

impl StreamHandle {
    /// Create a new handle pair.
    ///
    /// Returns `(StreamHandle, oneshot::Sender<()>)`. The sender half is
    /// stored in `EventStream` — when `EventStream` is dropped, the sender
    /// is dropped, closing the channel.
    pub(crate) fn new(is_streaming: Arc<AtomicBool>) -> (Self, oneshot::Sender<()>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                cancel_rx: Mutex::new(rx),
                is_streaming,
            },
            tx,
        )
    }

    /// Create a dummy StreamHandle that is never cancelled.
    /// Used in the Complete (non-streaming) response path where no
    /// EventStream exists to provide a real cancellation signal.
    pub(crate) fn new_dummy() -> Self {
        let (_, rx) = oneshot::channel();
        Self {
            cancel_rx: Mutex::new(rx),
            is_streaming: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if the stream has been dropped (cancellation requested).
    ///
    /// Returns `true` only when the oneshot sender has been dropped
    /// (i.e., `EventStream` was dropped, closing the cancel channel).
    pub(crate) fn is_cancelled(&self) -> bool {
        matches!(
            self.cancel_rx.lock().unwrap().try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed)
        )
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        self.is_streaming.store(false, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_handle_not_cancelled_when_sender_alive() {
        let is_streaming = Arc::new(AtomicBool::new(false));
        let (handle, _tx) = StreamHandle::new(Arc::clone(&is_streaming));
        assert!(!handle.is_cancelled());
    }

    #[test]
    fn test_stream_handle_is_cancelled_when_sender_dropped() {
        let is_streaming = Arc::new(AtomicBool::new(false));
        let (handle, tx) = StreamHandle::new(Arc::clone(&is_streaming));
        drop(tx);
        assert!(handle.is_cancelled());
    }

    #[test]
    fn test_is_streaming_cleared_on_drop() {
        let is_streaming = Arc::new(AtomicBool::new(true));
        let (handle, _tx) = StreamHandle::new(Arc::clone(&is_streaming));
        drop(handle);
        assert!(!is_streaming.load(Ordering::SeqCst));
    }
}
