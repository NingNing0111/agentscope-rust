# 工作空间 / Workspace

> 一句话定位：`agent_scope_workspace` 为每个 Agent 提供独立的文件系统沙箱——通过 `WorkspaceBase` trait 抽象文件 I/O、Bash 执行、MCP 客户端配置、技能管理和上下文卸载能力，`LocalWorkspace` 提供即开即用的本地实现。

## 1. 模块概述 (Overview)

| 组件 | 职责 |
|------|------|
| `WorkspaceBase` | 工作空间生命周期抽象：`initialize()`、`close()`、`reset()` |
| `LocalWorkspace` | 本地文件系统实现，提供 Read/Write/Edit/Glob/Grep/Bash 等内置工具 |
| `WorkspaceBackend` | 文件后端 trait：`read_file`、`write_file`、`delete_file`、`glob`、`grep` 等 |
| `WorkspaceManager` | 多工作空间管理器，管理 session 与 workspace 的映射 |
| `McpClientConfig` / `McpRegistry` | MCP 客户端配置（Stdio、SSE 传输），注册与发现 |
| `SkillManager` / `Skill` / `SkillsIndex` | 技能文件管理、加载、索引 |
| Offload | 上下文卸载——将大文件内容移到工作空间，减轻上下文压力 |

**适用场景**：Agent 需要读写文件时；运行基础设施工具（Git、Docker）时；集成 MCP 服务时；加载 Skill 文件时。

**前置阅读**：[Agent 系统](./agent.md)、[工具系统](./tool.md)、[技能系统](./skill.md)、[沙箱](./sandbox.md)

## 2. 核心概念与主要公开类型 (Core Concepts)

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
    // ... 文件操作、Bash 执行、MCP/Skill 管理方法
}
```

生命周期：`创建 → initialize() → Alive → close() → Closed`，支持中途 `reset()`。

### 2.2 `LocalWorkspace`

`LocalWorkspace::new(config)` 配置项：

| 字段 | 说明 |
|------|------|
| `workdir` | 工作空间根目录 |
| `workspace_id` | 可选 ID，不提供时自动生成 |
| `default_mcps` | 初始化时自动注册的 MCP 客户端列表 |
| `skill_paths` | 初始化时加载的技能文件路径 |
| `instructions` | 可选的 Agent 指令文本 |

### 2.3 内置工具 (Built-in Tools)

`LocalWorkspace.list_tools()` 返回的工具：

| 工具 | 功能 |
|------|------|
| Bash | 执行 Shell 命令（带超时和输出限制） |
| Read | 读取文件内容 |
| Write | 写入文件 |
| Edit | 精确字符串替换 |
| Glob | 文件模式匹配 |
| Grep | 内容搜索 |

### 2.4 MCP 集成

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

`McpRegistry` 是**纯配置**注册表（加载/保存/索引）；运行时连接、工具发现与工具调用适配器在 `agent_scope_mcp` crate（`McpClient` / `McpTool` / `McpExt`）。详见 [MCP 集成](./mcp.md)。

### 2.5 技能管理

`Skill` 表示一个技能文件：
- 从 `.md` 文件中解析 frontmatter（name、description）
- 正文为 Markdown 指令
- `SkillManager` 维护 `SkillsIndex`，支持加载、列表、搜索

### 2.6 上下文卸载 (Offload)

将大段文本移到工作空间文件中，上下文中只保留路径引用，减轻 token 压力。

## 3. 快速示例 (Quick Example)

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

// 使用内置工具
let tools = ws.list_tools().await?;
// tools 包含 Bash、Read、Write、Edit、Glob、Grep

ws.close().await?;
```

## 4. 关键用法模式 (Usage Patterns)

### 4.1 在 Agent 中使用 Workspace

```rust
use std::sync::Arc;
let ws = Arc::new(tokio::sync::Mutex::new(LocalWorkspace::new(config)));
ws.lock().await.initialize().await?;

let agent = ReActAgent::new(
    agent_config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![], // 注入 workspace-aware middleware
)?;
```

### 4.2 MCP 客户端注册

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

### 4.3 Bash 执行

Workspace 提供的 Bash 工具包含安全控制：
- 命令超时
- 输出大小限制
- 工作目录限定为 workspace 范围

### 4.4 多工作空间管理

```rust
use agent_scope_workspace::WorkspaceManager;
let mut manager = WorkspaceManager::new();
let ws_id = manager.create_workspace(config).await?;
let ws = manager.get_workspace(&ws_id)?;
```

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误 | 原因 |
|------|------|
| `WorkspaceError::IoError` | 文件操作失败 |
| `WorkspaceError::InvalidSkill` | 技能文件格式错误 |
| `WorkspaceError::GatewayError` | 沙箱网关错误（占位，待集成） |
| `WorkspaceError::AlreadyClosed` | 对已关闭 workspace 操作 |
| `WorkspaceError::McpNotFound` | 不存在该名称的持久化 MCP 配置 |
| `WorkspaceError::McpConnectionError` | MCP 传输层失败（派生/连接/断开） |
| `WorkspaceError::McpCallError` | MCP 调用期间协议/对端错误 |

**不支持**：
- `GatewayError` 当前为占位，沙箱←→工作空间网关集成待完善。
- 远程工作空间后端不在当前范围内。
- 工作空间内的网络隔离由 `agent_scope_sandbox` 提供，不在 workspace 层。

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L2**（核心 workspace 行为）
- **权威来源**: `specs/012-workspace-management/spec.md`
- **已知偏差**: `McpRegistry` 和 `SkillManager` 是 Rust 侧抽象，不完全对应 Python 侧

## 7. 相关模块 (See Also)

- [沙箱 / sandbox](./sandbox.md) — 工作空间的执行隔离层
- [技能系统 / skill](./skill.md) — 技能在 workspace 中的管理
- [工具系统](./tool.md) — 工作空间内置工具通过 Tool 接口暴露
- [Agent 系统](./agent.md) — Agent 如何使用 workspace
