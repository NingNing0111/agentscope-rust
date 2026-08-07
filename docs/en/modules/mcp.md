# MCP Integration / MCP Integration

> One-liner: `agent_scope_mcp` turns external **Model Context Protocol (MCP)** servers into unified `agent_scope_tool::Tool` adapters — an Agent connects to a registered MCP server (stdio subprocess or streamable-http), discovers its remote tools, and calls them through the same tool contract as local tools. Built on the official Rust MCP SDK `rmcp`.

## 1. Module Overview (Overview)

| Component | Responsibility |
|-----------|---------------|
| `McpClient` | Runtime MCP client wrapping an `rmcp` connection: connect/disconnect, tool discovery, tool calls |
| `McpTool` | Adapter exposing a remote MCP tool as an `agent_scope_tool::Tool` |
| `McpExt` | Extension trait adding `connect_mcp` / `disconnect_mcp` / `get_mcp_tools` to a workspace |
| `McpClientConfig` / `McpTransportConfig` | **Persisted** configuration (Stdio / SSE / StreamableHttp), owned by `agent_scope_workspace` |
| `.mcp` file | JSON array persistence for registered MCP configs, stored under the workspace root |

**When to use**: Agent needs tools that live in an external service (Excalidraw canvas, web search, file editors, etc.) rather than a local implementation.

**Prerequisites**: [Workspace](./workspace.md) (owns persisted MCP config), [Tool System](./tool.md) (the unified tool contract), [Agent System](./agent.md).

**Architecture note**: this crate sits *above* `agent_scope_workspace` and `agent_scope_tool` to break a crate dependency cycle — the workspace crate owns persisted MCP *configuration*, while `agent_scope_mcp` owns the *runtime connection* and the *tool adapter*.

## 2. Core Concepts & Main Public Types (Core Concepts)

### 2.1 Transports (`McpTransportConfig`)

```rust
#[serde(tag = "type")]
pub enum McpTransportConfig {
    Stdio { command: String, args: Vec<String> },
    Sse { url: String, headers: HashMap<String, String> },
    StreamableHttp { url: String, headers: HashMap<String, String> },
}
```

| Variant | Wire format | Notes |
|---------|-------------|-------|
| `Stdio` | Spawns a subprocess, talks over stdin/stdout | The most common for Node/Python MCP servers (e.g. `npx -y mcp-excalidraw-server`) |
| `StreamableHttp` | HTTP streaming (the modern MCP spec transport) | Native support |
| `Sse` | Legacy transport | Mapped to `StreamableHttp` at connect time with an `info!` notice (FR-002); kept for backward compatibility with older `.mcp` files |

### 2.2 `McpClient`

A live MCP connection. Created from a persisted [`McpClientConfig`](./workspace.md), it holds the `rmcp` `RunningService` for the duration of the connection.

```rust
pub struct McpClient {
    name: String,
    config: McpClientConfig,
    service: tokio::sync::Mutex<Option<RunningService<RoleClient, ClientInfo>>>,
    tools_cache: std::sync::Mutex<Vec<Tool>>,
}
```

Key methods:

| Method | Behavior |
|--------|----------|
| `new(config)` | Create from a persisted config (not yet connected) |
| `connect()` | Build the transport for the config variant, establish the connection, discover tools (idempotent) |
| `attach(service)` | `#[doc(hidden)]` completion path; also the in-process test injection point |
| `disconnect()` | Close the connection and clear the tools cache |
| `is_connected()` | Non-blocking peek (never deadlocks) |
| `list_tools()` | Return the cached tool list (clone) |
| `call_tool(name, arguments)` | Call a remote tool by name with a JSON object of arguments |

Calls are serialized through the client's `tokio::sync::Mutex`, so many `McpTool` instances can share one live connection safely.

### 2.3 `McpTool`

An adapter exposing a remote MCP tool as an `agent_scope_tool::Tool`:

- `name()` → `"{mcp_name}/{tool_name}"` (e.g. `excalidraw/create_element`)
- `description()` → `"[remote MCP: {mcp_name}] {original description}"`
- `input_schema()` → the remote JSON Schema
- `read_only` propagates from the remote tool's `annotations.read_only_hint`
- `call(input)` → forwards to the shared client, concatenates text content blocks, maps errors into the unified `ToolError` taxonomy

### 2.4 `McpExt` (workspace extension)

```rust
#[async_trait]
pub trait McpExt: WorkspaceBase {
    async fn connect_mcp(&mut self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError>;
    async fn disconnect_mcp(&mut self, name: &str) -> Result<(), WorkspaceError>;
    async fn get_mcp_tools(&self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError>;
}
```

Implemented for `LocalWorkspace`. Adding the trait to the workspace public signature would break the compatibility baseline (Constitution Article 1), so connection lifecycle lives in this extension trait instead.

## 3. Quick Example (Quick Example)

