//! EventBase — shared base fields for all events.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_id() -> String {
    uuid::Uuid::new_v4().as_simple().to_string()
}

fn default_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Base fields shared by all event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBase {
    #[serde(default = "default_id")]
    pub id: String,
    #[serde(default = "default_timestamp")]
    pub created_at: String,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl EventBase {
    /// Create a new EventBase with auto-generated id and timestamp.
    pub fn new() -> Self {
        Self {
            id: default_id(),
            created_at: default_timestamp(),
            metadata: HashMap::new(),
        }
    }
}

impl Default for EventBase {
    fn default() -> Self {
        Self::new()
    }
}
