//! AgentScope Agent System — orchestration layer for AI agents.
//!
//! This crate defines the [`Agent`] trait (common interface for all agent types),
//! the [`ReActAgent`] (reasoning→acting loop with tool execution),
//! a [`Middleware`] trait (8 hook points for extension),
//! context compression, permission checking, and interruption handling.
//!
//! # Quick start
//!
//! ```rust,ignore
//! // use agent_scope_agent::{Agent, AgentConfig, ReActConfig, ReActAgent, ContextConfig};
//! use agent_scope_message::factory::user_msg;
//! use std::sync::Arc;
//!
//! // Create an agent with your model
//! let config = AgentConfig::builder()
//!     .name("assistant")
//!     .model(my_model)
//!     .build()
//!     .unwrap();
//!
//! let agent = ReActAgent::new(
//!     config,
//!     ReActConfig::default(),
//!     ContextConfig::default(),
//!     vec![],
//! ).unwrap();
//!
//! // Send a message
//! // let reply = agent.reply(Some(vec![user_msg("user", "Hello!").unwrap()])).await.unwrap();
//! ```

#![deny(unsafe_code)]

pub mod agent_error;
pub mod agent_trait;
pub mod config;
pub mod context_compression;
pub mod event_emitter;
pub mod memory_middleware;
pub mod middleware;
pub mod permission;
pub mod react_agent;
pub(crate) mod react_loop;
pub(crate) mod stream_handle;
pub(crate) mod streaming_reactor;
pub(crate) mod token_counter;

// Re-exports
pub use agent_error::AgentError;
pub use agent_trait::Agent;
pub use config::{AgentConfig, AgentConfigBuilder, ContextConfig, ReActConfig};
pub use memory_middleware::MemoryMiddleware;
pub use middleware::Middleware;
pub use permission::{PermissionEngine, PermissionResult, PermissionRule};
pub use react_agent::ReActAgent;
