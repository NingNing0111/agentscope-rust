# Quickstart: Workspace Management

**Feature**: 012-workspace-management | **Date**: 2026-07-31

## Prerequisites

- Rust toolchain (edition 2024)
- Project workspace: `crates/agent_scope_workspace/`
- Add to root `Cargo.toml` workspace members: `crates/agent_scope_workspace` is a directory under `crates/*`

## Validation Scenarios

### Scenario 1: Create and initialize a LocalWorkspace (US1)

```shell
# Build the new crate
cargo build -p agent_scope_workspace

# Run US1 tests
cargo test -p agent_scope_workspace -- local_workspace
```

**Expected outcome**:
- `initialize()` creates `{workdir}/data/`, `{workdir}/skills/`, `{workdir}/sessions/` directories
- `{workdir}/.mcp` file created with `[]` (empty default)
- `list_tools()` returns 6 tool names: Bash, Edit, Glob, Grep, Read, Write
- `is_alive()` returns `true` after initialize, `false` after close
- `workspace_id()` returns a non-empty UUID string
- `workdir()` returns the canonicalized absolute path

### Scenario 2: MCP registration and persistence (US2)

```shell
cargo test -p agent_scope_workspace -- resource
```

**Expected outcome**:
- `add_mcp()` persists config to `.mcp` file (valid JSON array)
- `list_mcps()` returns the registered config
- `remove_mcp()` removes from memory and `.mcp` file
- Re-initialize → MCP restored from `.mcp` file
- `add_mcp()` with duplicate name → `Err(McpAlreadyExists)`
- `remove_mcp()` with unknown name → `Ok(())` (logs warning)

### Scenario 3: Skill add, list, remove with dedup (US2)

```shell
cargo test -p agent_scope_workspace -- skill
```

**Expected outcome**:
- `add_skill()` copies valid skill dir into `skills/`
- `list_skills()` returns Skill objects with name, description, dir
- `add_skill()` with same SKILL.md content → no-op (hash match)
- `add_skill()` with missing SKILL.md → `Err(InvalidSkill)`
- `remove_skill()` deletes directory and updates `.skills` index
- Agent-facing name conflict → appended " (1)" suffix

### Scenario 4: Offload large context (US3)

```shell
cargo test -p agent_scope_workspace -- offload
```

**Expected outcome**:
- 100 messages (10 with base64 image DataBlocks) → `context.jsonl` with 100 lines
- Base64 data extracted to `data/{sha256}.{ext}` files (one per unique image)
- Duplicate base64 blocks → only one file written (SHA-256 dedup)
- Message content blocks with Base64Source replaced by URLSource (file:// URI)
- `offload_context` returns correct JSONL file path

### Scenario 5: Lifecycle and reset (US4)

```shell
cargo test -p agent_scope_workspace -- lifecycle
```

**Expected outcome**:
- Add files + MCPs + skills → `reset()` → all cleared
- `list_skills()` returns `[]` after reset
- `list_mcps()` returns `[]` after reset
- `data/`, `skills/`, `sessions/` directories exist but are empty
- `.mcp` file absent or `[]`
- `close()` disconnects stateful MCPs, sets `is_alive = false`
- Double `initialize()` → second call is no-op
- `get_backend()` on uninitialized workspace → `Err(NotInitialized)`

### Scenario 6: WorkspaceManager multi-tenant (US5)

```shell
cargo test -p agent_scope_workspace -- manager
```

**Expected outcome**:
- `manager.get("user-a")` creates workspace
- `manager.get("user-a")` again returns same instance
- `manager.get("user-b")` returns different instance
- TTL expiry → workspace auto-closed and evicted
- Without TTL → workspace retained indefinitely

### Scenario 7: Full workspace build and cross-crate check

```shell
# Verify whole workspace still compiles
cargo check --workspace

# Clippy check on the new crate
cargo clippy -p agent_scope_workspace

# Format check
cargo fmt --check -p agent_scope_workspace

# Run ALL workspace tests (regression check)
cargo test --workspace
```

**Expected outcome**:
- `cargo check` — 0 errors
- `cargo clippy` — 0 warnings
- `cargo fmt` — clean
- `cargo test --workspace` — all tests pass (existing tests not broken)
