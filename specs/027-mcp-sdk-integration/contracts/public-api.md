# Interface Contracts: MCP SDK Integration

**Feature**: 027-mcp-sdk-integration

This document defines the public API contracts introduced or modified by this feature.

---

## Contract 1: WorkspaceBase Extension Methods

### `connect_mcp`

```rust
/// Connect to an MCP server by name, returning its tools as [`Tool`] objects.
///
/// # Errors
/// - `McpNotFound` — no config registered with this name
/// - `McpConnectionError` — transport, handshake, or protocol error (reason never leaks secrets)
/// - `McpAlreadyExists` — already connected (call `disconnect_mcp` first)
async fn connect_mcp(&mut self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError>;
```

**Preconditions**:
- Workspace is initialized (`is_alive() == true`)
- A config exists with the given `name` (via `add_mcp`)
- Not already connected for this name

**Postconditions**:
- Connection established, handshake completed
- Remote tools discovered and cached
- Returns `Vec<Arc<dyn Tool>>` where each `Tool` is an `McpTool` adapter
- Tool names use `{mcp_name}/{tool_name}` prefix format

**Example**:
```rust
let tools = ws.connect_mcp("my-search-server").await?;
// tools[0].name() == "my-search-server/web_search"
// tools[0].description() == "[remote MCP: my-search-server] Search the web"
```

### `disconnect_mcp`

```rust
/// Disconnect an MCP client by name, releasing resources.
///
/// # Errors
/// - `McpNotFound` — no config or connection with this name
/// - `McpNotConnected` — config exists but not connected
async fn disconnect_mcp(&mut self, name: &str) -> Result<(), WorkspaceError>;
```

**Postconditions**:
- Connection terminated (`RunningService::cancel()`)
- Child process killed (if stdio)
- Tools cache cleared

### `get_mcp_tools`

```rust
/// Get the cached tool list for a connected MCP client.
///
/// # Errors
/// - `McpNotConnected` — not connected (call `connect_mcp` first)
fn get_mcp_tools(&self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError>;
```

**Preconditions**: Connected via `connect_mcp(name)`

**Guarantees**: Returns cached tools without re-discovering from server.

---

## Contract 2: McpTool — Tool Trait Implementation

### Trait conformance

`McpTool` implements `agent_scope_tool::Tool`:

```rust
impl Tool for McpTool {
    fn name(&self) -> &str;               // "{mcp_name}/{tool_name}"
    fn description(&self) -> &str;        // "[remote MCP: {mcp_name}] {original_description}"
    fn input_schema(&self) -> JsonValue;  // original JSON Schema from server
    fn is_concurrency_safe(&self) -> bool; // true
    fn is_read_only(&self) -> bool;       // from annotations.read_only_hint
    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError>;
}
```

### `call()` contract

1. Serialize `input: JsonValue` → `serde_json::Map<String, Value>` (JSON object)
2. Construct `CallToolRequestParams::new(tool_name).with_arguments(args)`
3. Call `McpClient::call_tool()` 
4. Map `CallToolResult` → `ToolExecOutput::Complete(ToolResultBlock { ... })`
5. Map errors:
   - `McpNotConnected` → `ToolError::Execution { reason: "MCP not connected" }`
   - `McpCallError` → `ToolError::Execution { reason }`
   - `ServiceError::Timeout` → `ToolError::Interrupted { reason: "MCP call timed out" }`

### Result content mapping

| `CallToolResult` field | `ToolResultBlock` field |
|---|---|
| `content[].text` (concatenated) | `content: vec![TextBlock { text }]` |
| `structured_content` | 纳入 result metadata（如含） |
| `is_error == Some(true)` | `state: Error` + error text from content |

---

## Contract 3: Configuration Persistence Contract (preserved)

Existing contract from Feature 012, verified unchanged:

```rust
// Configuration registration (no behavioral change)
ws.add_mcp(config).await?;              // Persists to .mcp, rejects duplicates
let configs = ws.list_mcps().await?;    // Returns scrubbed copies
ws.remove_mcp("name").await?;           // Removes from memory + .mcp, silent on missing
```

**Invariant**: Sensitive headers are scrubbed in `.mcp` persistence and `list_mcps()` output (regression from defect 3 fix).

**SSE Compatibility**: `McpTransportConfig::Sse {...}` can still be deserialized from existing `.mcp` files. `connect_mcp()` will map it to streamable-http and emit `info!` log.

---

## Contract 4: Error Model

### New WorkspaceError variants

| Variant | Meaning | HTTP analogy |
|---------|---------|-------------|
| `McpConnectionError { name, reason }` | Transport/handshake/protocol failure | 502/503 |
| `McpCallError { mcp_name, tool_name, reason }` | Tool execution failed on server | 500 |
| `McpNotConnected { name }` | `connect_mcp()` not called or connection lost | 503 |

### Secret safety

- `reason` field MUST NOT contain API keys, tokens, or raw header values
- Error messages derived from `rmcp::ServiceError` are scrubbed before being stored in `reason`

---

## Contract 5: Test Transport Contract

### WorkerTransport availability

Test code can create MCP connections using `rmcp`'s `WorkerTransport`:

```rust
// Server side: define tool handler implementing rmcp server traits
// Client side: serve_client(handler, WorkerTransport::new(server_rx, client_tx))
```

**Contract**: All unit/integration tests in CI MUST use `WorkerTransport`. Real-transport tests (`#[ignore]`) are optional.

### Test fixture contract

```rust
/// Creates a connected McpClient against an in-process server exposing
/// one tool: "add(a: i64, b: i64) -> a + b".
async fn setup_test_mcp() -> (McpClient, Arc<McpTool>) {
    // 1. Create WorkerTransport pair
    // 2. Spawn server handler with "add" tool
    // 3. Create McpClient + connect
    // 4. Return (client, add_tool)
}
```
