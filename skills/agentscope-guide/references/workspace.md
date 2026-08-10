# 参考:工作空间与沙箱(`agent_scope_workspace` / `agent_scope_sandbox`)

> 详细 API 参考:`LocalWorkspace`、`WorkspaceBase`、`WorkspaceManager`、`Skill` 管理、MCP 配置,以及 `LocalSandboxSession` 沙箱执行与路径安全。

## 1. `WorkspaceBase` trait

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

生命周期:`创建 → initialize() → Alive → close() → Closed`,支持中途 `reset()`。

## 2. `LocalWorkspace`

`LocalWorkspace::new(config)` 配置项:

| 字段 | 说明 |
|------|------|
| `workdir` | 工作空间根目录 |
| `workspace_id` | 可选 ID,不提供时自动生成 |
| `default_mcps` | 初始化时自动注册的 MCP 客户端 |
| `skill_paths` | 初始化时加载的技能文件路径 |
| `instructions` | 可选的 Agent 指令文本 |

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
assert!(ws.is_alive());

let tools = ws.list_tools().await?; // Bash、Read、Write、Edit、Glob、Grep
ws.close().await?;
```

内置工具:

| 工具 | 功能 |
|------|------|
| Bash | 执行 Shell 命令(超时 + 输出限制) |
| Read | 读文件 |
| Write | 写文件 |
| Edit | 精确字符串替换 |
| Glob | 文件模式匹配 |
| Grep | 内容搜索 |

## 3. `WorkspaceManager`

多工作空间管理:

```rust
use agent_scope_workspace::WorkspaceManager;

let mut manager = WorkspaceManager::new();
let ws_id = manager.create_workspace(config).await?;
let ws = manager.get_workspace(&ws_id)?;
```

## 4. Skill 管理(workspace 侧)

`Skill` 表示一个技能文件(带 frontmatter 的 Markdown):

| 字段 | 说明 |
|------|------|
| `name` | 唯一标识(文件名或 frontmatter) |
| `description` | 一行描述 |
| `content` | Markdown 正文 |

`SkillManager` 管理生命周期:`load(path)` / `load_dir(dir)` / `unload(name)` / `list()` / `index()`(生成 `SkillsIndex`)。

```rust
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

let config = LocalWorkspaceConfig {
    workdir: "/tmp/ws".into(),
    skill_paths: vec!["/path/to/skills".into()],
    ..Default::default()
};
let mut ws = LocalWorkspace::new(config);
ws.initialize().await?;
// 技能已自动加载,可通过 workspace 的 skill_manager 访问
```

## 5. Skill 工具化(tool 侧)

`agent_scope_tool` 提供加载与工具化转换:

```rust
use agent_scope_tool::{LocalSkillLoader, SkillLoader, SkillViewer};

let loader = LocalSkillLoader::new("/path/to/skills");
let skill = loader.load("weather-reporter.md").await?;
let skills = loader.load_dir("/path/to/skills").await?;
let view = SkillViewer::format_skills(&skills);
```

`SKILL.md` 格式:

```markdown
---
name: weather-reporter
description: Report weather for a given city
---

You are a weather reporter. When asked about weather:
1. Use the `get_weather` tool if available
2. Format the response in a friendly way
```

`ToolKit` 侧:`add_skill_dir(path)` / `add_skill(skill)` / `add_skill_loader(loader)`,`ToolKit::new()` 默认自动带 `SkillViewer`。

## 6. MCP 配置与运行时(`agent_scope_mcp`)

MCP 在 AgentScope Rust 中分成两层:

- `agent_scope_workspace`:只保存和管理持久化配置(`.mcp`、`McpClientConfig`、`add_mcp` / `remove_mcp` / `list_mcps`)。
- `agent_scope_mcp`:负责运行时连接、工具发现和 `Tool` 适配(`McpClient`、`McpTool`、`McpExt`)。

因此应用代码需要连接 MCP 服务器时,除 workspace 外还要显式依赖 `agent_scope_mcp`:

```toml
agent_scope_workspace = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_mcp = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
```

### 6.1 配置注册(workspace 侧)

`McpClientConfig` 描述一个外部 MCP 服务器,持久化到 `<workdir>/.mcp`(JSON 数组):

```rust
use agent_scope_workspace::mcp::{McpClientConfig, McpTransportConfig};

let mcp = McpClientConfig {
    name: "my-server".into(),
    transport: McpTransportConfig::Stdio {
        command: "node".into(),
        args: vec!["server.js".into()],
    },
    is_stateful: true,
};
// 或 McpTransportConfig::Sse { url, headers }(连接时映射到 StreamableHttp)
// 或 McpTransportConfig::StreamableHttp { url, headers }
```

通过 `WorkspaceBase` 的 `add_mcp` / `remove_mcp` / `list_mcps` 管理。`McpRegistry` 只负责配置的加载/保存/索引,`authorization`、`x-api-key` 等敏感 header 在持久化与返回时**始终脱敏为 `[REDACTED]`**。

### 6.2 运行时连接(`agent_scope_mcp` crate)

**配置在 workspace,运行时在 `agent_scope_mcp`**(依赖 `rmcp` 官方 SDK)。连接生命周期通过 `McpExt` 扩展 trait 挂到 `LocalWorkspace` 上:

```rust
use agent_scope_mcp::McpExt;
use agent_scope_workspace::mcp::{McpClientConfig, McpTransportConfig};
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
    workdir: "/tmp/my-workspace".into(),
    workspace_id: None,
    default_mcps: vec![],
    skill_paths: vec![],
    instructions: None,
});
ws.initialize().await?;

