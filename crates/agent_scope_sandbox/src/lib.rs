//! AgentScope Sandbox — controlled execution sessions and workspace adapter.
//!
//! This crate provides a local reference sandbox backend with explicit capability
//! reporting. It prevents path traversal and symlink escape for filesystem
//! operations, records command execution history, enforces timeouts and output
//! limits, and reports unavailable hard isolation features instead of silently
//! pretending to support them. The local backend is useful for development and
//! lightweight controlled execution, but it is not a microVM or container
//! isolation boundary.
//!
//! Enable the `microsandbox` feature to use the feature-gated
//! `MicrosandboxSession` backend. That backend requires a separately installed
//! microsandbox runtime, keeps `NetworkPolicy::Disabled` as its default, and
//! returns capability / unsupported-policy errors instead of falling back to the
//! local-process backend.
//!
//! Always inspect `CapabilityReport` for the selected backend; the presence of a
//! backend does not mean every `SandboxPolicy` field can be enforced exactly.
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
//! let report = session.capability_report().await?;
//! assert_eq!(report.backend_name, "local-process");
//! session.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ```rust,no_run
//! # #[cfg(feature = "microsandbox")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use agent_scope_sandbox::{MicrosandboxConfig, MicrosandboxSession, SandboxSession};
//!
//! let mut session = MicrosandboxSession::new(MicrosandboxConfig::default())?;
//! session.initialize().await?;
//! session.close().await?;
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]

pub mod capability;
pub mod error;
pub mod execution;
pub mod local;
#[cfg(feature = "microsandbox")]
pub mod microsandbox;
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
#[cfg(feature = "microsandbox")]
pub use microsandbox::{MicrosandboxConfig, MicrosandboxSession};
pub use mount::{MountAccess, MountOwner, SandboxMount};
pub use policy::{CpuLimit, NetworkPolicy, SandboxPolicy};
pub use session::{LocalSandboxConfig, SandboxSession, SandboxState};
pub use workspace_backend::SandboxWorkspaceBackend;
