//! AgentScope Foundation Layer — core type definitions.
//!
//! No internal dependency on other `agent_scope_*` crates.

#![deny(unsafe_code)]

pub mod error;
pub mod hook;
pub mod json;
pub mod reply;

// Re-exports
pub use error::{ErrorInfo, ErrorType};
pub use json::JsonValue;
pub use reply::ReplyFinishedReason;

/// Embedding vector type.
pub type Embedding = Vec<f64>;
