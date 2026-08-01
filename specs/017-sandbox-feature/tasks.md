# Tasks: Sandbox Feature（代码执行沙箱）

**Input**: Design documents from `/specs/017-sandbox-feature/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md, constitution.md

**Tests**: 已包含测试任务，因为 spec.md Success Criteria 和 quickstart.md 明确要求 unit/integration/concurrency/security/compatibility 验证。测试任务应先写，并在对应实现前确认失败或无法编译。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- New sandbox crate: `crates/agent_scope_sandbox/`
- Existing workspace integration: `crates/agent_scope_workspace/`
- Compatibility artifacts: `specs/001-compatibility-baseline/`
- Feature artifacts: `specs/017-sandbox-feature/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the sandbox crate skeleton and wire it into the Rust workspace.

- [X] T001 Create `crates/agent_scope_sandbox/Cargo.toml` with package metadata and dependencies on `async-trait`, `chrono`, `serde`, `sha2`, `tokio`, `tracing`, `uuid`, and dev-dependency `tempfile`
- [X] T002 Add `agent_scope_sandbox = { path = "crates/agent_scope_sandbox" }` to the root package dependencies in `Cargo.toml`
- [X] T003 Create `crates/agent_scope_sandbox/src/lib.rs` with `#![deny(unsafe_code)]`, module declarations, and public re-exports for sandbox API types
- [X] T004 [P] Create empty module files in `crates/agent_scope_sandbox/src/error.rs`, `policy.rs`, `mount.rs`, `session.rs`, `execution.rs`, `capability.rs`, `local.rs`, `path.rs`, and `workspace_backend.rs`
- [X] T005 [P] Create integration test files in `crates/agent_scope_sandbox/tests/session_tests.rs`, `file_isolation_tests.rs`, `policy_tests.rs`, `audit_tests.rs`, and `concurrency_tests.rs`
- [X] T006 [P] Create workspace integration test file `crates/agent_scope_workspace/tests/sandbox_backend_tests.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core public data types, error model, path primitives, and backend skeleton required before any story implementation.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T007 Implement typed `SandboxError` and stable error category helpers in `crates/agent_scope_sandbox/src/error.rs`
- [X] T008 [P] Implement `SandboxPolicy`, `NetworkPolicy`, `CpuLimit`, defaults, validation, and serde support in `crates/agent_scope_sandbox/src/policy.rs`
- [X] T009 [P] Implement `SandboxMount`, `MountAccess`, `MountOwner`, mount validation, and serde support in `crates/agent_scope_sandbox/src/mount.rs`
- [X] T010 [P] Implement `ExecutionRequest`, `ExecutionResult`, `ExecutionStatus`, `ExecutionRecord`, `OutputSummary`, `OutputRef`, and `ResourceLimitHit` in `crates/agent_scope_sandbox/src/execution.rs`
- [X] T011 [P] Implement `CapabilityReport`, `SandboxCapability`, `UnsupportedCapability`, and `CompatibilityLevel` in `crates/agent_scope_sandbox/src/capability.rs`
- [X] T012 Implement `SandboxState`, `LocalSandboxConfig`, and `SandboxSession` trait matching `contracts/sandbox-session.md` in `crates/agent_scope_sandbox/src/session.rs`
- [X] T013 Implement `SandboxPathResolver` skeleton for root/workdir joining, parent canonicalization, and scope checks in `crates/agent_scope_sandbox/src/path.rs`
- [X] T014 Implement `LocalSandboxSession` struct skeleton with session id, root/workdir, state, policy, mounts, history, timestamps, and constructor validation in `crates/agent_scope_sandbox/src/local.rs`
- [X] T015 Implement internal output reference writer skeleton for `stdout`/`stderr` audit files in `crates/agent_scope_sandbox/src/local.rs`
- [X] T016 Implement redacted command summary helper that hides sensitive environment values in `crates/agent_scope_sandbox/src/execution.rs`
- [X] T017 Add serialization round-trip tests for policy, mount, execution, and capability types in `crates/agent_scope_sandbox/tests/session_tests.rs`
- [X] T018 Register Sandbox target compatibility level and initial unsupported deviations in `specs/001-compatibility-baseline/capability-matrix.json`

**Checkpoint**: Foundation ready - public types compile, serde contracts are testable, and user story implementation can begin.

---

## Phase 3: User Story 1 - 在受控沙箱中执行命令 (Priority: P1) 🎯 MVP

**Goal**: Developers can create a sandbox session, execute commands in its isolated workdir, read/write sandbox files, observe structured results, and clean resources safely.

**Independent Test**: Create a sandbox session, run `printf hello`, write/read a file under the sandbox workdir, verify the file is not created in caller cwd, close/cleanup the session, and verify operations after close return lifecycle errors.

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T019 [P] [US1] Add command execution success and non-zero exit tests in `crates/agent_scope_sandbox/tests/session_tests.rs`
- [X] T020 [P] [US1] Add lifecycle initialize/close/cleanup idempotency and closed-session rejection tests in `crates/agent_scope_sandbox/tests/session_tests.rs`
- [X] T021 [P] [US1] Add sandbox read/write/delete/list file operation tests in `crates/agent_scope_sandbox/tests/file_isolation_tests.rs`
- [X] T022 [P] [US1] Add command timeout cleanup test for a long-running child process in `crates/agent_scope_sandbox/tests/policy_tests.rs`

### Implementation for User Story 1

- [X] T023 [US1] Implement `LocalSandboxSession::initialize`, temp root/workdir creation, and Created→Ready transitions in `crates/agent_scope_sandbox/src/local.rs`
- [X] T024 [US1] Implement `LocalSandboxSession::close` and `cleanup` idempotent transitions and temp directory removal in `crates/agent_scope_sandbox/src/local.rs`
- [X] T025 [US1] Implement lifecycle guard for execute/read/write/list/delete/history/capability operations in `crates/agent_scope_sandbox/src/local.rs`
- [X] T026 [US1] Implement argv validation, cwd resolution, env application, stdin support, and `tokio::process::Command` spawn in `crates/agent_scope_sandbox/src/local.rs`
- [X] T027 [US1] Implement timeout handling with kill/wait and `ExecutionStatus::TimedOut` or `SandboxError::TimeoutError` diagnostics in `crates/agent_scope_sandbox/src/local.rs`
- [X] T028 [US1] Implement stdout/stderr capture into `OutputSummary` without exceeding configured inline limit in `crates/agent_scope_sandbox/src/local.rs`
- [X] T029 [US1] Implement `ExecutionRecord` append with monotonic sequence for each accepted execute request in `crates/agent_scope_sandbox/src/local.rs`
- [X] T030 [US1] Implement sandbox `read_file`, `write_file`, `delete_path`, and `list_dir` operations in `crates/agent_scope_sandbox/src/local.rs`
- [X] T031 [US1] Implement missing path and directory listing behavior consistent with contracts in `crates/agent_scope_sandbox/src/local.rs`
- [X] T032 [US1] Expose US1 public API re-exports and documentation comments in `crates/agent_scope_sandbox/src/lib.rs`

**Checkpoint**: User Story 1 is fully functional and independently testable with `cargo test -p agent_scope_sandbox -- sandbox_session_execute`, `sandbox_file_isolation`, and `sandbox_execution_status`.

---

## Phase 4: User Story 2 - 将 Workspace 绑定到沙箱后端 (Priority: P2)

**Goal**: Existing Workspace operations can use a sandbox backend so Bash/Read/Write/Edit/Grep style tools share one sandbox boundary.

**Independent Test**: Create a Workspace using `SandboxWorkspaceBackend`, write/read files and execute commands through the workspace backend, and verify all operations stay inside the sandbox workdir and reset/close cleanup occurs.

### Tests for User Story 2 ⚠️

- [X] T033 [P] [US2] Add `exec_shell`, `read_file`, and `write_file` sandbox backend integration tests in `crates/agent_scope_workspace/tests/sandbox_backend_tests.rs`
- [X] T034 [P] [US2] Add `list_dir`, `file_exists`, `is_dir`, `delete_path`, and `stat_mtime` sandbox backend tests in `crates/agent_scope_workspace/tests/sandbox_backend_tests.rs`
- [X] T035 [P] [US2] Add workspace path traversal and symlink escape rejection tests in `crates/agent_scope_workspace/tests/sandbox_backend_tests.rs`
- [X] T036 [P] [US2] Add workspace reset/close sandbox cleanup test in `crates/agent_scope_workspace/tests/sandbox_backend_tests.rs`

### Implementation for User Story 2

- [X] T037 [US2] Add `agent_scope_sandbox` dev-dependency or integration dependency to `crates/agent_scope_workspace/Cargo.toml`
- [X] T038 [US2] Implement `SandboxWorkspaceBackend` struct, constructor, and session ownership model in `crates/agent_scope_sandbox/src/workspace_backend.rs`
- [X] T039 [US2] Implement `WorkspaceBackend::exec_shell` mapping to `ExecutionRequest` and `ExecOutput` in `crates/agent_scope_sandbox/src/workspace_backend.rs`
- [X] T040 [US2] Implement `WorkspaceBackend` file methods by delegating to sandbox read/write/list/delete/stat operations in `crates/agent_scope_sandbox/src/workspace_backend.rs`
- [X] T041 [US2] Implement `join_path`, `basename`, `dirname`, `normpath`, and `is_absolute` semantics for workspace-visible paths in `crates/agent_scope_sandbox/src/workspace_backend.rs`
- [X] T042 [US2] Implement conversion from `SandboxError` to `agent_scope_workspace::WorkspaceError` preserving stable sandbox categories in `crates/agent_scope_sandbox/src/workspace_backend.rs`
- [X] T043 [US2] Add sandbox workspace instructions text describing workdir, mounts, and refused operations in `crates/agent_scope_sandbox/src/workspace_backend.rs`

**Checkpoint**: User Stories 1 and 2 work independently; workspace backend integration passes `cargo test -p agent_scope_workspace -- sandbox_backend`.

---

## Phase 5: User Story 3 - 限制资源与网络访问 (Priority: P3)

**Goal**: Sandbox policies enforce or explicitly reject execution timeout, output size, writable roots, read-only mounts, network policy, and unsupported resource limits.

**Independent Test**: Create policy-constrained sessions and verify timeout, output truncation, path boundary, network policy, read-only mount writes, and unsupported CPU/memory/process limits return stable outcomes.

### Tests for User Story 3 ⚠️

- [X] T044 [P] [US3] Add output truncation and full output reference tests in `crates/agent_scope_sandbox/tests/policy_tests.rs`
- [X] T045 [P] [US3] Add path traversal, canonicalization, and symlink escape tests in `crates/agent_scope_sandbox/tests/file_isolation_tests.rs`
- [X] T046 [P] [US3] Add read-only mount write/delete denial tests in `crates/agent_scope_sandbox/tests/file_isolation_tests.rs`
- [X] T047 [P] [US3] Add unsupported CPU/memory/process/network policy tests in `crates/agent_scope_sandbox/tests/policy_tests.rs`
- [X] T048 [P] [US3] Add capability report tests for supported and unsupported local sandbox features in `crates/agent_scope_sandbox/tests/policy_tests.rs`

### Implementation for User Story 3

- [X] T049 [US3] Complete path resolver canonicalization for existing paths, missing leaf parents, `..`, repeated separators, and symlink escape in `crates/agent_scope_sandbox/src/path.rs`
- [X] T050 [US3] Implement longest-prefix mount resolution and read-only/read-write permission checks in `crates/agent_scope_sandbox/src/mount.rs`
- [X] T051 [US3] Wire path and mount permission checks into all file operations in `crates/agent_scope_sandbox/src/local.rs`
- [X] T052 [US3] Implement output truncation, full output audit file writing, SHA-256 digest, byte counts, and `OutputRef` paths in `crates/agent_scope_sandbox/src/local.rs`
- [X] T053 [US3] Enforce `default_timeout`, `max_timeout`, and per-request timeout validation in `crates/agent_scope_sandbox/src/local.rs`
- [X] T054 [US3] Implement explicit `UnsupportedFeature` handling for unavailable CPU, memory, process, and network hard-isolation policies in `crates/agent_scope_sandbox/src/local.rs`
- [X] T055 [US3] Implement accurate local backend `CapabilityReport` supported/unsupported/deviation entries in `crates/agent_scope_sandbox/src/capability.rs`

**Checkpoint**: User Story 3 policy and security behavior passes `cargo test -p agent_scope_sandbox -- sandbox_path_policy`, `sandbox_output_limit`, and `sandbox_capability_report`.

---

## Phase 6: User Story 4 - 记录可审计的沙箱执行历史 (Priority: P4)

**Goal**: Debuggers and maintainers can query ordered sandbox execution history with status, duration, failure categories, output summaries, and output references.

**Independent Test**: Execute multiple commands, including success, non-zero exit, timeout, and output-heavy commands, then query history and verify order, stable categories, summaries, and output references are complete and do not expose sensitive env values.

### Tests for User Story 4 ⚠️

- [X] T056 [P] [US4] Add ordered execution history tests for multiple commands in `crates/agent_scope_sandbox/tests/audit_tests.rs`
- [X] T057 [P] [US4] Add failure category audit tests for timeout, permission denied, unsupported feature, and sandbox errors in `crates/agent_scope_sandbox/tests/audit_tests.rs`
- [X] T058 [P] [US4] Add sensitive environment value redaction tests for command summaries and records in `crates/agent_scope_sandbox/tests/audit_tests.rs`
- [X] T059 [P] [US4] Add 20 concurrent sessions isolation test for files, env, and execution history in `crates/agent_scope_sandbox/tests/concurrency_tests.rs`

### Implementation for User Story 4

- [X] T060 [US4] Implement `SandboxSession::history` returning an ordered snapshot without exposing internal mutable state in `crates/agent_scope_sandbox/src/local.rs`
- [X] T061 [US4] Add failure category derivation from `ExecutionStatus` and `SandboxError` in `crates/agent_scope_sandbox/src/execution.rs`
- [X] T062 [US4] Ensure output refs from truncated and non-truncated executions are linked into `ExecutionRecord` in `crates/agent_scope_sandbox/src/local.rs`
- [X] T063 [US4] Implement thread-safe history sequence assignment for concurrent execute calls in `crates/agent_scope_sandbox/src/local.rs`
- [X] T064 [US4] Ensure session-scoped cleanup terminates or rejects running commands during close/reset in `crates/agent_scope_sandbox/src/local.rs`

**Checkpoint**: All user stories are independently functional; audit and concurrency behavior passes `cargo test -p agent_scope_sandbox -- sandbox_concurrent_sessions` and `cargo test -p agent_scope_sandbox -- audit`.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, compatibility evidence, validation gates, and final quality cleanup across all stories.

- [X] T065 [P] Update crate-level API documentation and examples in `crates/agent_scope_sandbox/src/lib.rs`
- [X] T066 [P] Add sandbox usage quick example or doctest in `crates/agent_scope_sandbox/src/session.rs`
- [X] T067 Update Sandbox compatibility notes and deviations in `specs/001-compatibility-baseline/capability-matrix.json`
- [X] T068 Validate quickstart scenarios and record command evidence in `specs/017-sandbox-feature/quickstart.md`
- [X] T069 Run `rtk cargo test -p agent_scope_sandbox` and fix any failures in `crates/agent_scope_sandbox/`
- [X] T070 Run `rtk cargo test -p agent_scope_workspace -- sandbox_backend` and fix any failures in `crates/agent_scope_workspace/`
- [X] T071 Run `rtk cargo check --workspace`, `rtk cargo test --workspace`, `rtk cargo clippy --workspace --all-targets -- -D warnings`, and `rtk cargo fmt --check` from repository root

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - **US1 (Phase 3)** is the MVP and should be implemented first
  - **US2 (Phase 4)** depends on US1 session/file/execute behavior for meaningful integration
  - **US3 (Phase 5)** depends on US1 core execution and file operations; some path-policy work strengthens US2
  - **US4 (Phase 6)** depends on US1 execution records and benefits from US3 status categories
- **Polish (Phase 7)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational - no dependencies on other stories
- **User Story 2 (P2)**: Starts after Foundational and should validate against US1 behavior; independently testable through `SandboxWorkspaceBackend`
- **User Story 3 (P3)**: Starts after US1 core path/execute primitives exist; independently testable through policy tests
- **User Story 4 (P4)**: Starts after US1 execution records exist; independently testable through history/audit tests

### Within Each User Story

- Tests MUST be written before implementation and should fail or fail to compile before the implementation task lands
- Public data contracts before backend behavior
- Path/mount validation before file write/delete behavior
- Command spawn before timeout/output/history completion
- Story checkpoint validation before moving to the next priority story

### Parallel Opportunities

- T004, T005, and T006 can run in parallel after T001-T003 are staged
- T008-T011 can run in parallel after T003-T004
- T019-T022 can run in parallel because they target separate test scenarios/files
- T033-T036 can run in parallel because they target workspace integration tests before adapter implementation
- T044-T048 can run in parallel across policy and isolation test cases
- T056-T059 can run in parallel across audit and concurrency test files
- Documentation tasks T065-T066 can run in parallel with compatibility update T067 once implementation is stable

---

## Parallel Example: User Story 1

```bash
# Write tests for User Story 1 in parallel:
Task: "T019 [US1] Add command execution success and non-zero exit tests in crates/agent_scope_sandbox/tests/session_tests.rs"
Task: "T021 [US1] Add sandbox read/write/delete/list file operation tests in crates/agent_scope_sandbox/tests/file_isolation_tests.rs"
Task: "T022 [US1] Add command timeout cleanup test in crates/agent_scope_sandbox/tests/policy_tests.rs"
```

## Parallel Example: User Story 2

```bash
# Write workspace backend contract tests in parallel:
Task: "T033 [US2] Add exec_shell/read_file/write_file sandbox backend integration tests in crates/agent_scope_workspace/tests/sandbox_backend_tests.rs"
Task: "T034 [US2] Add list_dir/file_exists/is_dir/delete_path/stat_mtime sandbox backend tests in crates/agent_scope_workspace/tests/sandbox_backend_tests.rs"
Task: "T035 [US2] Add workspace path traversal and symlink escape rejection tests in crates/agent_scope_workspace/tests/sandbox_backend_tests.rs"
```

## Parallel Example: User Story 3

```bash
# Write policy/security tests in parallel:
Task: "T044 [US3] Add output truncation and full output reference tests in crates/agent_scope_sandbox/tests/policy_tests.rs"
Task: "T045 [US3] Add path traversal, canonicalization, and symlink escape tests in crates/agent_scope_sandbox/tests/file_isolation_tests.rs"
Task: "T048 [US3] Add capability report tests for local sandbox features in crates/agent_scope_sandbox/tests/policy_tests.rs"
```

## Parallel Example: User Story 4

```bash
# Write audit/concurrency tests in parallel:
Task: "T056 [US4] Add ordered execution history tests in crates/agent_scope_sandbox/tests/audit_tests.rs"
Task: "T058 [US4] Add sensitive environment value redaction tests in crates/agent_scope_sandbox/tests/audit_tests.rs"
Task: "T059 [US4] Add 20 concurrent sessions isolation test in crates/agent_scope_sandbox/tests/concurrency_tests.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**:
   - `rtk cargo test -p agent_scope_sandbox -- sandbox_session_execute`
   - `rtk cargo test -p agent_scope_sandbox -- sandbox_file_isolation`
   - `rtk cargo test -p agent_scope_sandbox -- sandbox_execution_status`
