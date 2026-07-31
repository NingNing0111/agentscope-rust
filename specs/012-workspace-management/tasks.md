# Tasks: Workspace Management（工作空间管理）

**Input**: Design documents from `specs/012-workspace-management/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Tests are included — spec SC-007 requires "100% 的公开 API 有对应的测试覆盖". quickstart.md defines 7 validation scenarios.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create new crate scaffolding and configure workspace dependencies

- [x] T001 Create `crates/agent_scope_workspace/` directory structure with `Cargo.toml`, `src/lib.rs`, `tests/` directory per plan.md project structure
- [x] T002 [P] Configure `agent_scope_workspace/Cargo.toml` dependencies: `agent_scope_message` (path), `tokio` (fs, sync, process, time), `serde` + `serde_json`, `sha2`, `uuid`, `base64`, `mime_guess`, `tracing`, `async-trait`, `tempfile` (dev), `chrono` (optional)
- [x] T003 [P] Add `#![deny(unsafe_code)]` and `#![deny(clippy::unwrap_used)]` to `crates/agent_scope_workspace/src/lib.rs` per Constitution §IX
- [x] T004 Verify workspace builds with `cargo check --workspace` (new crate compiles empty)

**Checkpoint**: Workspace structure ready — new crate compiles, no dependency errors

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core error types, Backend abstraction, and test utilities that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Error types

- [x] T005 [P] Define `WorkspaceError` enum with all variants (`BackendError`, `NotInitialized`, `AlreadyInitialized`, `InvalidSkill`, `SkillNotFound`, `McpNotFound`, `McpAlreadyExists`, `PathTraversal`, `CorruptMcpFile`, `GatewayError`, `OffloadError`) in `crates/agent_scope_workspace/src/error.rs` per data-model.md Entity 11
- [x] T006 [P] Implement `std::fmt::Display` and `std::error::Error` for `WorkspaceError` in `crates/agent_scope_workspace/src/error.rs` per Constitution §XIII

### Backend trait + LocalBackend

- [x] T007 Define `ExecOutput` struct (`stdout: Vec<u8>`, `stderr: Vec<u8>`, `exit_code: i32`, `fn ok()`) in `crates/agent_scope_workspace/src/backend.rs` per data-model.md Entity 2 and contracts/workspace-backend.md
- [x] T008 Define `WorkspaceBackend` trait with all methods (`exec_shell`, `read_file`, `write_file`, `is_dir`, `list_dir`, `delete_path`, `file_exists`, `join_path`, `basename`, `dirname`, `stat_mtime`, `normpath`, `is_absolute`) in `crates/agent_scope_workspace/src/backend.rs` per contracts/workspace-backend.md
- [x] T009 Implement `LocalBackend` struct with all `WorkspaceBackend` trait methods using `tokio::fs` and `tokio::process` in `crates/agent_scope_workspace/src/backend.rs` per data-model.md Entity 1
- [x] T010 [P] Write unit tests for `LocalBackend` (file write/read cycle, exec_shell echo, is_dir, list_dir recursive, delete_path idempotent, stat_mtime, normpath) in `crates/agent_scope_workspace/tests/backend_tests.rs`

### Test utilities

- [x] T011 [P] Create test helper module with `fn temp_workdir() -> TempDir` that creates a temp directory with a unique workspace path in `crates/agent_scope_workspace/tests/common/mod.rs`

### Module declarations

- [x] T012 Setup module declarations in `crates/agent_scope_workspace/src/lib.rs`: `pub mod error; pub mod backend; pub mod base; pub mod local_workspace; pub mod skill; pub mod mcp; pub mod offload; pub mod manager; pub mod instructions;`

**Checkpoint**: Backend trait + LocalBackend + Error types ready — user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - 创建和管理本地工作空间 (Priority: P1) 🎯 MVP

**Goal**: 开发者可创建 `LocalWorkspace` 实例，初始化工作目录结构，获取绑定到工作空间的工具列表

