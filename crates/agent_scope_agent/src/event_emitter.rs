//! AgentEvent emission helper.
//!
//! Internal (`pub(crate)`) factory for creating per-reply mpsc channels.
//! Each `reply()` / `reply_stream()` call creates a fresh channel pair via
//! `EventEmitter::create_channel()`.
//!
//! The sender half is passed to the reactor loop; the receiver half is either
//! wrapped in [`EventStream`](super::react_agent::EventStream) (streaming) or
//! consumed by a background drainer (batch).

use agent_scope_event::AgentEvent;
use tokio::sync::mpsc;

/// Factory for per-reply event channels.
///
/// Stores the capacity preference; actual channels are created on demand.
#[derive(Clone)]
pub(crate) struct EventEmitter {
    /// Channel capacity: `None` = effectively unbounded, `Some(n)` = bounded.
    /// Validated at construction: `Some(0)` panics.
    capacity: Option<usize>,
}

impl EventEmitter {
    /// Create a new factory.
    ///
    /// - `None` → effectively unbounded (large internal capacity)
    /// - `Some(cap)` → bounded channel with `cap` slots (must be > 0)
    ///
    /// **Panics**: if `Some(0)` is passed (P1-8 fix).
    pub(crate) fn new(capacity: Option<usize>) -> Self {
        if let Some(0) = capacity {
            panic!("EventEmitter: capacity `Some(0)` is invalid; use `None` for unbounded");
        }
        Self { capacity }
    }

    /// Create a new `(sender, receiver)` pair.
    pub(crate) fn create_channel(&self) -> (mpsc::Sender<AgentEvent>, mpsc::Receiver<AgentEvent>) {
        match self.capacity {
            Some(cap) => {
                debug_assert!(cap > 0);
                mpsc::channel::<AgentEvent>(cap)
            }
            None => {
                // Large capacity for effectively unbounded behavior.
                // The concurrent drainer in do_reply prevents deadlock.
                mpsc::channel::<AgentEvent>(262_144)
            }
        }
    }
}
