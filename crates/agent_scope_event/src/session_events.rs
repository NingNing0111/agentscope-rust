//! Session lifecycle events.
//!
//! Five event types covering the complete session lifecycle:
//! created → (saved/loaded/trimmed)* → closed.

use serde::{Deserialize, Serialize};

use crate::base::EventBase;

/// Emitted when a new session is created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreatedEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub session_id: String,
}

/// Emitted when a session is explicitly closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionClosedEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub session_id: String,
    /// Reason: "explicit_close" | "drop" | "error"
    pub reason: String,
}

/// Emitted when a session is persisted to a store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSavedEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub session_id: String,
    pub message_count: usize,
}

/// Emitted when a session is loaded from a store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLoadedEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub session_id: String,
    pub message_count: usize,
}

/// Emitted after context trimming is performed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTrimmedEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub session_id: String,
    pub messages_before: usize,
    pub messages_after: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_before: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_after: Option<usize>,
}
