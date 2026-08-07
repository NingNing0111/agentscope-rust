# Workspace / Workspace

> One-liner: `agent_scope_workspace` provides each Agent with an isolated filesystem sandbox — through the `WorkspaceBase` trait abstracting file I/O, Bash execution, MCP client configuration, skill management, and context offloading, with `LocalWorkspace` providing a ready-to-use local implementation.

## 1. Module Overview (Overview)

| Component | Responsibility |
|-----------|---------------|
| `WorkspaceBase` | Workspace lifecycle abstraction: `initialize()`, `close()`, `reset()` |
| `LocalWorkspace` | Local filesystem implementation providing Read/Write/Edit/Glob/Grep/Bash built-in tools |
| `WorkspaceBackend` | File backend trait: `read_file`, `write_file`, `delete_file`, `glob`, `grep`, etc. |
| `WorkspaceManager` | Multi-workspace manager mapping sessions to workspaces |
| `McpClientConfig` / `McpRegistry` | MCP client configuration (Stdio, SSE transport), registration and discovery |
| `SkillManager` / `Skill` / `SkillsIndex` | Skill file management, loading, and indexing |
| Offload | Context offloading — moving large file content into the workspace to reduce context pressure |

**When to use**: Agent needs to read/write files; running infrastructure tools (Git, Docker); integrating MCP services; loading Skill files.

**Prerequisites**: [Agent System](./agent.md), [Tool System](./tool.md), [Skill System](./skill.md), [Sandbox](./sandbox.md)

## 2. Core Concepts & Main Public Types (Core Concepts)

### 2.1 `WorkspaceBase` trait

```rust
#[async_trait]
pub trait WorkspaceBase: Send + Sync {
    async fn initialize(&mut self) -> Result<(), WorkspaceError>;
    async fn close(&mut self) -> Result<(), WorkspaceError>;
    async fn reset(&mut self) -> Result<(), WorkspaceError>;

    fn workspace_id(&self) -> &str;
    fn workdir(&self) -> &str;
    fn is_alive(&self) -> bool;

    async fn list_tools(&self) -> Result<Vec<ToolInfo>, WorkspaceError>;
    // ... file operations, Bash execution, MCP/Skill management methods
}
```

Lifecycle: `Create → initialize() → Alive → close() → Closed`, with `reset()` available at any point.

### 2.2 `LocalWorkspace`

`LocalWorkspace::new(config)` configuration:

| Field | Description |
|-------|-------------|
| `workdir` | Workspace root directory |
| `workspace_id` | Optional ID, auto-generated if not provided |
| `default_mcps` | MCP clients auto-registered on initialization |
| `skill_paths` | Skill file paths loaded on initialization |
| `instructions` | Optional Agent instruction text |

### 2.3 Built-in Tools

Tools returned by `LocalWorkspace.list_tools()`:

| Tool | Function |
|------|----------|
| Bash | Execute Shell commands (with timeout and output limits) |
| Read | Read file contents |
| Write | Write to files |
| Edit | Exact string replacement in files |
| Glob | File pattern matching |
| Grep | Content search |

### 2.4 MCP Integration

```rust
pub struct McpClientConfig {
    pub name: String,
    pub transport: McpTransportConfig,
    pub is_stateful: bool,
}
pub enum McpTransportConfig {
    Stdio { command: String, args: Vec<String> },
    Sse { url: String, headers: HashMap<String, String> },
    StreamableHttp { url: String, headers: HashMap<String, String> },
}
```

`McpRegistry` is a **config-only** registry (load/save/list); the runtime connection, tool discovery, and tool-call adapters live in the `agent_scope_mcp` crate (`McpClient` / `McpTool` / `McpExt`). See [MCP Integration](./mcp.md).

### 2.5 Skill Management

A `Skill` represents a skill file:
- Parsed from `.md` files with frontmatter (name, description)
- Body is Markdown instructions
- `SkillManager` maintains a `SkillsIndex`, supporting load, list, and search

### 2.6 Context Offloading

Moves large text blocks to workspace files, keeping only path references in context to reduce token pressure.

## 3. Quick Example (Quick Example)

```rust
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

let config = LocalWorkspaceConfig {
    workdir: "/tmp/my-workspace".into(),
    workspace_id: None,
    default_mcps: vec![],
    skill_paths: vec![],
    instructions: None,
};

let mut ws = LocalWorkspace::new(config);
ws.initialize().await?;

let tools = ws.list_tools().await?;
// tools include Bash, Read, Write, Edit, Glob, Grep

ws.close().await?;
```

## 4. Key Usage Patterns (Usage Patterns)

### 4.1 Using Workspace with Agents

```rust
use std::sync::Arc;
let ws = Arc::new(tokio::sync::Mutex::new(LocalWorkspace::new(config)));
ws.lock().await.initialize().await?;

let agent = ReActAgent::new(
    agent_config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![], // inject workspace-aware middleware
)?;
```

### 4.2 MCP Client Registration

```rust
let mcp = McpClientConfig {
    name: "my-server".into(),
    transport: McpTransportConfig::Stdio {
        command: "node".into(),
        args: vec!["server.js".into()],
    },
    is_stateful: true,
};
let config = LocalWorkspaceConfig {
    default_mcps: vec![mcp],
    ..Default::default()
};
```

### 4.3 Bash Execution

The workspace Bash tool includes safety controls:
- Command timeout
- Output size limit
- Working directory constrained to workspace scope

### 4.4 Multi-Workspace Management

```rust
use agent_scope_workspace::WorkspaceManager;
let mut manager = WorkspaceManager::new();
let ws_id = manager.create_workspace(config).await?;
let ws = manager.get_workspace(&ws_id)?;
```

## 5. Errors & Unsupported Capabilities (Errors & Unsupported)

| Error | Cause |
|-------|-------|
| `WorkspaceError::IoError` | File operation failure |
| `WorkspaceError::InvalidSkill` | Invalid skill file format |
| `WorkspaceError::GatewayError` | Sandbox gateway error (placeholder, pending integration) |
| `WorkspaceError::AlreadyClosed` | Operating on a closed workspace |
| `WorkspaceError::McpNotFound` | No persisted MCP config with that name |
| `WorkspaceError::McpConnectionError` | MCP transport-level failure (spawn/connect/disconnect) |
| `WorkspaceError::McpCallError` | Protocol/peer error during an MCP call |

**Unsupported**:
- `GatewayError` is currently a placeholder; sandbox↔workspace gateway integration is pending.
- Remote workspace backends are out of scope.
- Network isolation within workspace is provided by `agent_scope_sandbox`, not the workspace layer.

## 6. Compatibility (Compatibility)

- **Compatibility Level**: **L2** (core workspace behavior)
- **Authority**: `specs/012-workspace-management/spec.md`
- **Known Deviations**: `McpRegistry` and `SkillManager` are Rust-side abstractions not directly corresponding to Python-side equivalents

## 7. See Also (Related Modules)

- [Sandbox](./sandbox.md) — Execution isolation layer on top of workspace
- [Skill System](./skill.md) — Skill management within workspace
- [Tool System](./tool.md) — Workspace built-in tools exposed through Tool interface
- [Agent System](./agent.md) — How Agents consume workspaces
