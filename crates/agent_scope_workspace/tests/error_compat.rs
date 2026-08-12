use std::error::Error;

use agent_scope_workspace::WorkspaceError;

#[test]
fn display_text_for_workspace_errors_is_stable() {
    let cases = [
        (
            WorkspaceError::BackendError {
                message: "disk full".into(),
            },
            "backend error: disk full",
        ),
        (WorkspaceError::NotInitialized, "workspace not initialized"),
        (
            WorkspaceError::AlreadyInitialized,
            "workspace already initialized",
        ),
        (
            WorkspaceError::InvalidSkill {
                path: "skills/demo/SKILL.md".into(),
                reason: "missing name".into(),
            },
            "invalid skill at skills/demo/SKILL.md: missing name",
        ),
        (
            WorkspaceError::SkillNotFound {
                name: "demo".into(),
            },
            "skill not found: demo",
        ),
        (
            WorkspaceError::McpNotFound {
                name: "draw".into(),
            },
            "MCP not found: draw",
        ),
        (
            WorkspaceError::McpAlreadyExists {
                name: "draw".into(),
            },
            "MCP already exists: draw",
        ),
        (
            WorkspaceError::McpConnectionError {
                name: "draw".into(),
                reason: "handshake failed".into(),
            },
            "MCP connection failed for 'draw': handshake failed",
        ),
        (
            WorkspaceError::McpCallError {
                mcp_name: "draw".into(),
                tool_name: "create".into(),
                reason: "bad args".into(),
            },
            "MCP tool call failed: 'draw/create': bad args",
        ),
        (
            WorkspaceError::McpNotConnected {
                name: "draw".into(),
            },
            "MCP client not connected: draw",
        ),
        (
            WorkspaceError::PathTraversal {
                path: "../secret".into(),
            },
            "path traversal detected: ../secret",
        ),
        (
            WorkspaceError::CorruptMcpFile {
                path: ".mcp".into(),
                message: "invalid json".into(),
            },
            "corrupt .mcp file at .mcp: invalid json",
        ),
        (
            WorkspaceError::GatewayError {
                message: "denied".into(),
            },
            "gateway error: denied",
        ),
        (
            WorkspaceError::OffloadError {
                message: "too large".into(),
            },
            "offload error: too large",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none(), "unexpected source for {expected}");
    }
}