ws.add_mcp(McpClientConfig {
    name: "my-server".into(),
    transport: McpTransportConfig::Stdio {
        command: "node".into(),
        args: vec!["server.js".into()],
    },
    is_stateful: true,
}).await?;

let tools = ws.connect_mcp("my-server").await?;    // 连接 + 发现工具 → Vec<Arc<dyn Tool>>
let cached = ws.get_mcp_tools("my-server").await?; // 缓存列表,连接生命周期内可查
ws.disconnect_mcp("my-server").await?;             // 关闭连接,释放子进程/套接字
```

- 工具名带前缀:`"{mcp_name}/{tool_name}"`(如 `excalidraw/create_element`)。
- 每个远程工具被适配为 `agent_scope_mcp::McpTool`,实现统一 `Tool` trait,`read_only` 从远程 `annotations.read_only_hint` 传递。
- 所有工具共享同一条活跃连接(`McpClient` 内部 `tokio::sync::Mutex` 串行化调用)。
- workspace `close()` / `reset()` 自动断开所有 MCP 连接(FR-010)。
- 依赖:需在 `Cargo.toml` 加入 `agent_scope_mcp`(非 workspace 默认依赖)。

### 6.3 把 MCP 工具交给 Agent

`connect_mcp()` 返回的是 `Vec<Arc<dyn Tool>>`。这些工具与本地 `FunctionTool` 走同一个 `Tool` trait,可以直接调用,也可以注册进 `ToolKit` 后交给 `ReActAgent` 调度:

```rust
use std::sync::Arc;

use agent_scope_tool::{Tool, ToolError, ToolExecOutput, ToolKit};
use serde_json::Value;

struct SharedTool(Arc<dyn Tool>);

#[async_trait::async_trait]
impl Tool for SharedTool {
    fn name(&self) -> &str { self.0.name() }
    fn description(&self) -> &str { self.0.description() }
    fn input_schema(&self) -> Value { self.0.input_schema() }
    fn is_concurrency_safe(&self) -> bool { self.0.is_concurrency_safe() }
    fn is_read_only(&self) -> bool { self.0.is_read_only() }

    async fn call(&self, input: Value) -> Result<ToolExecOutput, ToolError> {
        self.0.call(input).await
    }
}

let mcp_tools = ws.connect_mcp("my-server").await?;
let mut toolkit = ToolKit::new();
for tool in mcp_tools {
    toolkit.register(SharedTool(tool));
}
// 后续把 toolkit 放进 AgentConfig::builder().toolkit(toolkit)
```

若只想手动调试单个远程工具,按带前缀的名字查找后调用:

```rust
use serde_json::json;

let create = cached
    .iter()
    .find(|tool| tool.name() == "my-server/create_element")
    .expect("remote tool exists");
let output = create.call(json!({ "type": "rectangle", "x": 100, "y": 100 })).await?;
```

### 6.4 真实 stdio 示例

真实示例:`crates/agent_scope_mcp/examples/mcp_excalidraw_debug.rs` 对 `mcp-excalidraw-server` 做完整 stdio 往返:注册 `.mcp`、派生 Node 子进程、发现远程工具、调用 `clear_canvas` / `create_element` / `describe_scene` / `query_elements`,最后 `disconnect_mcp()` 释放子进程。

运行前先安装服务器并确保命令在 `PATH` 中:

```bash
npm i -g mcp-excalidraw-server
cargo run -p agent_scope_mcp --example mcp_excalidraw_debug
```

如果不想全局安装,也可以把 `McpTransportConfig::Stdio` 改成 `command: "npx"`、`args: vec!["-y".into(), "mcp-excalidraw-server".into()]`。

### 6.5 常见 MCP 坑

| 现象 | 检查点 |
|------|--------|
| `McpNotFound` | 先 `add_mcp()` 或在 `default_mcps` 中注册;`connect_mcp(name)` 的名字必须与配置一致 |
| 派生失败 / `McpConnectionError` | stdio `command` 不在 `PATH`、`args` 错误、服务器启动后没有按 MCP 协议响应 |
| 找不到远程工具 | 工具名必须带 MCP 前缀:`"{mcp_name}/{tool_name}"` |
| Agent 不调用 MCP 工具 | 连接后要把返回的 `Arc<dyn Tool>` 注册进 `ToolKit`,并在 system prompt 中说明可用工具 |
| HTTP header 凭据丢失 | 持久化/列表会脱敏敏感 header;需要运行时真实凭据时避免把只含 `[REDACTED]` 的展示值重新作为连接配置 |
| 长驻进程残留 | 结束时调用 `disconnect_mcp()` / `close()` / `reset()`;workspace 会自动断开活跃 MCP 连接 |

## 7. 沙箱:`SandboxSession` trait

```rust
#[async_trait]
pub trait SandboxSession: Send + Sync {
    fn session_id(&self) -> &str;
    fn state(&self) -> SandboxState;  // Created → Ready → Closing → Closed
    fn policy(&self) -> &SandboxPolicy;

