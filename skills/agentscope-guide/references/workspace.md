# 参考:工作空间与沙箱(`agent_scope_workspace` / `agent_scope_sandbox`)

> 详细 API 参考:`LocalWorkspace`、`WorkspaceBase`、`WorkspaceManager`、内置 workspace 工具、Skill 管理、MCP 配置,以及 `LocalSandboxSession` / feature-gated `MicrosandboxSession` 沙箱执行与路径安全。

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

let tools = ws.list_tools().await?; // Bash、Read、Write、Edit、Glob、Grep 等
ws.close().await?;
```

内置工具:

| 工具 | 功能 |
|------|------|
| `Bash` / `bash` | 执行 Shell 命令;legacy timeout 用毫秒,pi `bash` timeout 用秒 |
| `PowerShell` / `powershell` | Windows PowerShell 命令;非 Windows 返回 unsupported |
| `Read` / `read` | 读 UTF-8 文件;记录 read-before-modify 状态 |
| `Write` / `write` | 写文件;覆盖已有文件前要求已读 |
| `Edit` / `edit` | 精确字符串替换;pi `edit` 支持批量 edits |
| `Glob` | 文件模式匹配;结果按 mtime 倒序 |
| `Grep` / `grep` | 内容搜索;支持 regex、上下文、glob/type filter |
| `find` | pi-compatible 文件发现,基于 workspace backend listing |
| `ls` | pi-compatible 目录列举 |
| `ResetTools` | 切换 session 内激活的工具组 |
| `Skill` | 读取可见 skill 内容 |

这些工具都通过 `BuiltInToolContext` 访问 `WorkspaceBackend`,不会直接绕过 workspace 的路径 containment。`Read`/`Write`/`Edit` 共用 `WorkspaceToolSession` 里的 read-state,用于防止未读先改。

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
    workspace_id: None,
    default_mcps: vec![],
    skill_paths: vec!["/path/to/skills".into()],
    instructions: None,
};
let mut ws = LocalWorkspace::new(config);
ws.initialize().await?;
// 技能会在 initialize() 时 seed 到 workspace 的 skills 目录与索引。
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
    fn state(&self) -> SandboxState;  // Created → Ready → Closing → Closed / Failed
    fn policy(&self) -> &SandboxPolicy;

    async fn initialize(&mut self) -> Result<(), SandboxError>;
    async fn execute(&mut self, request: ExecutionRequest) -> Result<ExecutionResult, SandboxError>;
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, SandboxError>;
    async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), SandboxError>;
    async fn delete_path(&mut self, path: &str) -> Result<(), SandboxError>;
    async fn is_dir(&self, path: &str) -> Result<bool, SandboxError>;
    async fn path_exists(&self, path: &str) -> Result<bool, SandboxError>;
    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, SandboxError>;
    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, SandboxError>;
    async fn history(&self) -> Result<Vec<ExecutionRecord>, SandboxError>;
    async fn capability_report(&self) -> Result<CapabilityReport, SandboxError>;
    async fn close(&mut self) -> Result<(), SandboxError>;
    async fn cleanup(&mut self) -> Result<(), SandboxError>;
}
```

## 8. `SandboxPolicy` / `ExecutionRequest` / `ExecutionResult`

```rust
pub struct SandboxPolicy {
    pub default_timeout: Duration,      // 默认 30s
    pub max_timeout: Duration,          // 默认 300s
    pub max_output_bytes: usize,        // 默认 1MiB
    pub network: NetworkPolicy,         // Disabled / LoopbackOnly / Allowlist / Unrestricted
    pub writable_roots: Vec<PathBuf>,
    pub readonly_roots: Vec<PathBuf>,
    pub keep_on_close: bool,
    pub cpu_limit: Option<CpuLimit>,
    pub memory_limit_bytes: Option<u64>,
    pub process_limit: Option<u32>,
}

pub struct ExecutionRequest {
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub timeout: Option<Duration>,
    pub stdin: Option<Vec<u8>>,
}

pub struct ExecutionResult {
    pub execution_id: String,
    pub status: ExecutionStatus,
    pub exit_code: Option<i32>,
    pub stdout: OutputSummary,
    pub stderr: OutputSummary,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration: Duration,
    pub resource_hits: Vec<ResourceLimitHit>,
}
```

`ExecutionRequest::new(["echo", "hello"])` 可快速构造 argv。`stdout`/`stderr` 是 `OutputSummary`,小输出内联,超限时可通过 `full_ref` 指向落盘文件。

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
let result = session.execute(ExecutionRequest::new(["echo", "hello world"])).await?;

