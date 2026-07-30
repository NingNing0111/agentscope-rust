//! AgentScope Foundation Layer — State management.

#![deny(unsafe_code)]

pub mod agent_state;
pub mod permission;
pub mod session;
pub mod session_store;
pub mod task;
pub mod trim;

// Re-exports
pub use agent_state::{
    AgentState, AppendContextError, ReadCacheEntry, ReplyContext, SummaryContent, ToolContext,
};
pub use permission::{PermissionContext, PermissionRule};
pub use session::{Session, SessionError, SessionImpl, SessionMeta, SessionStatus};
pub use session_store::{InMemorySessionStore, SessionStore};
pub use task::{Task, TaskContext, TaskError, TaskState};
pub use trim::{TokenCounter, TrimResult, TrimStrategy, trim_context};
