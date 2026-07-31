# Contract: LocalWorkspace

**Feature**: 012-workspace-management | **Status**: Draft

## Constructor

```rust
pub struct LocalWorkspaceConfig {
    /// Filesystem path to the workspace root (will be resolved to absolute).
    pub workdir: String,
    /// Optional pre-assigned workspace id (auto-generated UUID if None).
    pub workspace_id: Option<String>,
    /// MCP configs seeded into a brand-new workspace.
    pub default_mcps: Vec<McpClientConfig>,
    /// Local skill dirs seeded on first initialize().
    pub skill_paths: Vec<String>,
    /// Custom system prompt template (uses DEFAULT_WORKSPACE_INSTRUCTIONS if None).
    pub instructions: Option<String>,
}

impl LocalWorkspace {
    /// Create a new LocalWorkspace instance.
    /// Does NOT initialize — caller must call `initialize()`.
    pub fn new(config: LocalWorkspaceConfig) -> Self;
}
```

## Behavior Contract

### Directory Layout

```text
{workdir}/
├── .mcp          # JSON array of McpClientConfig (persisted)
├── data/         # offloaded base64 payloads (sha256.ext)
├── skills/       # skill subdirectories, each with SKILL.md
│   └── .skills   # hash index (JSON)
└── sessions/
    └── {session_id}/
        ├── context.jsonl       # offloaded messages
        └── tool_result-{id}.txt  # offloaded tool results
```

### initialize() behavior

1. Create `workdir/` if absent (including parents)
2. Create `data/`, `skills/`, `sessions/` subdirs if absent
3. Restore `.mcp` if exists and valid JSON → populate `_mcps`
4. If `.mcp` absent → use `default_mcps`, persist to `.mcp`
5. If `.mcp` corrupted → log warning, use `default_mcps`, overwrite
6. Reconnect stateful MCPs (skip stateless), log+remove failed connections
7. Seed `skill_paths` into `skills/` (skip if `skills/` already has entries)
8. Build/reconcile `.skills` index
9. Set `is_alive = true`

### close() behavior

1. For each stateful MCP in `_mcps`: attempt `close()`, log+skip failures
2. Clear `_mcps` list
3. Set `is_alive = false`
4. Directories on disk are left intact (persistence layer)

### reset() behavior

1. Close and disconnect all MCPs
2. Clear `_mcps` in memory
3. Delete `.mcp` file
4. Delete `skills/` directory recursively
5. Delete `sessions/` directory recursively
6. Delete `data/` directory recursively
7. Re-create empty `data/`, `skills/`, `sessions/` dirs
8. Do NOT re-seed `default_mcps` or `skill_paths`

### add_mcp(config) behavior

1. Acquire `_mcp_lock`
2. If name already exists → `Err(McpAlreadyExists)`
3. If stateful → attempt connect; on failure → `Err(BackendError)`
4. Append to `_mcps`
5. Persist entire `_mcps` list to `.mcp` file
6. Log success

### add_skill(skill_path) behavior

1. Acquire `_skill_lock`
2. Verify `skill_path/SKILL.md` exists and parses with name+description
3. Compute SHA-256 of SKILL.md content
4. If hash already in `.skills` index → log + return Ok (idempotent)
5. Resolve agent-facing name conflicts (append " (N)" suffix)
6. Resolve directory name conflicts (append "_N" suffix)
7. Validate dest path is within `skills/` (canonicalize + starts_with check)
8. Copy directory tree to `skills/{sanitized_dir_name}`
9. Update `.skills` index and persist
10. Release lock

### offload_context(session_id, msgs) behavior

1. Create `sessions/{session_id}/` if absent
2. Deep-clone messages (do not mutate input)
3. For each message: iterate content blocks
   - If `DataBlock` with `Base64Source`:
     - Compute SHA-256 of base64 string
     - Guess extension from `media_type` (default `.bin`)
     - If `data/{hash}{ext}` absent → decode+write
     - Replace `Base64Source` with `URLSource(file://{abs_path})`
4. Serialize each message as JSON, join with `\n`, append a trailing `\n`
5. Read existing file content (if any)
6. Write `existing + new_lines` to `sessions/{session_id}/context.jsonl`
7. Return the JSONL file path

### list_tools() behavior

1. Check `is_alive` → error if not
2. Return `ToolInfo` entries for: Bash(cwd=workdir), Edit, Glob, Grep, Read, Write
3. Each tool is bound to `self._backend`
4. On Windows: replace Bash with PowerShell

## Compatibility

| Item | Python Source | Rust Equivalent |
|------|--------------|-----------------|
| `.mcp` format | JSON array of MCPClient model_dump | Same JSON structure |
| `.skills` format | `_SkillsFile` TypedDict | Same JSON structure |
| offload hash | `hashlib.sha256(block.source.data.encode())` | SHA-256 of base64 string |
| context.jsonl | One JSON per line | Same |
| tool_result filename | `tool_result-{id}.txt` | Same |
