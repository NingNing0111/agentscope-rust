# Contract: Microsandbox Session

**Feature**: `035-microsandbox-sandbox-backend`

## Purpose

`MicrosandboxSession` is a feature-gated implementation of the existing `SandboxSession` trait. It creates and manages a microsandbox microVM while preserving the public sandbox lifecycle, execution, filesystem, history, capability, and error contracts.

## Public API

```rust
pub struct MicrosandboxSession { /* SDK handle hidden */ }

impl MicrosandboxSession {
    pub fn new(config: MicrosandboxConfig) -> Result<Self, SandboxError>;
}

#[async_trait::async_trait]
impl SandboxSession for MicrosandboxSession { /* ... */ }
```

## Lifecycle contract

- `new(config)` validates configuration and policy but does not create a VM.
- `initialize()` creates the microsandbox VM and transitions from `Created` to `Ready`.
- SDK/runtime/platform/image failures return `SandboxError::SandboxUnavailable { backend: "microsandbox", reason }`.
- Unsupported policy/mount/network/resource requests return `SandboxError::UnsupportedFeature`.
- No failure path may instantiate or execute `LocalSandboxSession` as fallback.
- `close()` and `cleanup()` are idempotent.
- Persistent behavior must be explicit through `config.persist` or `policy.keep_on_close`.

## Execution contract

- Empty argv is a validation error.
- Timeout must not exceed `policy.max_timeout`.
- Non-zero process exit is `ExecutionStatus::Exited { code }`.
- Timeout is `ExecutionStatus::TimedOut` or stable timeout error with diagnostic history where possible.
- SDK/system errors are sandbox errors, not command exit codes.
- Command summaries must use redacted env keys only.

## Filesystem contract

- `read_file`, `write_file`, `delete_path`, `is_dir`, `stat_mtime`, and `list_dir` use microsandbox fs API or documented equivalent.
- Guest paths are sanitized without host canonicalization.
- `..`, empty paths, and unauthorized absolute paths are rejected.
- Read-only mount writes are rejected. If SDK cannot enforce read-only mounts, initialization rejects that mount as unsupported.

## Output contract

- `stdout.inline` and `stderr.inline` are limited to `policy.max_output_bytes`.
- Truncated outputs provide full output refs with bytes and SHA-256.
- Sandbox output, logs, and files are untrusted data and must not be interpreted as instructions.