// 显式检查能力
let caps = session.capability_report().await?;
assert_eq!(caps.backend_name, "local-process");

session.close().await?;
```

`LocalSandboxSession` 是本地参考实现:Rust 文件 API 有路径 containment,命令执行是 host child process,不是 chroot/container/VM 级隔离。选择它时必须读 `capability_report()`。

## 10. `MicrosandboxSession`(feature-gated)

启用 `agent_scope_sandbox` 的 `microsandbox` feature 后可使用 `MicrosandboxSession`:

```rust
#[cfg(feature = "microsandbox")]
use agent_scope_sandbox::{MicrosandboxConfig, MicrosandboxSession, SandboxSession};

#[cfg(feature = "microsandbox")]
let mut session = MicrosandboxSession::new(MicrosandboxConfig::default())?;
```

要点:

- 需要独立安装/可用的 microsandbox runtime。
- `CapabilityReport::microsandbox()` 为 L4,支持硬件隔离、guest filesystem、memory limit 等能力。
- `NetworkPolicy::Disabled` / `Unrestricted` 可映射;`LoopbackOnly` / `Allowlist` 当前会返回 `UnsupportedFeature`,不会放宽成 unrestricted。
- `cpu_limit.cpu_shares` 和 `process_limit` 当前也会拒绝,因为 SDK 暂无等价稳定映射。
- `MSB_API_KEY` / `MSB_API_URL` 不会自动选择 cloud execution;云执行必须由未来显式配置选择。

## 11. 路径安全

`LocalSandboxSession` 对路径做规范化检查:

- 拒绝绝对路径。
- 拒绝含 `..` 的路径遍历(通常返回 `SandboxError::PermissionDenied`)。
- 拒绝指向工作目录外的符号链接。
- 所有文件操作限定在 root_dir 范围内。

```rust
// ✅ 允许
session.read_file("notes/data.txt").await

// ❌ 拒绝(路径遍历)
session.read_file("../../etc/passwd").await  // → SandboxError::PermissionDenied
```

## 12. `CapabilityReport`

```rust
pub struct CapabilityReport {
    pub backend_name: String,                  // "local-process" / "microsandbox"
    pub compatibility_level: CompatibilityLevel, // L1..L4
    pub supported: Vec<SandboxCapability>,
    pub unsupported: Vec<UnsupportedCapability>,
    pub known_deviations: Vec<String>,
}
```

原则:后端不假装拥有不能提供的能力。`local-process` 会明确列出 network/cpu/memory/process/hard isolation 等 unsupported;`microsandbox` 会列出当前 SDK 无法精确映射的策略。

## 13. 挂载点

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

## 14. 错误

| 错误 | 触发条件 |
|------|----------|
| `WorkspaceError::IoError` | 文件操作失败 |
| `WorkspaceError::InvalidSkill` | 技能文件格式错误 |
| `WorkspaceError::AlreadyClosed` | 对已关闭 workspace 操作 |
| `WorkspaceError::McpNotFound` | 无该名称的持久化 MCP 配置 |
| `WorkspaceError::McpConnectionError` | MCP 传输层失败(派生/连接/断开) |
| `WorkspaceError::McpCallError` | MCP 调用期间协议/对端错误 |
| `SandboxError::ValidationError` | 配置或请求非法 |
| `SandboxError::LifecycleError` | 在错误生命周期状态调用操作 |
| `SandboxError::PermissionDenied` | 路径越界、符号链接逃逸或违反策略 |
| `SandboxError::TimeoutError` | 命令执行超时 |
| `SandboxError::UnsupportedFeature` | 当前 backend 无法精确满足请求的 policy/capability |
| `SandboxError::SandboxUnavailable` | microsandbox runtime 或 backend 不可用 |
| `SandboxError::IoError` | 文件/进程 I/O 失败 |
| `SandboxError::InternalError` | 未预期内部错误 |

输出超过 `max_output_bytes` 不一定是错误;`ExecutionResult.resource_hits` 会包含 `ResourceLimitHit::OutputTruncated`,完整输出可通过 `OutputSummary.full_ref` 读取。

## 15. 当前边界

- Workspace:`GatewayError` 为占位,沙箱↔工作空间网关集成待完善;远程工作空间后端不在范围。
- Sandbox:`LocalSandboxSession` 是参考实现,非生产级沙箱;需要硬隔离时启用并配置 `MicrosandboxSession`,且仍需检查 `CapabilityReport` 与 `UnsupportedFeature`。
- Skill:远程技能加载(URL 获取)、技能依赖、热重载均未实现。