    async fn initialize(&mut self) -> Result<(), SandboxError>;
    async fn execute(&mut self, request: ExecutionRequest) -> Result<ExecutionResult, SandboxError>;
    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, SandboxError>;
    async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), SandboxError>;
    async fn delete_file(&mut self, path: &str) -> Result<(), SandboxError>;
    async fn get_capabilities(&self) -> CapabilityReport;
    async fn close(&mut self) -> Result<(), SandboxError>;
}
```

## 8. `SandboxPolicy` / `ExecutionRequest` / `ExecutionResult`

```rust
pub struct SandboxPolicy {
    pub allow_unrestricted_filesystem: bool,
    pub command_timeout_seconds: u32,   // 默认 30
    pub max_output_bytes: usize,        // 默认 100KB
    pub allow_network: bool,            // 当前不支持,显式报告
}

pub struct ExecutionRequest {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<String>,
}

pub struct ExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}
```

## 9. `LocalSandboxSession` 使用

```rust
use agent_scope_sandbox::{LocalSandboxSession, LocalSandboxConfig, SandboxSession, ExecutionRequest};

let config = LocalSandboxConfig::default();
let mut session = LocalSandboxSession::new(config)?;
session.initialize().await?;

// 文件操作(带路径安全检查)
session.write_file("notes/result.txt", b"hello").await?;
let data = session.read_file("notes/result.txt").await?;

// 命令执行(带超时与输出限制)
let result = session.execute(ExecutionRequest {
    command: "echo".into(),
    args: vec!["hello world".into()],
    env: Default::default(),
    working_dir: None,
}).await?;

// 显式检查能力
let caps = session.get_capabilities().await;
assert!(!caps.network_isolation); // 本地参考实现无网络隔离

session.close().await?;
```

## 10. 路径安全

`LocalSandboxSession` 对路径做规范化检查:

- 拒绝绝对路径。
- 拒绝含 `..` 的路径遍历(`SandboxError::PathTraversal`)。
- 拒绝指向工作目录外的符号链接。
- 所有文件操作限定在 root_dir 范围内。

```rust
// ✅ 允许
session.read_file("notes/data.txt").await

// ❌ 拒绝(路径遍历)
session.read_file("../../etc/passwd").await  // → SandboxError::PathTraversal
```

## 11. `CapabilityReport`(伪兼容对策)

```rust
pub struct CapabilityReport {
    pub hard_isolation: bool,        // false — 无硬隔离
    pub network_isolation: bool,     // false
    pub resource_limits: bool,       // false
    pub filesystem_isolation: bool,  // true — 路径遍历防护
    pub sandbox_type: String,        // "local-reference"
}
```

原则:`LocalSandboxSession` 不假装拥有不能提供的能力(`network_isolation: false` 明确告知软隔离)。

## 12. 挂载点

```rust
use agent_scope_sandbox::SandboxMount;

let config = LocalSandboxConfig {
    mounts: vec![
        SandboxMount::read_only("/data/public"),
        SandboxMount::read_write("/data/agent-scratch"),
    ],
    ..Default::default()
};
```

## 13. 错误

| 错误 | 触发条件 |
|------|----------|
| `WorkspaceError::IoError` | 文件操作失败 |
| `WorkspaceError::InvalidSkill` | 技能文件格式错误 |
| `WorkspaceError::AlreadyClosed` | 对已关闭 workspace 操作 |
| `WorkspaceError::McpNotFound` | 无该名称的持久化 MCP 配置 |
| `WorkspaceError::McpConnectionError` | MCP 传输层失败(派生/连接/断开) |
| `WorkspaceError::McpCallError` | MCP 调用期间协议/对端错误 |
| `SandboxError::PathTraversal` | 路径越界或符号链接逃逸 |
| `SandboxError::Timeout` | 命令执行超时 |
| `SandboxError::OutputLimitExceeded` | 输出超 `max_output_bytes` |
| `SandboxError::PermissionDenied` | 违反 `SandboxPolicy` |
| `SandboxError::SessionClosed` | 对已关闭 session 操作 |

## 14. 不支持的能力

- Workspace:`GatewayError` 为占位,沙箱↔工作空间网关集成待完善;远程工作空间后端不在范围。
- Sandbox:硬隔离(Docker/VM)、网络隔离、资源限制(CPU/内存)均不支持并显式报告;`LocalSandboxSession` 是参考实现,非生产级沙箱。
- Skill:远程技能加载(URL 获取)、技能依赖、热重载均未实现。