```rust
use agent_scope_mcp::McpExt;
use agent_scope_workspace::mcp::{McpClientConfig, McpTransportConfig};
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
    workdir: "/tmp/my-workspace".into(),
    workspace_id: None,
    default_mcps: vec![McpClientConfig {
        name: "excalidraw".into(),
        transport: McpTransportConfig::Stdio {
            command: "mcp-excalidraw-server".into(),
            args: vec![],
        },
        is_stateful: true,
    }],
    skill_paths: vec![],
    instructions: None,
});
ws.initialize().await?;

// Connect to the registered server and get its tools as adapters.
let tools = ws.connect_mcp("excalidraw").await?;

// The cached list stays queryable for the lifetime of the connection.
let cached = ws.get_mcp_tools("excalidraw").await?;
assert_eq!(cached.len(), tools.len());

// Release the live connection and its subprocess/socket when done.
ws.disconnect_mcp("excalidraw").await?;
```

`close()` / `reset()` on the workspace also disconnect every live MCP connection, so long-lived processes cannot leak subprocesses or sockets (FR-010).

## 4. Key Usage Patterns (Usage Patterns)

### 4.1 Persisted configuration via `.mcp`

Registered configs are persisted as a JSON array in `<workdir>/.mcp`:

```json
[
  {
    "name": "excalidraw",
    "transport": { "type": "stdio", "command": "mcp-excalidraw-server", "args": [] },
    "is_stateful": true
  }
]
```

Manage them through `WorkspaceBase` methods `add_mcp` / `remove_mcp` / `list_mcps`. On a corrupt file, initialization falls back to `default_mcps` with a warning.

### 4.2 Sensitive header scrubbing

Header names such as `authorization`, `x-api-key`, and `cookie` are **always scrubbed** to `[REDACTED]` — both when persisting `.mcp` and when returned by `list_mcps()`. A config snapshot at connect time already carries the scrubbed copy, so the runtime layer never touches raw credentials (FR-003 / FR-009).

### 4.3 Ad-hoc registration without `default_mcps`

```rust
use agent_scope_workspace::mcp::{McpClientConfig, McpTransportConfig};

let config = McpClientConfig {
    name: "search".into(),
    transport: McpTransportConfig::StreamableHttp {
        url: "https://api.example.com/mcp".into(),
        headers: Default::default(),
    },
    is_stateful: true,
};
ws.add_mcp(config).await?;
let tools = ws.connect_mcp("search").await?;
```

### 4.4 Calling a remote tool directly through the adapter

```rust
use agent_scope_tool::{Tool, ToolExecOutput};
use serde_json::json;

let create = tools.iter().find(|t| t.name() == "excalidraw/create_element").unwrap();
match create.call(json!({ "type": "rectangle", "x": 100, "y": 100, "width": 200, "height": 120 })).await? {
    ToolExecOutput::Complete(block) => println!("{}", block.output),
    ToolExecOutput::Stream(_) => println!("<streaming>"),
}
```

## 5. Errors (Errors)

| `WorkspaceError` | Cause |
|------------------|-------|
| `McpNotFound { name }` | No persisted config with that name |
| `McpAlreadyExists { name }` | A config with that name already exists (or a live connection is already registered) |
| `McpConnectionError { name, reason }` | Transport-level failure: spawn failed, `serve` failed, or the connection broke |
| `McpCallError { mcp_name, tool_name, reason }` | Protocol/peer error during a call (typed MCP error, timeout, cancelled) |
| `McpNotConnected { name }` | Operation on a client that is not (or no longer) connected |

`McpTool` maps these into the unified `ToolError::Execution` / `ToolError::InvalidInput` taxonomy for the agent loop.

## 6. Compatibility (Compatibility)

- **Compatibility Level**: **L2** (external tool integration; no Python-side counterpart)
- **Authority**: `specs/027-mcp-sdk-integration/spec.md`
- **Implementation**: `rmcp` v3.1.1 (official Rust MCP SDK) with features `client`, `transport-child-process`, `transport-streamable-http-client-reqwest`, `transport-worker`
- **Validation**: in-process integration tests run over `tokio::io::duplex` (no external process/network). A real-world stdio round-trip is exercised by the example `crates/agent_scope_mcp/examples/mcp_excalidraw_debug.rs` against `mcp-excalidraw-server` (discover 26 tools, call `clear_canvas` / `create_element` / `describe_scene` / `query_elements`).
- **Known Deviations**: SSE is not a first-class `rmcp` transport; legacy `sse` configs are mapped to streamable-http. `resources` / `prompts` server capabilities are not surfaced (tools only).

## 7. See Also (Related Modules)

- [Workspace](./workspace.md) — owns persisted MCP config (`McpClientConfig`), `add_mcp` / `remove_mcp` / `list_mcps`
- [Tool System](./tool.md) — the unified tool contract that `McpTool` adapters implement
- [Agent System](./agent.md) — how Agents consume tools, including remote MCP tools
- [Skill System](./skill.md) — local skill tools, the counterpart to remote MCP tools
