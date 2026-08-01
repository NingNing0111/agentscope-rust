# Quickstart: Sandbox Feature

**Feature**: 017-sandbox-feature | **Date**: 2026-08-01

## Prerequisites

- Rust toolchain with workspace edition 2024 support
- Project workspace root: `agentscope-rust/`
- New crate target: `crates/agent_scope_sandbox/`
- Existing integration target: `crates/agent_scope_workspace/`

## Validation Scenarios

### Scenario 1: Create a sandbox session and execute a command (US1)

```shell
rtk cargo test -p agent_scope_sandbox -- sandbox_session_execute
```

**Evidence (2026-08-01)**: `rtk cargo test -p agent_scope_sandbox` -> 18 passed.

**Security review evidence (2026-08-01)**: independent reviewer PASS after fixes for explicit network support, timeout kill/wait, sandbox-backed workspace metadata, honest capability reporting, and regression coverage.

**Expected outcome**:
- `initialize()` creates a unique session root and workdir.
- Executing `printf hello` returns `ExecutionStatus::Exited { code: 0 }`.
- Result contains exit code, stdout summary, stderr summary, start/finish time, and duration.
- A matching `ExecutionRecord` is appended with sequence `1`.

### Scenario 2: File writes stay inside the sandbox (US1)

```shell
rtk cargo test -p agent_scope_sandbox -- sandbox_file_isolation
```

**Expected outcome**:
- Writing `notes/result.txt` succeeds inside the sandbox workdir.
- Reading the same relative path through the same session returns the written bytes.
- The file is not created in the repository or caller cwd.
- After `cleanup()` with `keep_on_close = false`, temporary session files are removed.

### Scenario 3: Timeout and non-zero exit are distinct (US1/US3)

```shell
rtk cargo test -p agent_scope_sandbox -- sandbox_execution_status
```

**Expected outcome**:
- A command that exits `7` returns `Ok(ExecutionResult)` with `Exited { code: 7 }`.
- A command exceeding its timeout returns `TimedOut` or `TimeoutError` and does not leave a running child process.
- Spawn/backend failures return sandbox system errors, not fake command results.

### Scenario 4: Reject path traversal, symlink escape, and read-only writes (US2/US3)

```shell
cargo test -p agent_scope_sandbox -- sandbox_path_policy
```

**Expected outcome**:
- `../outside.txt` is rejected.
- A symlink pointing outside the sandbox is rejected.
- Writing to a read-only mount returns `PermissionDenied`.
- Failed attempts are visible in structured errors and, when associated with command execution, in execution history.

### Scenario 5: Workspace uses sandbox backend (US2)

```shell
cargo test -p agent_scope_workspace -- sandbox_backend
```

**Expected outcome**:
- Workspace `Bash`, `Read`, `Write`, `Edit`, `Glob`, and `Grep` operations use the same sandbox boundary.
- Files written through Workspace can be read by the same Workspace and are not visible outside authorized paths.
- Workspace `reset()` or `close()` closes/cleans the associated sandbox session.

### Scenario 6: Output limit writes full output reference (US3/US4)

```shell
cargo test -p agent_scope_sandbox -- sandbox_output_limit
```

**Expected outcome**:
- Large stdout/stderr are truncated in inline summaries.
- `truncated = true` and `full_ref` points to sandbox-owned output files.
- Output references include byte count and sha256.

### Scenario 7: Capability report exposes unsupported features (US3/US4)

```shell
cargo test -p agent_scope_sandbox -- sandbox_capability_report
```

**Expected outcome**:
- Local MVP reports supported features such as temp root, path containment, timeout, output limits, and audit history.
- Unsupported hard isolation features such as CPU/memory/network enforcement are reported explicitly when unavailable.
- Requests for unsupported limits return `UnsupportedFeature` instead of silently degrading.

### Scenario 8: Concurrent sessions are isolated (US4)

```shell
cargo test -p agent_scope_sandbox -- sandbox_concurrent_sessions
```

**Expected outcome**:
- 20 sessions can concurrently create the same relative filename with distinct content.
- No session can read another session's file, environment variable, or execution history.
- Closing all sessions leaves no active session state or running child processes.

### Scenario 9: Full workspace regression gate

```shell
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

**Expected outcome**:
- Workspace builds with the new sandbox crate.
- All tests pass.
- Clippy reports zero warnings.
- Formatting is clean.
