//! AgentScope Foundation Layer — State management.

#![deny(unsafe_code)]

pub mod agent_state;
pub mod permission;
pub mod task;

// Re-exports
pub use agent_state::{
    AgentState, AppendContextError, ReadCacheEntry, ReplyContext, SummaryContent, ToolContext,
};
pub use permission::{PermissionContext, PermissionRule};
pub use task::{Task, TaskContext, TaskError, TaskState};
