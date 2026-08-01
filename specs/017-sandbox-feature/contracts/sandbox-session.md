# Contract: Sandbox Session API

**Feature**: 017-sandbox-feature
**Crate**: `agent_scope_sandbox`

## Public Types

```rust
#[async_trait::async_trait]
pub trait SandboxSession: Send + Sync {
    fn session_id(&self) -> &str;
    fn state(&self) -> SandboxState;
    fn policy(&self) -> &SandboxPolicy;

    async fn initialize(&mut self) -> Result<(), SandboxError>;
    async fn execute(&mut self, request: ExecutionRequest) -> Result<ExecutionResult, SandboxError>;
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, SandboxError>;
    async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), SandboxError>;
    async fn delete_path(&mut self, path: &str) -> Result<(), SandboxError>;
    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, SandboxError>;
    async fn history(&self) -> Result<Vec<ExecutionRecord>, SandboxError>;
    async fn capability_report(&self) -> Result<CapabilityReport, SandboxError>;
    async fn close(&mut self) -> Result<(), SandboxError>;
    async fn cleanup(&mut self) -> Result<(), SandboxError>;
}
```

## Construction

```rust
pub struct LocalSandboxConfig {
    pub session_id: Option<String>,
    pub root_dir: Option<PathBuf>,
    pub workdir: Option<PathBuf>,
    pub policy: SandboxPolicy,
    pub mounts: Vec<SandboxMount>,
}

impl LocalSandboxSession {
    pub fn new(config: LocalSandboxConfig) -> Result<Self, SandboxError>;
}
```

## Contract Guarantees

| Guarantee | Detail |
|-----------|--------|
| Lifecycle safety | `execute/read/write/list/delete` return lifecycle error after `close()` |
| No silent fallback | unavailable isolation features return `UnsupportedFeature` or appear in `CapabilityReport.unsupported` |
| Non-zero exit | command exit code != 0 returns `Ok(ExecutionResult { status: Exited { code } })` |
| Timeout cleanup | timed-out commands are killed or marked as failed cleanup with diagnostic error |
| Output bound | inline stdout/stderr never exceed configured output limit |
| Path containment | all file operations reject traversal and symlink escape |
| Auditable order | every accepted execute request gets monotonic `ExecutionRecord.sequence` |
| Idempotent cleanup | repeated `close()`/`cleanup()` calls are stable |

## Error Contract

```rust
pub enum SandboxError {
    ValidationError { message: String },
    LifecycleError { state: SandboxState, operation: String },
    PermissionDenied { path: Option<String>, operation: String },
    TimeoutError { execution_id: String, timeout: Duration },
    UnsupportedFeature { feature: String, reason: String },
    SandboxUnavailable { backend: String, reason: String },
    IoError { operation: String, message: String },
    InternalError { message: String },
}
```

Errors must not include API keys, raw sensitive environment variable values, or unredacted secrets.
