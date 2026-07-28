//! Tool call and tool result state enums — placeholder.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ToolCallState {
    #[default]
    Pending,
    Asking,
    Allowed,
    Submitted,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ToolResultState {
    #[default]
    Running,
    Success,
    Error,
    Interrupted,
    Denied,
}
