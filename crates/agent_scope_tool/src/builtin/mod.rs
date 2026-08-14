//! Built-in workspace tools — executable `Tool` implementations bound to a
//! `WorkspaceBackend`, mirroring the Python `agentscope/tool/_builtin/`
//! reference implementations (upstream commit `9d1026fa`).
//!
//! Tools: `Bash`, `Read`, `Edit`, `Write`, `Grep`, `Glob`, `PowerShell`,
//! `ResetTools`, `Skill`.
//!
//! All tools share a [`BuiltInToolContext`] carrying the workspace backend
//! handle and the per-session [`WorkspaceToolSession`] (read-state + active
//! groups). The dependency direction stays `agent_scope_tool` →
//! `agent_scope_workspace` (Constitution Art.11).

mod bash;
mod edit;
mod glob;
mod grep;
mod powershell;
mod read;
mod reset_tools;
mod session;
mod skill;
mod write;

use std::path::Path;
use std::sync::{Arc, RwLock};

use agent_scope_workspace::backend::WorkspaceBackend;

pub use session::WorkspaceToolSession;

pub use bash::BashTool;
pub use edit::EditTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use powershell::PowerShellTool;
pub use read::ReadTool;
pub use reset_tools::ResetToolsTool;
pub use skill::SkillTool;
pub use write::WriteTool;

/// Typed error category for rejected or failed tool invocations (FR-024).
///
/// Maps onto the Constitution Art.13 error model — see
/// `specs/029-agent-workspace-tools/data-model.md` for the full mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolErrorCategory {
    /// Invalid or missing parameters (ValidationError).
    ValidationFailure,
    /// Workspace boundary or authorization failure (PermissionDenied).
    PermissionDenied,
    /// Platform-dependent tool unavailable, e.g. `PowerShell` on non-Windows
    /// (UnsupportedFeature).
    UnsupportedCapability,
    /// Command exceeded its configured timeout (TimeoutError).
    Timeout,
    /// Command or filesystem operation failed after validation (ToolError).
    ExecutionFailure,
    /// Unexpected framework error (InternalError).
    InternalFailure,
}

impl ToolErrorCategory {
    /// Stable machine-readable error-code prefix (FR-024).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ValidationFailure => "validation",
            Self::PermissionDenied => "permission",
            Self::UnsupportedCapability => "unsupported_capability",
            Self::Timeout => "timeout",
            Self::ExecutionFailure => "execution",
            Self::InternalFailure => "internal",
        }
    }
}

/// Machine-readable metadata for a built-in tool (FR-022).
///
/// Supersedes the lightweight `agent_scope_workspace::ToolInfo` for built-ins
/// by adding availability, read-only, and concurrency attributes.
#[derive(Debug, Clone)]
pub struct BuiltInToolInfo {
    /// Public tool name (e.g. "Bash", "Edit").
    pub name: &'static str,
    /// Human-readable model-facing description.
    pub description: &'static str,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
    /// Whether the tool requires workspace access.
    pub requires_workspace: bool,
    /// Whether the tool requires a Windows shell (PowerShell).
    pub requires_windows_shell: bool,
    /// Whether the tool has no observable mutation side effects.
    pub read_only: bool,
    /// Whether parallel calls are supported.
    pub concurrency_safe: bool,
}

/// Shared execution context for built-in workspace tools.
///
/// Carries the workspace backend (all filesystem/process I/O happens through
/// it, enforcing containment) and the per-session tool state.
#[derive(Clone)]
pub struct BuiltInToolContext {
    /// Workspace backend handle — all I/O is confined to the workspace.
    pub backend: Arc<dyn WorkspaceBackend>,
    /// Workspace root path (used for containment checks and relative display).
    pub workdir: String,
    /// Per-session read-state + active-group store.
    pub session: Arc<RwLock<WorkspaceToolSession>>,
}

impl BuiltInToolContext {
    /// Create a new context bound to a workspace backend and workdir.
    #[must_use]
    pub fn new(
        backend: Arc<dyn WorkspaceBackend>,
        workdir: impl Into<String>,
        session: Arc<RwLock<WorkspaceToolSession>>,
    ) -> Self {
        Self {
            backend,
            workdir: workdir.into(),
            session,
        }
    }

    /// Normalize a user-supplied path and verify it stays inside the workdir
    /// (lexically). Symlink-escape checks are delegated to the backend's
    /// containment logic on the actual I/O call.
    pub fn resolve_in_workspace(&self, input: &str) -> Result<String, String> {
        let path = Path::new(input);
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            Path::new(&self.workdir).join(path)
        };
        let normalized = normalize_path(&joined);
        if !normalized.starts_with(&self.workdir) {
            return Err("path escapes workspace".to_string());
        }
        Ok(normalized.to_string_lossy().to_string())
    }
}

/// Remove `.` and resolve `..` components lexically.
fn normalize_path(path: &Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_in_workspace_lexical() {
        let backend: Arc<dyn WorkspaceBackend> =
            Arc::new(agent_scope_workspace::backend::LocalBackend::new());
        let ctx = BuiltInToolContext::new(
            backend,
            "/ws",
            Arc::new(RwLock::new(WorkspaceToolSession::new("ws-1"))),
        );
        assert!(ctx.resolve_in_workspace("a.txt").is_ok());
        assert!(ctx.resolve_in_workspace("/ws/a.txt").is_ok());
        // Parent traversal escapes.
        assert!(ctx.resolve_in_workspace("../a.txt").is_err());
        assert!(ctx.resolve_in_workspace("/etc/passwd").is_err());
    }

    #[test]
    fn error_category_str_stable() {
        assert_eq!(ToolErrorCategory::Timeout.as_str(), "timeout");
        assert_eq!(
            ToolErrorCategory::UnsupportedCapability.as_str(),
            "unsupported_capability"
        );
    }
}
