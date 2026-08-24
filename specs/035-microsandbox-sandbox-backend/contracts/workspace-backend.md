# Contract: Sandbox Workspace Backend

**Feature**: `035-microsandbox-sandbox-backend`

## Purpose

`SandboxWorkspaceBackend` adapts any `SandboxSession` to `agent_scope_workspace::WorkspaceBackend`, allowing workspace tools to execute commands and file operations inside a shared sandbox boundary.

## Required API

```rust
pub struct SandboxWorkspaceBackend {
    session: std::sync::Arc<tokio::sync::Mutex<Box<dyn SandboxSession>>>,
    instructions: String,
}

impl SandboxWorkspaceBackend {
    pub fn new(session: LocalSandboxSession) -> Self;

    pub fn from_session<S>(session: S) -> Self
    where
        S: SandboxSession + 'static;

    pub fn from_boxed_session(session: Box<dyn SandboxSession>) -> Self;

    pub fn instructions(&self) -> &str;
    pub async fn initialize(&self) -> Result<(), WorkspaceError>;
    pub async fn close(&self) -> Result<(), WorkspaceError>;
}
```

## Compatibility requirements

- `new(LocalSandboxSession)` remains source-compatible.
- Existing local-process workspace tests continue to pass.
- Instructions must not depend on `LocalSandboxSession::workdir()` because other session types may not expose a host workdir.
- `WorkspaceBackend` methods delegate to `SandboxSession` trait methods, not backend-specific APIs.

## Error mapping

`SandboxError::PermissionDenied` maps to `WorkspaceError::PathTraversal` for path-like denials. Other sandbox errors map to `WorkspaceError::GatewayError` with the sandbox error category and display message.

## Tool behavior

Bash, Read, Write, Edit, Grep, Glob, and ResetTools should work through the existing `WorkspaceBackend` trait without backend-specific changes. A microsandbox-backed workspace shares one session filesystem across those tools.
