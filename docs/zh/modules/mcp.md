# MCP 集成 / MCP Integration

> 一句话定位：`agent_scope_mcp` 把外部 **Model Context Protocol (MCP)** 服务器变成统一的 `agent_scope_tool::Tool` 适配器——Agent 连接到已注册的 MCP 服务器（stdio 子进程或 streamable-http），发现其远程工具，并通过与本地工具一致的工具契约调用它们。基于官方 Rust MCP SDK `rmcp` 构建。

## 1. 模块概述 (Overview)

| 组件 | 职责 |
|------|------|
| `McpClient` | 运行时 MCP 客户端，包装 `rmcp` 连接：连接/断开、工具发现、工具调用 |
| `McpTool` | 把远程 MCP 工具适配成 `agent_scope_tool::Tool` 的适配器 |
| `McpExt` | 扩展 trait，为 workspace 添加 `connect_mcp` / `disconnect_mcp` / `get_mcp_tools` |
| `McpClientConfig` / `McpTransportConfig` | **持久化**配置（Stdio / SSE / StreamableHttp），归属 `agent_scope_workspace` |
| `.mcp` 文件 | 已注册 MCP 配置的 JSON 数组持久化，存放于 workspace 根目录 |

**适用场景**：Agent 需要的工具位于外部服务（Excalidraw 画布、网页搜索、文件编辑器等），而非本地实现。

**前置阅读**：[工作空间](./workspace.md)（持有持久化 MCP 配置）、[工具系统](./tool.md)（统一工具契约）、[Agent 系统](./agent.md)。

**架构说明**：本 crate 位于 `agent_scope_workspace` 与 `agent_scope_tool` *之上*，以打破 crate 循环依赖——workspace crate 持有持久化的 MCP *配置*，而 `agent_scope_mcp` 持有*运行时连接*与*工具适配器*。

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 传输方式（`McpTransportConfig`）

```rust
#[serde(tag = "type")]
pub enum McpTransportConfig {
    Stdio { command: String, args: Vec<String> },
    Sse { url: String, headers: HashMap<String, String> },
    StreamableHttp { url: String, headers: HashMap<String, String> },
}
```

| 变体 | 线路格式 | 说明 |
|------|----------|------|
| `Stdio` | 派生子进程，通过 stdin/stdout 通信 | Node/Python MCP 服务器最常见（例如 `npx -y mcp-excalidraw-server`） |
| `StreamableHttp` | HTTP 流式（现代 MCP 规范传输） | 原生支持 |
| `Sse` | 遗留传输 | 连接时映射到 `StreamableHttp` 并发出 `info!` 日志（FR-002）；为兼容旧 `.mcp` 文件保留 |

### 2.2 `McpClient`

一条活跃的 MCP 连接。从持久化的 [`McpClientConfig`](./workspace.md) 创建，在连接期间持有 `rmcp` 的 `RunningService`。

```rust
pub struct McpClient {
    name: String,
    config: McpClientConfig,
    service: tokio::sync::Mutex<Option<RunningService<RoleClient, ClientInfo>>>,
    tools_cache: std::sync::Mutex<Vec<Tool>>,
}
```

关键方法：

| 方法 | 行为 |
|------|------|
| `new(config)` | 从持久化配置创建（尚未连接） |
| `connect()` | 为配置变体构建传输、建立连接、发现工具（幂等） |
| `attach(service)` | `#[doc(hidden)]` 完成路径；也是进程内测试的注入点 |
| `disconnect()` | 关闭连接并清空工具缓存 |
| `is_connected()` | 非阻塞探测（永不死锁） |
| `list_tools()` | 返回缓存的工具列表（克隆） |
| `call_tool(name, arguments)` | 按名调用远程工具，参数为 JSON 对象 |

调用通过客户端的 `tokio::sync::Mutex` 串行化，因此多个 `McpTool` 实例可安全共享同一条活跃连接。

### 2.3 `McpTool`

把远程 MCP 工具适配成 `agent_scope_tool::Tool` 的适配器：

- `name()` → `"{mcp_name}/{tool_name}"`（例如 `excalidraw/create_element`）
- `description()` → `"[remote MCP: {mcp_name}] {原始描述}"`
- `input_schema()` → 远程 JSON Schema
- `read_only` 从远程工具的 `annotations.read_only_hint` 传递
- `call(input)` → 转发到共享客户端，拼接文本内容块，把错误映射进统一的 `ToolError` 分类

### 2.4 `McpExt`（workspace 扩展）

