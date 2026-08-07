# Research: MCP SDK Integration

**Feature**: 027-mcp-sdk-integration
**Date**: 2026-08-07
**Source**: rmcp v3.1.1 (modelcontextprotocol/rust-sdk, cloned to /tmp/mcp-rust-sdk-study)

---

## 1. SDK 版本与许可

- **Decision**: 锁定 `rmcp` v3.1.1（crates.io 发布）
- **Rationale**: 当前稳定版；实现 MCP 2026-07-28 规范，向后兼容 2025-11-25；活跃维护中
- **Alternatives**: `rmcp` 2.x（已不推荐，有 3.x 迁移指南）；自研 MCP 协议（违反宪法第四条"不自研协议层"假设）
- **License**: MIT

---

## 2. 客户端 API 形态

### 2.1 核心类型链

```
McpClientConfig (持久化配置)
  → rmcp transport (TokioChildProcess / StreamableHttpClientTransport)
    → ().serve(transport) / ClientInfo::default().serve(transport)
      → RunningService<RoleClient, impl Service>
        → .peer() → Peer<RoleClient>
          → .list_tools(None) → ListToolsResult { tools: Vec<Tool>, next_cursor }
          → .list_all_tools() → Vec<Tool>                   // 自动分页
          → .call_tool(CallToolRequestParams) → CallToolResult
        → .cancel() → QuitReason                             // 断开连接
```

### 2.2 关键 API 签名

```rust
// 创建 stdio 客户端
let client = ().serve(TokioChildProcess::new(Command::new("npx").configure(|cmd| {
    cmd.arg("-y").arg("@modelcontextprotocol/server-everything");
}))?).await?;

// 创建 streamable-http 客户端
let transport = StreamableHttpClientTransport::from_uri("http://localhost:8000/mcp");
let client = ClientInfo::default().serve(transport).await?;

// 工具操作
let tools: Vec<rmcp::model::Tool> = client.list_all_tools().await?;
let result: CallToolResult = client.call_tool(
    CallToolRequestParams::new("echo").with_arguments(serde_json::json!({"message": "hi"}).as_object().unwrap().clone())
).await?;

// 断开
client.cancel().await?;
```

- **Decision**: 使用 `().serve(transport)` 默认 handler（`()` 实现 `Service<RoleClient>`，无自定义 handler 回调）；`list_all_tools()` 自动处理分页
- **Rationale**: 客户端回调功能（`create_message`、`list_roots` 等）不在本次 scope（仅聚焦 tools 协议），默认 handler 即可覆盖需求
- **Alternatives**: 自定义 `impl ClientHandler`（复杂但完整，留给后续 Feature）

### 2.3 关闭/断开机制

- `RunningService::cancel()` → 发取消信号 → 返回 `QuitReason`
- `RunningService` 在 drop 时不会自动取消，必须显式调用
- `Peer::cancel(reason)` 可取消单个 in-flight 请求

- **Decision**: `McpClient` 的 `disconnect()` 调用 `RunningService::cancel()`；`Drop` 实现中异步取消（通过 `tokio::spawn` 确保清理）
- **Rationale**: 对齐 `close()`/`reset()` 的资源释放语义（FR-010）；`cancel()` 是 safe shutdown

---

## 3. 传输层映射

### 3.1 传输类型对应表

| 当前 `McpTransportConfig` 变体 | rmcp transport 类型 | feature flag | 说明 |
|---|---|---|---|
| `Stdio { command, args }` | `TokioChildProcess` | `transport-child-process` | 启动子进程，stdin/stdout JSON-RPC |
| `Sse { url, headers }` | ❌ SDK 不支持 | — | 映射到 StreamableHttp，见 3.2 |
| `StreamableHttp { url, headers }` | `StreamableHttpClientTransport` | `transport-streamable-http-client-reqwest` | HTTP POST + SSE 响应流 |

### 3.2 SSE 向后兼容方案

官方 SDK 明确将其列为"deliberate non-goal"。处理方案：`McpTransportConfig::Sse` 变体保留不删，添加 `#[serde(alias = "sse")]` 确保存量 `.mcp` 文件仍可解析；运行时将 SSE 配置映射到 `StreamableHttpClientTransport`：

```rust
McpTransportConfig::Sse { url, headers } => {
    tracing::info!("MCP SSE config mapped to streamable-http for {url}");
    StreamableHttpClientTransport::from_uri(&url)
}
```

- **Decision**: 保留 SSE 变体 + `#[serde(alias)]`，运行时映射 + `info!` 提示
- **Rationale**: 存量 `.mcp` 不坠毁；用户可见迁移提示（FR-002）
- **Known Deviation**: MCP 2024-11-05 HTTP+SSE 是 MCP 2026-07-28 streamable-http 的前身，语义有差异（session 管理方式不同），记录于兼容性矩阵

### 3.3 请求头注入

`StreamableHttpClientTransport` 支持自定义 headers 吗？需要查看 constructor：

```rust
// StreamableHttpClientTransport 构造
let transport = StreamableHttpClientTransport::from_uri("http://...");
```

SDK 的 `StreamableHttpClientTransport` 目前从源代码看，header 注入可以通过 `reqwest::Client` builder 的自定义实现。需要确认是否支持 per-request headers。

- **Decision**: headers 通过自定义 `reqwest::Client` 注入（`StreamableHttpClientTransport` 支持传入自定义 `reqwest::Client` builder），headers 值在内存中使用原始值（不脱敏），仅持久化 `.mcp` 和 `list_mcps()` 返回时脱敏
- **Rationale**: 运行时需要真实 headers 连接服务器；敏感信息隔离已在现有 `scrubbed()` 逻辑中覆盖

---

