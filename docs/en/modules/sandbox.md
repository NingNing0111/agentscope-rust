# Sandbox / Sandbox

> One-liner: `agent_scope_sandbox` provides secure code execution isolation for Agents — `LocalSandboxSession` implements path traversal prevention, command execution timeouts, output limits, and explicit capability reporting, refusing pseudo-compatibility.

## 1. Module Overview (Overview)

This module implements a local reference implementation of the `SandboxSession` trait, strictly following Constitution Principle 5 (no pseudo-compatibility):

| Component | Responsibility |
|-----------|---------------|
| `SandboxSession` trait | Sandbox lifecycle interface: `initialize()`, `execute()`, `read_file()`, `write_file()`, `close()` |
| `LocalSandboxSession` | Local reference implementation with file isolation and controlled command execution |
| `CapabilityReport` | **Explicitly reports unsupported capabilities** (e.g., network isolation, resource limits) — never pretends |
| `SandboxPolicy` | Execution policy: allowed/denied paths, command whitelist, timeout/output limits |
| `SandboxMount` | Read-only/read-write mount point configuration |
| Path Security | `normpath` normalization, symlink escape detection, working directory confinement |

**When to use**: Agent needs to execute arbitrary code or shell commands; restricting Agent's filesystem access; recording command execution history.

**Prerequisites**: [Workspace](./workspace.md), [Agent System](./agent.md)

## 2. Core Concepts & Main Public Types (Core Concepts)

### 2.1 `SandboxSession` trait

```rust
#[async_trait]
pub trait SandboxSession: Send + Sync {
    fn session_id(&self) -> &str;
    fn state(&self) -> SandboxState;  // Created → Ready → Closing → Closed
    fn policy(&self) -> &SandboxPolicy;

    async fn initialize(&mut self) -> Result<(), SandboxError>;
    async fn execute(&mut self, request: ExecutionRequest) -> Result<ExecutionResult, SandboxError>;
    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, SandboxError>;
    async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), SandboxError>;
    async fn delete_file(&mut self, path: &str) -> Result<(), SandboxError>;
    async fn get_capabilities(&self) -> CapabilityReport;
    async fn close(&mut self) -> Result<(), SandboxError>;
}
```

### 2.2 `SandboxState` Lifecycle

```
Created → initialize() → Ready → close() → Closing → Closed
                                ↘ execution error → Failed
```

### 2.3 `SandboxPolicy`

| Field | Description |
|-------|-------------|
| `allow_unrestricted_filesystem` | Whether unrestricted file access is allowed |
| `command_timeout_seconds` | Command execution timeout (default 30s) |
| `max_output_bytes` | Maximum output bytes (default 100KB) |
| `allow_network` | Whether network access is allowed (**currently unsupported**, explicitly reported) |

### 2.4 `ExecutionRequest` and `ExecutionResult`

```rust
pub struct ExecutionRequest {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<String>,
}
pub struct ExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}
```

### 2.5 `CapabilityReport` — Anti-Pseudo-Compatibility

```rust
pub struct CapabilityReport {
    pub hard_isolation: bool,  // false — explicitly states no hard isolation
    pub network_isolation: bool, // false
    pub resource_limits: bool,  // false
    pub filesystem_isolation: bool, // true — path traversal prevention
    pub sandbox_type: String,  // "local-reference"
}
```

**Key design principle**: `LocalSandboxSession` never pretends to have capabilities it cannot provide. `network_isolation: false` explicitly tells callers this is soft isolation.

### 2.6 Path Security

- `normpath()` — resolves `.`, `..`, normalizes relative paths
- Symlink escape detection — rejects symlinks pointing outside the working directory
- All file operations confined to root_dir scope

## 3. Quick Example (Quick Example)

```rust
use agent_scope_sandbox::{LocalSandboxSession, LocalSandboxConfig, SandboxSession};

let config = LocalSandboxConfig::default();
let mut session = LocalSandboxSession::new(config)?;

session.initialize().await?;

// File operations (with path security checks)
session.write_file("notes/result.txt", b"hello").await?;
let data = session.read_file("notes/result.txt").await?;
assert_eq!(data, b"hello");

// Command execution (with timeout and output limits)
use agent_scope_sandbox::ExecutionRequest;
let result = session.execute(ExecutionRequest {
    command: "echo".into(),
    args: vec!["hello world".into()],
    env: Default::default(),
    working_dir: None,
}).await?;
assert_eq!(result.stdout.trim(), "hello world");

// Explicitly check capabilities
let caps = session.get_capabilities().await;
assert!(!caps.network_isolation); // local reference has no network isolation

session.close().await?;
```

## 4. Key Usage Patterns (Usage Patterns)

### 4.1 Path Safety Validation

`LocalSandboxSession` internally normalizes and validates all paths:
- Rejects absolute paths
- Rejects `..` path traversal
- Rejects symlinks pointing outside the working directory

```rust
// ✅ Allowed
session.read_file("notes/data.txt").await

// ❌ Rejected (path traversal)
session.read_file("../../etc/passwd").await  // → SandboxError::PathTraversal
```

### 4.2 Command Execution with Timeout

```rust
let result = session.execute(ExecutionRequest {
    command: "sleep".into(),
    args: vec!["60".into()],  // exceeds policy.command_timeout_seconds
    ..Default::default()
}).await;
// → command is killed on timeout, returns SandboxError::Timeout
```

### 4.3 Execution History

`LocalSandboxSession` internally maintains a `Vec<ExecutionRecord>`, logging each command execution's timing, arguments, and results.

### 4.4 Mount Points

```rust
use agent_scope_sandbox::SandboxMount;
let config = LocalSandboxConfig {
    mounts: vec![
        SandboxMount::read_only("/data/public"),
        SandboxMount::read_write("/data/agent-scratch"),
    ],
    ..Default::default()
};
```

## 5. Errors & Unsupported Capabilities (Errors & Unsupported)

| Error | Cause |
|-------|-------|
| `SandboxError::PathTraversal` | Path escape or symlink escape attempt |
| `SandboxError::Timeout` | Command execution timed out |
| `SandboxError::OutputLimitExceeded` | Output exceeded `max_output_bytes` |
| `SandboxError::PermissionDenied` | Violated `SandboxPolicy` |
| `SandboxError::SessionClosed` | Operating on a closed session |
| `SandboxError::IoError` | Underlying filesystem error |

### Explicitly Unsupported Capabilities

`LocalSandboxSession` **explicitly reports** the following unsupported features via `CapabilityReport`:

| Capability | Status | Notes |
|------------|--------|-------|
| Hard Isolation | ❌ | Process-level, not container/VM-level |
| Network Isolation | ❌ | No network namespace isolation |
| Resource Limits (CPU/memory) | ❌ | No cgroup-level limits |
| Filesystem Isolation | ✅ | Path normalization + symlink detection |

This follows Constitution Principle 5 — **no pseudo-compatibility, no silent degradation**.

## 6. Compatibility (Compatibility)

- **Compatibility Level**: **L2** (core sandbox behavior, reference implementation)
- **Authority**: `specs/017-sandbox-feature/spec.md`
- **Known Deviations**:
  - `LocalSandboxSession` is a reference implementation, not a production-grade sandbox
  - Hard isolation (Docker/VM) is not supported and is explicitly reported
  - Python side may use Docker or other external sandboxes

## 7. See Also (Related Modules)

- [Workspace](./workspace.md) — workspace provides file/tool abstractions; sandbox adds isolation
- [Agent System](./agent.md) — Agents execute code securely through sandbox
- [Tool System](./tool.md) — Bash/Edit/Write inside sandbox exposed as Tools
