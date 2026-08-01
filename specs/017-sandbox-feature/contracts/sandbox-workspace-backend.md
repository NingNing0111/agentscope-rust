# Contract: Sandbox Workspace Backend

**Feature**: 017-sandbox-feature
**Implements**: `agent_scope_workspace::WorkspaceBackend`
**Adapter**: `agent_scope_sandbox::SandboxWorkspaceBackend`

## Interface

```rust
pub struct SandboxWorkspaceBackend {
    pub session: Arc<dyn SandboxSessionHandle>,
    pub workspace_root: PathBuf,
}

#[async_trait::async_trait]
impl WorkspaceBackend for SandboxWorkspaceBackend {
    async fn exec_shell(
        &self,
        cmd: &[&str],
        cwd: &str,
        timeout_secs: Option<f64>,
    ) -> Result<ExecOutput, WorkspaceError>;

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, WorkspaceError>;
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), WorkspaceError>;
    async fn is_dir(&self, path: &str) -> Result<bool, WorkspaceError>;
    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, WorkspaceError>;
    async fn delete_path(&self, path: &str) -> Result<(), WorkspaceError>;
    async fn file_exists(&self, path: &str) -> Result<bool, WorkspaceError>;
    fn join_path(&self, a: &str, b: &str) -> String;
    fn basename(&self, path: &str) -> String;
    fn dirname(&self, path: &str) -> String;
    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, WorkspaceError>;
    fn normpath(&self, path: &str) -> String;
    fn is_absolute(&self, path: &str) -> bool;
}
```

## Mapping Rules

| WorkspaceBackend method | Sandbox operation | Required behavior |
|-------------------------|-------------------|-------------------|
| `exec_shell` | `execute(ExecutionRequest)` | Preserve stdout/stderr/exit_code; map timeout to `WorkspaceError::GatewayError` or typed sandbox variant if added |
| `read_file` | `SandboxSession::read_file` | Reject closed session and path escape |
| `write_file` | `SandboxSession::write_file` | Auto-create allowed parents; reject read-only mount writes |
| `delete_path` | `SandboxSession::delete_path` | Idempotent for missing paths; reject read-only mount deletes |
| `list_dir` | `SandboxSession::list_dir` | Non-existent path returns empty Vec |
| `file_exists` | sandbox metadata lookup | Must not leak host existence outside sandbox |
| `stat_mtime` | sandbox metadata lookup | Returns `Ok(None)` for absent files |

## Workspace Integration Guarantees

- `LocalWorkspace` or future workspace constructors can select `SandboxWorkspaceBackend` without changing the public Workspace tool schemas.
- `Workspace reset()` must close or reset the associated sandbox session so there are no orphan processes or temporary resources.
- Workspace instructions must clearly mention the sandbox workdir and policy restrictions.
- All errors caused by sandbox policy must remain programmatically distinguishable from generic I/O failures.

## Compatibility Notes

- The adapter targets L2 core behavior compatibility with Python AgentScope workspace sandbox behavior.
- Host-platform limitations (for example unavailable network namespace) must be surfaced through `CapabilityReport`, not hidden behind local execution.
