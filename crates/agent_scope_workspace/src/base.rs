//! WorkspaceBase trait — the core workspace abstraction.

use crate::error::WorkspaceError;
use crate::mcp::McpClientConfig;
use crate::skill::Skill;

/// Lightweight tool metadata returned by `list_tools()`.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// Unique tool name (e.g. "Bash", "Read", "Write").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
}

/// Abstract workspace — provides an isolated working environment for Agents.
///
/// # Lifecycle
/// 1. Create → `initialize()` → Alive
/// 2. Alive → `reset()` → Alive (clean state)
/// 3. Alive → `close()` → Closed
#[async_trait::async_trait]
#[allow(unused)]
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
    async fn add_mcp(&mut self, mcp: McpClientConfig) -> Result<(), WorkspaceError>;

    /// Deregister an MCP by name.
    async fn remove_mcp(&mut self, name: &str) -> Result<(), WorkspaceError>;

    // ── skill management ──

    /// Skills available to the agent.
    async fn list_skills(&self) -> Result<Vec<Skill>, WorkspaceError>;

    /// Copy a local skill directory into skills/.
    async fn add_skill(&mut self, skill_path: &str) -> Result<(), WorkspaceError>;

    /// Remove a skill by agent-facing name.
    async fn remove_skill(&mut self, name: &str) -> Result<(), WorkspaceError>;

    // ── offload ──

    /// Append messages to context.jsonl. Extracts base64→data/.
    async fn offload_context(
        &self,
        session_id: &str,
        msgs: &[agent_scope_message::Msg],
    ) -> Result<String, WorkspaceError>;

    /// Persist a tool result to tool_result-{id}.txt.
    async fn offload_tool_result(
        &self,
        session_id: &str,
        tool_result: &agent_scope_message::ToolResultBlock,
    ) -> Result<String, WorkspaceError>;

    // ── internal ──

    /// Return the active execution backend.
    fn get_backend(&self) -> Result<&dyn crate::backend::WorkspaceBackend, WorkspaceError>;
}
