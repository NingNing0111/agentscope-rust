//! PermissionContext & PermissionRule — placeholder types.
//!
//! These will be replaced by the full permission module implementation
//! in a future feature.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Placeholder permission context — will be replaced.
pub type PermissionContext = HashMap<String, serde_json::Value>;

/// Placeholder permission rule — will be replaced.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionRule {
    #[serde(flatten)]
    pub extras: HashMap<String, serde_json::Value>,
}
