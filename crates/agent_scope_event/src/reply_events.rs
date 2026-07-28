//! Reply lifecycle events.

use serde::{Deserialize, Serialize};

use agent_scope_types::{ErrorInfo, ReplyFinishedReason};

use crate::base::EventBase;

/// A new reply has started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyStartEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub session_id: String,
    pub reply_id: String,
    pub name: String,
    pub role: String,
}

/// A reply has ended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyEndEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub session_id: String,
    pub reply_id: String,
    pub finished_reason: ReplyFinishedReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}
