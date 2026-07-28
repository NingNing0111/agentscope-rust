//! Model call lifecycle events.

use serde::{Deserialize, Serialize};

use agent_scope_types::ReplyFinishedReason;

use crate::base::EventBase;

/// A model call has started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCallStartEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub model_name: String,
}

/// A model call has ended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCallEndEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub finished_reason: ReplyFinishedReason,
}
