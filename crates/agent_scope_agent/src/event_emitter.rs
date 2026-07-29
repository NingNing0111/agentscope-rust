//! AgentEvent emission helper.
//!
//! Internal (`pub(crate)`) wrapper around `tokio::sync::broadcast` for
//! publishing [`AgentEvent`] items during the agent lifecycle.

use agent_scope_event::AgentEvent;
use tokio::sync::broadcast;

/// Wraps a bounded broadcast channel for agent event publishing.
pub(crate) struct EventEmitter {
    tx: broadcast::Sender<AgentEvent>,
}

impl EventEmitter {
    /// Create a new emitter with the given buffer capacity.
    pub(crate) fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event to all subscribers.
    ///
    /// Non-blocking: if the channel is full, the oldest event is dropped
    /// (broadcast behavior). A `tracing::warn` is emitted when send lags.
    pub(crate) fn emit(&self, event: impl Into<AgentEvent>) {
        let event = event.into();
        if self.tx.receiver_count() > 0
            && let Err(e) = self.tx.send(event)
        {
            tracing::warn!(
                dropped_event = ?e,
                "EventEmitter: failed to send event (no active receivers?)"
            );
        }
        // If no receivers, silently drop — events only consumed by reply_stream()
    }

    /// Create a new subscriber to the event stream.
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.tx.subscribe()
    }
}

impl Clone for EventEmitter {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}