## 4. Tool 适配层设计

### 4.1 MCP Tool → agent_scope_tool::Tool

| `agent_scope_tool::Tool` 方法 | 映射来源 |
|---|---|
| `name()` | `rmcp::model::Tool.name` |
| `description()` | `rmcp::model::Tool.description`（可选，fallback 到 `""`） |
| `input_schema()` | `rmcp::model::Tool.input_schema`（`Arc<JsonObject>` → `JsonValue`） |
| `call(input)` | `Peer::call_tool(CallToolRequestParams::new(name).with_arguments(input))` |
| `is_concurrency_safe()` | `true`（远端服务独立处理并发） |
| `is_read_only()` | `rmcp::model::Tool::annotations.read_only_hint`（可选） |

### 4.2 工具命名冲突策略

当 MCP 工具名称与本地工具重名时使用 `{mcp_name}/{tool_name}` 前缀格式（例如 mcp 名为 `search`、工具名为 `query` → `search/query`）。

- **Decision**: `{mcp_client_name}/{tool_name}` 前缀，避免需要用户显式配置；可通过 `McpClientConfig` 的 `name` 字段区分不同 MCP 服务器
- **Rationale**: 自动去重、可预测、无需额外配置字段

### 4.3 结果映射

- `CallToolResult.content`（`Vec<ContentBlock>`）→ 提取 `text` 字段 → 拼接为 `ToolResultBlock`
- `CallToolResult.is_error` → `ToolError::Execution`
- `CallToolResult.structured_content` → 保留为 JSON（如果存在）
- 连接失败/超时/取消 → 映射到 `ToolError` 对应变体

---

## 5. 进程内测试 (WorkerTransport)

SDK 提供 `WorkerTransport`（`transport-worker` feature）用于进程内测试：

```rust
// 定义 Server handler + Tool
// 通过 WorkerTransport 在同一进程内建立 client<>server 通道
let (client, _server_handle) = ...;
let tools = client.list_all_tools().await?;
let result = client.call_tool(...).await?;
```

- **Decision**: CI 测试使用 `WorkerTransport`，真实传输测试标为可选项（`#[ignore]` 或 feature-gated）
- **Rationale**: 确定性、快速、无外部依赖（FR-015）

---

## 6. 依赖引入策略

### 6.1 Cargo.toml 变更

```toml
[dependencies]
rmcp = { version = "3.1.1", default-features = false, features = [
    "client",
    "transport-child-process",
    "transport-streamable-http-client-reqwest",
    "transport-worker",
] }
```

这些 features 的依赖链：
- `client` → tokio-stream
- `transport-child-process` → async-rw + tokio/process + process-wrap
- `transport-streamable-http-client-reqwest` → streamable-http-client + reqwest
- `transport-worker` → tokio-stream

### 6.2 依赖冲突检查

`rmcp` 依赖与现有 `agent_scope_workspace` 依赖的兼容性：
- `tokio = "1"` — 一致
- `serde = "1.0"` — 一致
- `serde_json = "1.0"` — 一致
- `tracing = "0.1"` — 一致
- `futures = "0.3"` — 一致
- `reqwest = "0.13.2"` — **新引入**（workspace 目前无 reqwest），需确认版本兼容
- `process-wrap = "9.0"` — 新引入，仅 stdio 场景使用

- **Decision**: 使用 `default-features = false` 最小化依赖引入；reqwest 是新依赖但有限作用域（仅 streamable-http 客户端）
- **Rationale**: 避免 feature 污染；清晰声明

---

## 7. Python 参考行为

### 7.1 Python MCPClient（`agentscope.mcp._mcp_client.MCPClient`）

从 api-inventory.json：
- `config` 字段：`StdioMCPConfig` / `HttpMCPConfig`
- `connect()` → 启动子进程或 HTTP 连接
- `list_tools()` → 返回 `Tool` 列表
- `call_tool(name, arguments)` → 返回工具结果
- `close()` → 断开连接

### 7.2 Python MCPTool（`agentscope.tool._adapters.MCPTool`）

- 实现 `ToolProtocol`
- `name` / `description` / `parameters`（JSON Schema）→ 从 MCP Tool 派生
- `__call__(args)` → 通过 `MCPClient.call_tool()` 转发

### 7.3 行为基准对齐项

| 行为 | Python | Rust（本次） | 偏差 |
|------|--------|-------------|------|
| 配置注册后默认不自动连接 | ✅ | ✅ | 无 |
| 工具名称冲突处理 | 未知 | `{mcp_name}/{tool_name}` | 新增 |
| 连接断开后工具调用 | 返回错误 | 返回 `McpNotConnected` | 类型一致 |
| SSE 配置行为 | 自有 SSE 实现 | 映射到 streamable-http | **已知偏差** |
| close 时断开连接 | ✅ | ✅ | 无 |

---

## 8. 错误模型设计

新增 `WorkspaceError` 变体（对用户透明，不改变现有错误结构）：

```rust
pub enum WorkspaceError {
    // ...existing variants...
    /// MCP connection failed (handshake, transport, or protocol error).
    McpConnectionError { name: String, reason: String },
    /// MCP tool call failed.
    McpCallError { mcp_name: String, tool_name: String, reason: String },
    /// MCP client not connected — call connect_mcp() first.
    McpNotConnected { name: String },
}
```

错误来源映射：
- `rmcp::ServiceError::McpError` → `McpCallError`
- `rmcp::ServiceError::TransportSend` / `TransportClosed` → `McpConnectionError`
- `rmcp::ServiceError::Timeout` → `McpCallError`
- `rmcp::ClientInitializeError` → `McpConnectionError`
- 敏感信息绝不泄漏到 `reason` 字段
