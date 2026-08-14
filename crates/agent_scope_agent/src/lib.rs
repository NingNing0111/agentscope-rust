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
pub mod context_policy;
pub mod delegation;
pub mod delegation_trace;
pub mod event_emitter;
pub mod event_input;
pub mod hitl_resume;
pub mod memory_middleware;
pub mod middleware;
pub mod permission;
pub mod react_agent;
pub(crate) mod react_loop;
pub mod runtime_injection;
pub(crate) mod stream_handle;
pub(crate) mod streaming_reactor;
pub mod subagent;
pub mod subagent_error;
pub mod task_reminder;
pub mod task_tools;
pub(crate) mod token_counter;
pub(crate) mod tool_feedback;

// Re-exports
pub use agent_error::AgentError;
pub use agent_trait::Agent;
pub use config::{AgentConfig, AgentConfigBuilder, ContextConfig, InjectionConfig, ReActConfig};
pub use context_policy::{
    CapabilityScope, ContextSharingPolicy, MessageContextPolicy, ModelAccessPolicy,
    ResourceSharingPolicy, SharedContext, SideEffectPolicy,
};
pub use delegation::{
    CollaborationResult, CollaborationStatus, DelegationBudget, DelegationReplyMode,
    DelegationRequest, MultiAgentConversation, Participant, SideEffectRecord, SideEffectType,
    delegate_many, delegate_once, delegate_once_with_cancel, delegate_stream,
    observe_result_by_parent, unsupported_app_service_dispatch, unsupported_cross_host_migration,
    unsupported_durable_queue, unsupported_remote_worker,
};
pub use delegation_trace::{DelegationEvent, DelegationEventType, DelegationTrace, safe_summary};
pub use memory_middleware::MemoryMiddleware;
pub use middleware::Middleware;
pub use permission::{
    PermissionBehavior, PermissionContext, PermissionDecision, PermissionEngine, PermissionMode,
    PermissionResult, PermissionRule,
};
pub use react_agent::ReActAgent;
pub use subagent::{
    SelectionPolicy, SubAgent, SubAgentRegistry, SubAgentState, SubAgentTemplate, TemplateStatus,
};
pub use subagent_error::{SubAgentError, SubAgentErrorCategory, SubAgentErrorInfo};
pub use task_tools::{TASK_TOOL_NAMES, TaskCreateTool, TaskGetTool, TaskListTool, TaskUpdateTool};
