//! Workspace error types.

use std::fmt;

/// Errors returned by workspace operations.
#[derive(Debug, Clone)]
pub enum WorkspaceError {
    /// Backend I/O error.
    BackendError { message: String },
    /// Workspace not initialized.
    NotInitialized,
    /// Already initialized.
    AlreadyInitialized,
    /// SKILL.md is invalid or missing.
    InvalidSkill { path: String, reason: String },
    /// Skill name was not found in the index.
    SkillNotFound { name: String },
    /// MCP client name was not found.
    McpNotFound { name: String },
    /// MCP client with this name already registered.
    McpAlreadyExists { name: String },
    /// MCP connection failed (transport, handshake, or protocol error).
    /// `reason` MUST NOT contain authentication secrets (FR-009).
    McpConnectionError { name: String, reason: String },
    /// MCP tool call failed on the server.
    /// `reason` MUST NOT contain authentication secrets (FR-009).
    McpCallError {
        mcp_name: String,
        tool_name: String,
        reason: String,
    },
    /// MCP client is not connected — call `connect_mcp()` first.
    McpNotConnected { name: String },
    /// Path traversal attack detected.
    PathTraversal { path: String },
    /// .mcp file is corrupted.
    CorruptMcpFile { path: String, message: String },
    /// Placeholder for sandbox gateway errors.
    GatewayError { message: String },
    /// Context/tool-result offload failed.
    OffloadError { message: String },
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceError::BackendError { message } => write!(f, "backend error: {message}"),
            WorkspaceError::NotInitialized => write!(f, "workspace not initialized"),
            WorkspaceError::AlreadyInitialized => write!(f, "workspace already initialized"),
            WorkspaceError::InvalidSkill { path, reason } => {
                write!(f, "invalid skill at {path}: {reason}")
            }
            WorkspaceError::SkillNotFound { name } => write!(f, "skill not found: {name}"),
            WorkspaceError::McpNotFound { name } => write!(f, "MCP not found: {name}"),
            WorkspaceError::McpAlreadyExists { name } => write!(f, "MCP already exists: {name}"),
            WorkspaceError::McpConnectionError { name, reason } => {
                write!(f, "MCP connection failed for '{name}': {reason}")
            }
            WorkspaceError::McpCallError {
                mcp_name,
                tool_name,
                reason,
            } => write!(
                f,
                "MCP tool call failed: '{mcp_name}/{tool_name}': {reason}"
            ),
            WorkspaceError::McpNotConnected { name } => {
                write!(f, "MCP client not connected: {name}")
            }
            WorkspaceError::PathTraversal { path } => write!(f, "path traversal detected: {path}"),
            WorkspaceError::CorruptMcpFile { path, message } => {
                write!(f, "corrupt .mcp file at {path}: {message}")
            }
            WorkspaceError::GatewayError { message } => write!(f, "gateway error: {message}"),
            WorkspaceError::OffloadError { message } => write!(f, "offload error: {message}"),
        }
    }
}

impl std::error::Error for WorkspaceError {}