**Independent Test**: 创建 `LocalWorkspace` 并初始化，验证目录结构自动创建，`list_tools()` 返回绑定到 workdir 的内置工具，文件读写限定在工作空间范围内

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T013 [P] [US1] Write `LocalWorkspace` constructor + initialization tests (workdir created, subdirs created, .mcp seeded, is_alive true, double init is no-op) in `crates/agent_scope_workspace/tests/local_workspace_tests.rs` per quickstart.md Scenario 1
- [x] T014 [P] [US1] Write `list_tools()` test (returns 6 tools: Bash/Edit/Glob/Grep/Read/Write, Bash cwd=workdir) in `crates/agent_scope_workspace/tests/local_workspace_tests.rs`
- [x] T015 [P] [US1] Write file I/O scoping test (write/read via backend stays within workdir) in `crates/agent_scope_workspace/tests/local_workspace_tests.rs`

### Implementation for User Story 1 — WorkspaceBase trait

- [x] T016 [US1] Define `WorkspaceBase` async trait with all methods per contracts/workspace-base.md in `crates/agent_scope_workspace/src/base.rs` per data-model.md
- [x] T017 [US1] Define `ToolInfo` struct (name, description, input_schema as JsonValue) in `crates/agent_scope_workspace/src/base.rs` — lightweight tool metadata returned by `list_tools()`

### Implementation for User Story 1 — LocalWorkspace core

- [x] T018 [US1] Define `LocalWorkspaceConfig` struct (workdir, workspace_id, default_mcps, skill_paths, instructions) in `crates/agent_scope_workspace/src/local_workspace.rs`
- [x] T019 [US1] Implement `LocalWorkspace::new(config: LocalWorkspaceConfig) -> Self` — resolve workdir to absolute path, initialize internal state in `crates/agent_scope_workspace/src/local_workspace.rs` (depends on T016, T018)
- [x] T020 [US1] Implement `WorkspaceBase::initialize()` for `LocalWorkspace` — create workdir/data/skills/sessions dirs, seed .mcp with default_mcps, set is_alive=true, idempotent on re-call in `crates/agent_scope_workspace/src/local_workspace.rs` per contracts/local-workspace.md
- [x] T021 [US1] Implement `WorkspaceBase::list_tools()` for `LocalWorkspace` — return ToolInfo for Bash/Edit/Glob/Grep/Read/Write bound to workdir, on Windows use PowerShell instead of Bash in `crates/agent_scope_workspace/src/local_workspace.rs`
- [x] T022 [US1] Define `DEFAULT_WORKSPACE_INSTRUCTIONS` template constant in `crates/agent_scope_workspace/src/instructions.rs` per spec FR-032/FR-033 (translate Python `DEFAULT_WORKSPACE_INSTRUCTIONS` from `_utils.py` to Rust)
- [x] T023 [US1] Implement `WorkspaceBase::get_instructions()` for `LocalWorkspace` — format template with `{workdir}` and `{backend}` placeholders in `crates/agent_scope_workspace/src/local_workspace.rs`
- [x] T024 [US1] Implement `WorkspaceBase::get_backend()` for `LocalWorkspace` — return `&dyn WorkspaceBackend`, error with `NotInitialized` if not alive in `crates/agent_scope_workspace/src/local_workspace.rs`

**Checkpoint**: US1 complete — LocalWorkspace can be created, initialized, and provides tools + instructions

---

## Phase 4: User Story 2 - 工作空间内的资源管理 (Priority: P2)

**Goal**: 开发者可在工作空间内动态管理 MCP 客户端配置和 Skill（技能模块），包括增删查操作

**Independent Test**: 在已初始化的 `LocalWorkspace` 中执行 MCP 和 Skill 的完整 CRUD 操作，验证持久化和去重

### Tests for User Story 2

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T025 [P] [US2] Write MCP management tests (add/list/remove, persist to .mcp, restore on re-init, duplicate name error, unknown name warn, corrupt .mcp fallback) in `crates/agent_scope_workspace/tests/resource_tests.rs` per quickstart.md Scenario 2
- [x] T026 [P] [US2] Write Skill management tests (add valid skill, list skills, remove by name, add duplicate hash skip, add missing SKILL.md error, name conflict suffix, remove unknown warn) in `crates/agent_scope_workspace/tests/resource_tests.rs` per quickstart.md Scenario 3

