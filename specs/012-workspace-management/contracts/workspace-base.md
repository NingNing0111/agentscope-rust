# Contract: WorkspaceBase Trait

**Feature**: 012-workspace-management | **Status**: Draft

## Interface

```rust
/// Abstract workspace — provides isolated working environment for Agents.
///
/// # Lifecycle
/// 1. Create → `initialize()` → Alive
/// 2. Alive → `reset()` → Alive (clean state)
/// 3. Alive → `close()` → Closed
#[async_trait::async_trait]
pub trait WorkspaceBase: Send + Sync {
    // ── lifecycle ──

    /// Provision resources, restore MCPs, seed skills.
    /// Idempotent — no-op if already alive.
    async fn initialize(&mut self) -> Result<(), WorkspaceError>;

    /// Release all resources and connections.
    async fn close(&mut self) -> Result<(), WorkspaceError>;

    /// Return workspace to an empty state.
    /// Wipes skills/, sessions/, data/, and .mcp.
    /// Does NOT re-seed default_mcps or skill_paths.
    async fn reset(&mut self) -> Result<(), WorkspaceError>;

    // ── accessors ──

    fn workspace_id(&self) -> &str;
    fn workdir(&self) -> &str;
    fn is_alive(&self) -> bool;

    // ── discovery ──

    /// Built-in tools scoped to this workspace.
    async fn list_tools(&self) -> Result<Vec<ToolInfo>, WorkspaceError>;

    /// Workspace-specific system prompt fragment.
    async fn get_instructions(&self) -> String;

    // ── MCP management ──

    /// Currently registered MCP configurations.
    async fn list_mcps(&self) -> Result<Vec<McpClientConfig>, WorkspaceError>;

    /// Register a new MCP. Persists to .mcp.
    /// Error if name already exists.
    async fn add_mcp(&mut self, mcp: McpClientConfig) -> Result<(), WorkspaceError>;

    /// Deregister an MCP by name. Warns and returns Ok if not found.
    async fn remove_mcp(&mut self, name: &str) -> Result<(), WorkspaceError>;

    // ── skill management ──

    /// Skills available to the agent.
    async fn list_skills(&self) -> Result<Vec<Skill>, WorkspaceError>;

    /// Copy a local skill directory into skills/.
    /// Error if SKILL.md missing/invalid. Duplicates skipped by hash.
    async fn add_skill(&mut self, skill_path: &str) -> Result<(), WorkspaceError>;

    /// Remove a skill by agent-facing name.
    /// Warns and returns Ok if not found.
    async fn remove_skill(&mut self, name: &str) -> Result<(), WorkspaceError>;

    // ── offload ──

    /// Append messages to context.jsonl. Extracts base64→data/.
    async fn offload_context(
        &self,
        session_id: &str,
        msgs: &[Msg],
    ) -> Result<String, WorkspaceError>;

    /// Persist a tool result to tool_result-{id}.txt.
    async fn offload_tool_result(
        &self,
        session_id: &str,
        tool_result: &ToolResultBlock,
    ) -> Result<String, WorkspaceError>;

    // ── internal ──

    /// Return the active execution backend.
    /// Error if workspace not initialized.
    fn get_backend(&self) -> Result<&dyn WorkspaceBackend, WorkspaceError>;
}
```

## Contract Guarantees

| Guarantee | Detail |
|-----------|--------|
| Thread safety | `Send + Sync` for sharing via `Arc<dyn WorkspaceBase>` |
| Idempotent init | `initialize()` called twice on alive workspace is a no-op |
| Graceful close | `close()` swallows individual MCP close failures |
| Atomic reset | `reset()` runs under locks; no half-reset state visible to callers |
| Safe remove | `remove_mcp`/`remove_skill` for unknown names log warning and return `Ok(())` |
| Path safety | All file operations validate target is within `workdir` |
| mtime consistency | `list_skills` is O(1) when skills directory mtime unchanged (uses .skills index) |
| Backend latest | `get_backend()` always returns the current backend, never a stale reference |

## Cross-reference

- Python: `WorkspaceBase` in `agentscope/src/agentscope/workspace/_base.py` (749 lines)
- See also: `contracts/workspace-backend.md`, `contracts/local-workspace.md`
