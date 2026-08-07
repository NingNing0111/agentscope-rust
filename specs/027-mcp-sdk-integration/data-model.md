# Data Model: MCP SDK Integration

**Feature**: 027-mcp-sdk-integration

---

## Entity Overview

```
McpClientConfig ──owns──▶ McpTransportConfig
       │
       │ 1:1 (connect)
       ▼
  McpClient ──wraps──▶ rmcp::RunningService<RoleClient>
       │                        │
       │ 1:N (list/discover)    │ call_tool()
       ▼                        ▼
  McpTool ──implements──▶ agent_scope_tool::Tool
```

---

## Entities

### McpTransportConfig（已有，小幅修改）

传输配置枚举。三个变体，`Sse` 变体通过 `#[serde(alias)]` 保留向后兼容。

| 字段 | 类型 | 说明 |
|------|------|------|
| `Stdio::command` | `String` | 子进程命令 |
| `Stdio::args` | `Vec<String>` | 子进程参数 |
| `Sse::url` | `String` | SSE 端点 URL（**保留**，运行时映射到 streamable-http） |
| `Sse::headers` | `HashMap<String, String>` | 请求头（**脱敏后持久化**） |
| `StreamableHttp::url` | `String` | streamable-http 端点 URL |
| `StreamableHttp::headers` | `HashMap<String, String>` | 请求头（**脱敏后持久化**） |

**不变式**：
- 任何 `headers` 出现在 `.mcp` 文件或 `list_mcps()` 返回值时，敏感值 MUST 为 `[REDACTED]`
- `Sse` 变体解析成功（存量文件），运行时发出 `info!` 提示

### McpClientConfig（已有，不变）

MCP 客户端持久化配置。

| 字段 | 类型 | 约束 |
|------|------|------|
| `name` | `String` | 唯一标识，1-128 字符 |
| `transport` | `McpTransportConfig` | 传输配置 |
| `is_stateful` | `bool` | 默认 `true`；决定 disconnect 行为 |

**关系**：`McpClientConfig` 是持久化层的表示（→ `.mcp` 文件）；`McpClient` 是运行时层的表示（→ 连接）。

### McpClient（新增）

运行时 MCP 客户端，封装连接生命周期。**不是** `Serialize/Deserialize`——它是纯运行时对象。

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `String` | 对应 `McpClientConfig.name` |
| `config` | `McpClientConfig` | 创建时的配置快照 |
| `service` | `Arc<Mutex<Option<RunningService>>>` | SDK 连接句柄；`None` 表示未连接 |
| `tools_cache` | `Mutex<Vec<rmcp::model::Tool>>` | 连接时缓存的工具清单 |

**状态机**：

```
  ConfigCreated ──connect()──▶ Connected ──disconnect()──▶ Disconnected
       │                            │                          │
       │                            │ call_tool()              │
       │                            ▼                          │
       │                      [calling...]                     │
       │                            │                          │
       └────────────────────────────┴──────────────────────────┘
                               close() / reset()
```

**不变式**：
- `tools_cache` 在 `connect()` 成功时填充，在 `disconnect()` 后清空
- `call_tool()` 在未连接状态下返回 `McpNotConnected` 错误
- 并发调用由 `Arc<Mutex<...>>` 保护，`RunningService` 内部支持多路复用

### McpTool（新增）

远端 MCP 工具适配器，实现 `agent_scope_tool::Tool`。

| 字段 | 类型 | 来源 |
|------|------|------|
| `mcp_name` | `String` | `McpClientConfig.name` |
| `tool_name` | `String` | `rmcp::model::Tool.name`（对外暴露时加前缀如需要） |
| `description` | `String` | `rmcp::model::Tool.description` |
| `input_schema` | `JsonValue` | `rmcp::model::Tool.input_schema` |
| `client` | `Arc<McpClient>` | 用于 `call()` 转发 |
| `read_only` | `bool` | `rmcp::model::Tool.annotations.read_only_hint` |

**方法**：

| 方法 | 实现 |
|------|------|
| `name()` | 返回 `{mcp_name}/{tool_name}` 前缀形式（即使无冲突也加前缀以保证唯一性） |
| `description()` | 返回 `"[remote MCP: {mcp_name}] {description}"` |
| `input_schema()` | 返回 `rmcp::model::Tool.input_schema` 的 JSON 值 |
| `call(input)` | 转发到 `McpClient::call_tool(tool_name, input)` |
| `is_concurrency_safe()` | `true` |
| `is_read_only()` | 从 annotations 派生 |

### WorkspaceBase 扩展（新增方法）

| 方法 | 签名 | 说明 |
|------|------|------|
| `connect_mcp` | `&mut self, name: &str` → `Result<Vec<Arc<dyn Tool>>, WorkspaceError>` | 按名称连接已注册的 MCP 客户端，返回工具列表 |
| `disconnect_mcp` | `&mut self, name: &str` → `Result<(), WorkspaceError>` | 断开指定 MCP 连接 |
| `get_mcp_tools` | `&self, name: &str` → `Result<Vec<Arc<dyn Tool>>, WorkspaceError>` | 获取已连接 MCP 的工具列表（缓存） |

---

## 数据流

### 配置注册 → 连接 → 工具调用

```
1. add_mcp(config)     → 持久化到 .mcp + 加入 _mcps 列表
2. connect_mcp("name") → 按配置创建 transport → serve → RunningService
                       → list_all_tools() → 缓存 tools
                       → 为每个 Tool 创建 McpTool 适配器
3. get_mcp_tools("name") → 返回缓存工具列表
4. agent.call_tool(mcp_tool, args)
       → McpTool::call(args)
       → McpClient::call_tool(tool_name, args)
       → Peer::call_tool(CallToolRequestParams)
       → CallToolResult → 映射为 ToolExecOutput
```

### close() / reset() 时的清理

```
close() →
  for each McpClient:
    disconnect() → RunningService::cancel() → 等待 quit
    tools_cache.clear()

reset() →
  close() 的清理逻辑
  _mcps.clear()
  删除 .mcp 文件
```