### Implementation for User Story 2 — MCP Client Config

- [x] T027 [P] [US2] Define `McpTransportConfig` enum (`Stdio`, `Sse`, `StreamableHttp` variants) with `Serialize`/`Deserialize` in `crates/agent_scope_workspace/src/mcp.rs` per data-model.md Entity 4
- [x] T028 [P] [US2] Define `McpClientConfig` struct (name, transport, is_stateful) with `Serialize`/`Deserialize` in `crates/agent_scope_workspace/src/mcp.rs` per data-model.md Entity 3
- [x] T029 [US2] Implement `McpRegistry` — in-memory `Vec<McpClientConfig>` + `_save_mcp_file(backend, path)` / `_load_mcp_file(backend, path)` with corrupt-file fallback in `crates/agent_scope_workspace/src/mcp.rs`

### Implementation for User Story 2 — MCP trait methods on LocalWorkspace

- [x] T030 [US2] Implement `WorkspaceBase::list_mcps()` for `LocalWorkspace` — return clone of current MCP configs in `crates/agent_scope_workspace/src/local_workspace.rs` (depends on T029)
- [x] T031 [US2] Implement `WorkspaceBase::add_mcp()` for `LocalWorkspace` — validate name uniqueness, append, persist via McpRegistry under _mcp_lock in `crates/agent_scope_workspace/src/local_workspace.rs`
- [x] T032 [US2] Implement `WorkspaceBase::remove_mcp()` for `LocalWorkspace` — remove by name (warn on unknown), persist via McpRegistry under _mcp_lock in `crates/agent_scope_workspace/src/local_workspace.rs`

### Implementation for User Story 2 — Skill struct + SkillManager

- [x] T033 [P] [US2] Define `Skill` struct (name, description, dir, markdown, updated_at) with `Serialize`/`Deserialize` in `crates/agent_scope_workspace/src/skill.rs` per data-model.md Entity 5
- [x] T034 [P] [US2] Define `SkillEntry` struct (hash, skill_name) and `SkillsIndex` struct (skills_dir_mtime, skills: HashMap<String, SkillEntry>) in `crates/agent_scope_workspace/src/skill.rs` per data-model.md Entities 7-8
- [x] T035 [US2] Implement `SkillManager` — `load_index()`, `save_index()`, `reconcile()` (detect mtime change, add/remove entries), `validate_skill(path)` (check SKILL.md exists with name+description), `hash_skill(path)` (SHA-256 of SKILL.md content) in `crates/agent_scope_workspace/src/skill.rs` per data-model.md Entity 6
- [x] T036 [US2] Implement `SkillManager::add_skill()` — validate, hash-dedup, resolve name/dir conflicts (append " (N)" / "_N" suffix), copy directory tree, path-traversal guard (canonicalize + starts_with), update index in `crates/agent_scope_workspace/src/skill.rs`
- [x] T037 [US2] Implement `SkillManager::remove_skill()` — lookup by agent-facing name in index, delete dir, update index in `crates/agent_scope_workspace/src/skill.rs`
- [x] T038 [US2] Implement `SkillManager::list_skills()` — load index, reconcile if mtime changed, parse SKILL.md for each entry, return `Vec<Skill>` in `crates/agent_scope_workspace/src/skill.rs`

### Implementation for User Story 2 — Skill trait methods on LocalWorkspace

- [x] T039 [US2] Implement `WorkspaceBase::list_skills()` for `LocalWorkspace` — delegate to `SkillManager::list_skills()` under _skill_lock in `crates/agent_scope_workspace/src/local_workspace.rs`
- [x] T040 [US2] Implement `WorkspaceBase::add_skill()` for `LocalWorkspace` — delegate to `SkillManager::add_skill()` under _skill_lock, resolve path to absolute in `crates/agent_scope_workspace/src/local_workspace.rs`
- [x] T041 [US2] Implement `WorkspaceBase::remove_skill()` for `LocalWorkspace` — delegate to `SkillManager::remove_skill()` under _skill_lock in `crates/agent_scope_workspace/src/local_workspace.rs`