```rust
#[async_trait]
pub trait McpExt: WorkspaceBase {
    async fn connect_mcp(&mut self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError>;
    async fn disconnect_mcp(&mut self, name: &str) -> Result<(), WorkspaceError>;
    async fn get_mcp_tools(&self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError>;
}
```

为 `LocalWorkspace` 实现。把该 trait 加入 workspace 公开签名会破坏兼容基线（宪法规约第一条），因此连接生命周期放在这个扩展 trait 中。

## 3. 快速示例 (Quick Example)

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

// 连接到已注册的服务器，拿到其工具适配器。
let tools = ws.connect_mcp("excalidraw").await?;

// 缓存列表在连接生命周期内可持续查询。
let cached = ws.get_mcp_tools("excalidraw").await?;
assert_eq!(cached.len(), tools.len());

// 结束后释放活跃连接及其子进程/套接字。
ws.disconnect_mcp("excalidraw").await?;
```

workspace 的 `close()` / `reset()` 也会断开所有活跃 MCP 连接，长驻进程不会泄漏子进程或套接字（FR-010）。

## 4. 关键用法模式 (Usage Patterns)

### 4.1 通过 `.mcp` 持久化配置

已注册配置以 JSON 数组持久化到 `<workdir>/.mcp`：

```json
[
  {
    "name": "excalidraw",
    "transport": { "type": "stdio", "command": "mcp-excalidraw-server", "args": [] },
    "is_stateful": true
  }
]
```

通过 `WorkspaceBase` 的 `add_mcp` / `remove_mcp` / `list_mcps` 管理。文件损坏时，初始化回退到 `default_mcps` 并发出警告。

### 4.2 敏感 header 脱敏

`authorization`、`x-api-key`、`cookie` 等 header 名**始终**被脱敏为 `[REDACTED]`——无论写入 `.mcp` 还是由 `list_mcps()` 返回。连接时快照的配置已携带脱敏副本，运行时层不会触及原始凭据（FR-003 / FR-009）。

### 4.3 不通过 `default_mcps` 的临时注册

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

### 4.4 直接通过适配器调用远程工具

```rust
use agent_scope_tool::{Tool, ToolExecOutput};
use serde_json::json;

let create = tools.iter().find(|t| t.name() == "excalidraw/create_element").unwrap();
match create.call(json!({ "type": "rectangle", "x": 100, "y": 100, "width": 200, "height": 120 })).await? {
    ToolExecOutput::Complete(block) => println!("{}", block.output),
    ToolExecOutput::Stream(_) => println!("<streaming>"),
}
```

## 5. 错误处理 (Errors)

| `WorkspaceError` | 触发原因 |
|------------------|----------|
| `McpNotFound { name }` | 不存在该名称的持久化配置 |
| `McpAlreadyExists { name }` | 该名称配置已存在（或已有活跃连接被注册） |
| `McpConnectionError { name, reason }` | 传输层失败：派生失败、`serve` 失败或连接断开 |
| `McpCallError { mcp_name, tool_name, reason }` | 调用期间的协议/对端错误（类型化 MCP 错误、超时、取消） |
| `McpNotConnected { name }` | 对未（或不再）连接的客户端执行操作 |

`McpTool` 把这些映射进统一的 `ToolError::Execution` / `ToolError::InvalidInput` 分类，供 Agent 循环使用。

## 6. 兼容性 (Compatibility)

- **兼容级别**：**L2**（外部工具集成；无 Python 侧对应物）
- **权威**：`specs/027-mcp-sdk-integration/spec.md`
- **实现**：`rmcp` v3.1.1（官方 Rust MCP SDK），feature 含 `client`、`transport-child-process`、`transport-streamable-http-client-reqwest`、`transport-worker`
- **验证**：进程内集成测试通过 `tokio::io::duplex` 运行（无外部进程/网络）。真实 stdio 往返由示例 `crates/agent_scope_mcp/examples/mcp_excalidraw_debug.rs` 对 `mcp-excalidraw-server` 验证（发现 26 个工具，调用 `clear_canvas` / `create_element` / `describe_scene` / `query_elements`）
- **已知偏差**：SSE 不是 `rmcp` 的一等传输，遗留 `sse` 配置被映射为 streamable-http。`resources` / `prompts` 服务器能力未暴露（仅工具）

## 7. 相关模块 (See Also)

- [工作空间](./workspace.md) — 持有持久化 MCP 配置（`McpClientConfig`）、`add_mcp` / `remove_mcp` / `list_mcps`
- [工具系统](./tool.md) — `McpTool` 适配器实现的统一工具契约
- [Agent 系统](./agent.md) — Agent 如何消费工具，包括远程 MCP 工具
- [技能系统](./skill.md) — 本地技能工具，与远程 MCP 工具互为对照
