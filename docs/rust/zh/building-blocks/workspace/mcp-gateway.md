---
title: "MCP Gateway"
description: "工作空间与外部 MCP 服务的接入方式"
---

<Note>
**Rust 实现状态**: 部分支持。
- 已支持：MCP 客户端接入（`McpExt::connect_mcp` 连接外部 MCP server 并把工具接入 ToolKit）。
- 尚未实现：沙箱内运行 gateway 进程、把本地工具暴露为 MCP 服务端的能力。
</Note>

**MCP Gateway** 是运行在沙箱内、把上游 MCP server 会话暴露给宿主的轻量进程。AgentScope Rust 当前**没有实现服务端 gateway**，而是以**客户端接入**方式提供等价价值。

## 客户端接入

通过 `McpExt`（`agent_scope_mcp`）连接外部 MCP server：

```rust
use agent_scope_mcp::McpExt;
use agent_scope_workspace::mcp::{McpClientConfig, McpTransportConfig};
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
    workdir: "/tmp/ws".into(),
    ..Default::default()
});
ws.initialize().await?;

ws.add_mcp(McpClientConfig {
    name: "my-server".into(),
    transport: McpTransportConfig::Stdio {
        command: "mcp-server".into(),
        args: vec![],
    },
    is_stateful: true,
}).await?;

let tools = ws.connect_mcp("my-server").await?;   // 远程工具接入 ToolKit
ws.disconnect_mcp("my-server").await?;
```

## McpClientConfig / McpTransportConfig

`McpClientConfig` 描述一个 MCP 客户端连接：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `String` | 客户端唯一名 |
| `transport` | `McpTransportConfig` | 传输方式 |
| `is_stateful` | `bool` | 是否维护有状态连接（默认 `true`） |

`McpTransportConfig` 三种传输方式：

| 变体 | 字段 | 说明 |
|------|------|------|
| `Stdio` | `command: String`, `args: Vec<String>` | 通过标准输入输出启动本地 MCP server 进程 |
| `Sse` | `url: String`, `headers: HashMap` | 通过 Server-Sent Events 连接远程 server |
| `StreamableHttp` | `url: String`, `headers: HashMap` | 通过 HTTP 流式连接远程 server |

认证类请求头（`authorization`、`proxy-authorization`、`x-api-key`、`x-auth-token`、`cookie`、`set-cookie`）在持久化到 `.mcp` 与 `list_mcps()` 返回时都会被替换为 `[REDACTED]`，不会泄露密钥。

连接细节见 [tool/mcp](../tool/mcp)。

## 边界说明

- **传输**：`McpTransportConfig` 支持 Stdio / SSE / StreamableHttp。
- **Gateway 服务端**：将工作空间工具作为 MCP server 暴露的能力为「计划中」，尚未实现。