**Checkpoint**: US2 complete — MCP configs and Skills can be fully managed with persistence and dedup

---

## Phase 5: User Story 3 - 大内容上下文卸载 (Priority: P3)

**Goal**: 开发者可将包含大量数据的对话上下文和工具结果 offload 到工作空间持久化存储

**Independent Test**: 创建包含 base64 大数据的 Msg 对象，调用 `offload_context()` 验证 base64 提取为 `file://` URL，`offload_tool_result()` 验证持久化

### Tests for User Story 3

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T042 [P] [US3] Write `offload_context` tests (base64 extraction to data/, JSONL append, duplicate base64 skip, file:// URL replacement) in `crates/agent_scope_workspace/tests/offload_tests.rs` per quickstart.md Scenario 4
- [x] T043 [P] [US3] Write `offload_tool_result` tests (text+data block output, filename conflict suffix, session isolation) in `crates/agent_scope_workspace/tests/offload_tests.rs`

### Implementation for User Story 3

- [x] T044 [US3] Implement `_offload_data_block(block: &DataBlock, backend: &dyn WorkspaceBackend, data_dir: &str) -> Result<DataBlock>` — Base64Source → SHA-256 hash → decode → write to `data/{hash}.{ext}` → return DataBlock with URLSource(`file://` URI) in `crates/agent_scope_workspace/src/offload.rs` per spec FR-022/FR-023
- [x] T045 [US3] Implement `offload_context(session_id, msgs, backend, sessions_dir, data_dir) -> Result<String>` — deep-clone msgs, offload base64 blocks, serialize as JSONL, append to `sessions/{session_id}/context.jsonl`, return path in `crates/agent_scope_workspace/src/offload.rs` per spec FR-021/FR-022
- [x] T046 [US3] Implement `offload_tool_result(session_id, tool_result, backend, sessions_dir, data_dir) -> Result<String>` — extract text blocks + data block placeholders, handle filename collision (append `(N)` suffix), write to `sessions/{session_id}/tool_result-{id}.txt` in `crates/agent_scope_workspace/src/offload.rs` per spec FR-024/FR-025
- [x] T047 [US3] Implement `WorkspaceBase::offload_context()` for `LocalWorkspace` — ensure sessions dir exists, delegate to `offload_context()` free function in `crates/agent_scope_workspace/src/local_workspace.rs`
- [x] T048 [US3] Implement `WorkspaceBase::offload_tool_result()` for `LocalWorkspace` — ensure sessions dir exists, delegate to `offload_tool_result()` free function in `crates/agent_scope_workspace/src/local_workspace.rs`

**Checkpoint**: US3 complete — large context and tool results can be offloaded to persistent storage

---

## Phase 6: User Story 4 - 工作空间生命周期与重置 (Priority: P4)

**Goal**: 开发者可完整管理 workspace 生命周期（close）和重置（reset），确保资源正确释放和清理

**Independent Test**: 在同一 `LocalWorkspace` 中执行 initialize → 操作 → reset → 验证所有内容清空；close → 验证 is_alive=false

### Tests for User Story 4

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T049 [P] [US4] Write `reset()` tests (clears skills/mcps/sessions/data, dirs exist but empty, .mcp cleared, no re-seed of defaults) in `crates/agent_scope_workspace/tests/lifecycle_tests.rs` per quickstart.md Scenario 5
- [x] T050 [P] [US4] Write `close()` tests (is_alive becomes false, stateful MCPs disconnected, re-initialize restores from .mcp) in `crates/agent_scope_workspace/tests/lifecycle_tests.rs`
- [x] T051 [P] [US4] Write edge case tests (get_backend on uninitialized → error, initialize on alive → no-op idempotent, close on closed → idempotent) in `crates/agent_scope_workspace/tests/lifecycle_tests.rs`

### Implementation for User Story 4

- [x] T052 [US4] Implement `WorkspaceBase::close()` for `LocalWorkspace` — close stateful MCPs (log+skip failures), clear _mcps, set is_alive=false, idempotent in `crates/agent_scope_workspace/src/local_workspace.rs` per spec FR-028
- [x] T053 [US4] Implement `WorkspaceBase::reset()` for `LocalWorkspace` — close all MCPs, clear _mcps, delete .mcp file, delete skills/, sessions/, data/ dirs, re-create empty dirs, do NOT re-seed defaults in `crates/agent_scope_workspace/src/local_workspace.rs` per spec FR-026/FR-027

**Checkpoint**: US4 complete — workspace lifecycle (init/close/reset/re-init) fully functional

---

## Phase 7: User Story 5 - 工作空间管理器（多租户） (Priority: P5)

**Goal**: 平台开发者可通过 `WorkspaceManager` 管理多个 workspace 实例的生命周期，支持按 key 隔离和 TTL 淘汰

**Independent Test**: 创建 `WorkspaceManager`，通过 `get(key)` 获取/创建工作空间，验证同一 key 返回同一实例，TTL 超时后自动清理

### Tests for User Story 5

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T054 [P] [US5] Write `WorkspaceManager` tests (get creates workspace, get returns same instance for same key, different keys get different instances, TTL eviction, no TTL = never evict) in `crates/agent_scope_workspace/tests/manager_tests.rs` per quickstart.md Scenario 6

### Implementation for User Story 5

- [x] T055 [US5] Define `ManagerEntry` struct (workspace: `Arc<dyn WorkspaceBase>`, last_access: `Instant`) in `crates/agent_scope_workspace/src/manager.rs` per data-model.md Entity 10
- [x] T056 [US5] Implement `WorkspaceManager` struct with fields: entries (`Arc<RwLock<HashMap<String, ManagerEntry>>>`), ttl (`Option<Duration>`), cleanup_handle (`Option<JoinHandle<()>>`) in `crates/agent_scope_workspace/src/manager.rs` per data-model.md Entity 9 (depends on T016)
- [x] T057 [US5] Implement `WorkspaceManager::new(ttl: Option<Duration>) -> Self` — initialize empty entries map, optionally spawn background cleanup task (runs every ttl/2, evicts expired entries) in `crates/agent_scope_workspace/src/manager.rs`
- [x] T058 [US5] Implement `WorkspaceManager::get(key: &str) -> Result<Arc<dyn WorkspaceBase>>` — return existing entry if present (update last_access), otherwise call `create_fn` to build+init new workspace, insert and return in `crates/agent_scope_workspace/src/manager.rs`
- [x] T059 [US5] Implement `Drop` for `WorkspaceManager` — abort cleanup task if running, close all remaining workspaces in `crates/agent_scope_workspace/src/manager.rs`

**Checkpoint**: US5 complete — multi-tenant workspace management with TTL eviction

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Integration validation, documentation, and cross-crate quality checks

- [x] T060 [P] Add `tracing` instrument macros to all public methods (initialize, close, reset, list_tools, add_mcp, remove_mcp, add_skill, remove_skill, offload_context, offload_tool_result, WorkspaceManager::get) per Constitution §VI
- [x] T061 [P] Add crate-level doc comment (`//!`) to `crates/agent_scope_workspace/src/lib.rs` with module overview and usage example
- [x] T062 Run `cargo fmt` on the new crate
- [x] T063 Run `cargo clippy -- -D warnings` on the new crate and fix all warnings
- [x] T064 Run all workspace tests with `cargo test --workspace` to verify no regressions
- [x] T065 Run quickstart.md Scenario 7 validation: `cargo check --workspace`, `cargo clippy -p agent_scope_workspace`, `cargo fmt --check -p agent_scope_workspace`, `cargo test --workspace`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup (T001) for crate structure — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational — No dependencies on other stories
- **User Story 2 (Phase 4)**: Depends on Foundational + US1 (T016 WorkspaceBase trait, T019 LocalWorkspace::new) — MCP/Skill methods are additional trait methods on LocalWorkspace
- **User Story 3 (Phase 5)**: Depends on Foundational + US1 (needs initialized workspace + backend)
- **User Story 4 (Phase 6)**: Depends on Foundational + US1 (needs initialized workspace) + US2 (MCP close/reset uses McpRegistry)
- **User Story 5 (Phase 7)**: Depends on US1 (needs WorkspaceBase trait as type parameter) — Can start after US1, independent of US2-US4
- **Polish (Phase 8)**: Depends on all user stories being complete

### User Story Dependencies

| Story | Depends On | Can Parallel With |
|-------|-----------|-------------------|
| US1 (P1) | Foundational | — |
| US2 (P2) | US1 (WorkspaceBase trait + LocalWorkspace) | US3, US5 |
| US3 (P3) | US1 (initialized workspace + backend) | US2, US5 |
| US4 (P4) | US1 + US2 (MCP close/reset) | US5 |
| US5 (P5) | US1 (WorkspaceBase trait) | US2, US3, US4 |

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Data types/models before service logic
- Service logic before trait method delegation
- Trait methods before integration tests pass

### Parallel Opportunities

- **Phase 1**: T002, T003 all [P] — can run in parallel
- **Phase 2**: T005, T006, T010, T011 all [P]
- **US1 tests**: T013, T014, T015 all [P]
- **US2 tests**: T025, T026 all [P]
- **US2 impl**: T027, T028, T033, T034 all [P] — MCP types and Skill types are independent
- **US3 tests**: T042, T043 all [P]
- **US4 tests**: T049, T050, T051 all [P]
- **US5 tests**: T054 [P]
- **Polish**: T060, T061 all [P]
- **Cross-story**: US2, US3, US5 can be implemented in parallel after US1 (if multiple developers)

---

## Parallel Example: User Story 2

```bash
# Step 1: Launch all tests together (ensure they FAIL):
Task: "Write MCP management tests in crates/agent_scope_workspace/tests/resource_tests.rs"
Task: "Write Skill management tests in crates/agent_scope_workspace/tests/resource_tests.rs"

# Step 2: Launch MCP types and Skill types in parallel:
Task: "Define McpTransportConfig enum in crates/agent_scope_workspace/src/mcp.rs"
Task: "Define McpClientConfig struct in crates/agent_scope_workspace/src/mcp.rs"
Task: "Define Skill struct in crates/agent_scope_workspace/src/skill.rs"
Task: "Define SkillEntry and SkillsIndex structs in crates/agent_scope_workspace/src/skill.rs"

# Step 3: Implement services (sequential within each domain, parallel across):
Task: "Implement McpRegistry in crates/agent_scope_workspace/src/mcp.rs"
Task: "Implement SkillManager in crates/agent_scope_workspace/src/skill.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T004)
2. Complete Phase 2: Foundational (T005-T012)
3. Complete Phase 3: User Story 1 (T013-T024)
4. **STOP and VALIDATE**: `cargo test -p agent_scope_workspace -- local_workspace`
5. Can already create, initialize a workspace, get tools and instructions

### Incremental Delivery

1. Setup + Foundational → Backend trait, error types, LocalBackend ready
2. Add User Story 1 → Test independently → Workspace creation MVP!
3. Add User Story 2 → Test independently → MCP + Skill management
4. Add User Story 3 → Test independently → Context offloading
5. Add User Story 4 → Test independently → Lifecycle + reset
6. Add User Story 5 → Test independently → Multi-tenant manager
7. Polish → Production ready

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together (T001-T012)
2. Developer A: User Story 1 (T013-T024) ← critical path
3. Once US1 complete:
   - Developer A: User Story 2 (T025-T041)
   - Developer B: User Story 3 (T042-T048)
   - Developer C: User Story 5 (T054-T059, can start after US1 trait defined)
4. Developer D: User Story 4 (T049-T053) after US2 complete
5. Everyone: Phase 8 Polish together

---

## Notes

- [P] tasks = different files, no dependencies — can run in parallel
- [Story] label maps task to specific user story for traceability
- Each user story is independently testable via `cargo test -- <test_file_filter>`
- Tests written first per TDD: verify they FAIL, then implement
- Commit after each task or logical group
- Stop at any Checkpoint to validate story independently
- All paths are project-relative from repository root
- crate is auto-included via workspace `crates/*` glob — no need to edit root Cargo.toml
