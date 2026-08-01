//! AgentScope Sandbox — controlled execution sessions and workspace adapter.
//!
//! This crate provides a local reference sandbox backend with explicit capability
//! reporting. It prevents path traversal and symlink escape for filesystem
//! operations, records command execution history, enforces timeouts and output
//! limits, and reports unavailable hard isolation features instead of silently
//! pretending to support them.
//!
//! ```rust,no_run
//! use agent_scope_sandbox::{LocalSandboxConfig, LocalSandboxSession, SandboxSession};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut session = LocalSandboxSession::new(LocalSandboxConfig::default())?;
//! session.initialize().await?;
//! session.write_file("notes/result.txt", b"hello").await?;
//! let data = session.read_file("notes/result.txt").await?;
//! assert_eq!(data, b"hello");
//! session.close().await?;
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]

pub mod capability;
pub mod error;
pub mod execution;
pub mod local;
pub mod mount;
pub mod path;
pub mod policy;
pub mod session;
pub mod workspace_backend;

pub use capability::{
    CapabilityReport, CompatibilityLevel, SandboxCapability, UnsupportedCapability,
};
pub use error::{SandboxError, SandboxResult};
pub use execution::{
    ExecutionRecord, ExecutionRequest, ExecutionResult, ExecutionStatus, OutputRef, OutputSummary,
    ResourceLimitHit, redacted_command_summary,
};
pub use local::LocalSandboxSession;
pub use mount::{MountAccess, MountOwner, SandboxMount};
pub use policy::{CpuLimit, NetworkPolicy, SandboxPolicy};
pub use session::{LocalSandboxConfig, SandboxSession, SandboxState};
pub use workspace_backend::SandboxWorkspaceBackend;