5. Demo: create a sandbox session, execute `printf hello`, write/read `notes/result.txt`, close/cleanup

### Incremental Delivery

1. Setup + Foundational → crate compiles and public contracts exist
2. US1 → core sandbox execution MVP
3. US2 → workspace integration and tool-compatible backend semantics
4. US3 → policy/security hardening and honest unsupported capability reporting
5. US4 → audit history, output refs, and concurrent isolation evidence
6. Polish → compatibility matrix, documentation, full workspace validation

### Parallel Team Strategy

With multiple agents/developers:

1. One lane creates crate/setup and public contracts (Phase 1-2)
2. Once Foundational is complete:
   - Developer A: US1 core execution and file lifecycle
   - Developer B: US2 workspace backend tests and adapter mapping
   - Developer C: US3 policy/path/mount security tests and enforcement
   - Developer D: US4 audit/concurrency tests and history behavior
3. Integrate in priority order, validating each checkpoint independently

---

## Final Validation Gates

- `rtk cargo test -p agent_scope_sandbox`
- `rtk cargo test -p agent_scope_workspace -- sandbox_backend`
- `rtk cargo check --workspace`
- `rtk cargo test --workspace`
- `rtk cargo clippy --workspace --all-targets -- -D warnings`
- `rtk cargo fmt --check`

## Notes

- All tasks follow the required checklist format: `- [X] T### [P?] [US?] Description with file path`
- User story tasks include `[US1]`, `[US2]`, `[US3]`, or `[US4]`; setup, foundational, and polish tasks intentionally omit story labels
- `[P]` tasks target independent files or pre-implementation test work and should not require incomplete sibling tasks
- Non-zero command exit is a successful sandbox execution result, not a sandbox system error
- Unsupported CPU/memory/process/network hard isolation must be visible through `UnsupportedFeature` and `CapabilityReport.unsupported`
- Never silently fall back from sandbox execution to ordinary host-local execution
