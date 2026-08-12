//! Workspace error types.

use thiserror::Error;

/// Errors returned by workspace operations.
#[derive(Debug, Clone, Error)]
pub enum WorkspaceError {
    /// Backend I/O error.
    #[error("backend error: {message}")]
    BackendError { message: String },
    /// Workspace not initialized.
    #[error("workspace not initialized")]
    NotInitialized,
    /// Already initialized.
    #[error("workspace already initialized")]
    AlreadyInitialized,
    /// SKILL.md is invalid or missing.
    #[error("invalid skill at {path}: {reason}")]
    InvalidSkill { path: String, reason: String },
    /// Skill name was not found in the index.
    #[error("skill not found: {name}")]
    SkillNotFound { name: String },
    /// MCP client name was not found.
    #[error("MCP not found: {name}")]
    McpNotFound { name: String },
    /// MCP client with this name already registered.
    #[error("MCP already exists: {name}")]
    McpAlreadyExists { name: String },
    /// MCP connection failed (transport, handshake, or protocol error).
    /// `reason` MUST NOT contain authentication secrets (FR-009).
    #[error("MCP connection failed for '{name}': {reason}")]
    McpConnectionError { name: String, reason: String },
    /// MCP tool call failed on the server.
    /// `reason` MUST NOT contain authentication secrets (FR-009).
    #[error("MCP tool call failed: '{mcp_name}/{tool_name}': {reason}")]
    McpCallError {
        mcp_name: String,
        tool_name: String,
        reason: String,
    },
    /// MCP client is not connected — call `connect_mcp()` first.
    #[error("MCP client not connected: {name}")]
    McpNotConnected { name: String },
    /// Path traversal attack detected.
    #[error("path traversal detected: {path}")]
    PathTraversal { path: String },
    /// .mcp file is corrupted.
    #[error("corrupt .mcp file at {path}: {message}")]
    CorruptMcpFile { path: String, message: String },
    /// Placeholder for sandbox gateway errors.
    #[error("gateway error: {message}")]
    GatewayError { message: String },
    /// Context/tool-result offload failed.
    #[error("offload error: {message}")]
    OffloadError { message: String },
}
