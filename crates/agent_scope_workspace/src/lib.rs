//! AgentScope Workspace — isolated working environment for Agents.
//!
//! This crate provides a workspace abstraction that gives each agent its own
//! filesystem sandbox with built-in tools (Bash, Edit, Glob, Grep, Read, Write),
//! MCP client configuration management, skill management, and context offloading.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = LocalWorkspaceConfig {
//!     workdir: "/tmp/my-workspace".into(),
//!     workspace_id: None,
//!     default_mcps: vec![],
//!     skill_paths: vec![],
//!     instructions: None,
//! };
//! let mut ws = LocalWorkspace::new(config);
//! ws.initialize().await?;
//! assert!(ws.is_alive());
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]
#![deny(clippy::unwrap_used)]

pub mod backend;
pub mod base;
pub mod error;
pub mod instructions;
pub mod local_workspace;
pub mod manager;
pub mod mcp;
pub mod offload;
pub mod skill;

pub use backend::{ContainedBackend, ExecOutput, LocalBackend, WorkspaceBackend};
pub use base::{McpConnectionHandle, McpConnectionsHost, ToolInfo, WorkspaceBase};
pub use error::WorkspaceError;
pub use local_workspace::{LocalWorkspace, LocalWorkspaceConfig};
pub use manager::WorkspaceManager;
pub use mcp::{McpClientConfig, McpRegistry, McpTransportConfig};
pub use skill::{Skill, SkillEntry, SkillManager, SkillsIndex};
