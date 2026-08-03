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

## 6. MCP 配置

```rust
use agent_scope_workspace::{McpClientConfig, McpTransportConfig};

let mcp = McpClientConfig {
    name: "my-server".into(),
    transport: McpTransportConfig::Stdio {
        command: "node".into(),
        args: vec!["server.js".into()],
    },
};
// 或 McpTransportConfig::Sse { url: String }
```

`McpRegistry` 管理 MCP 客户端生命周期(启动、发现 tools、关闭)。

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
| `WorkspaceError::McpError` | MCP 客户端启动/通信失败 |
| `SandboxError::PathTraversal` | 路径越界或符号链接逃逸 |
| `SandboxError::Timeout` | 命令执行超时 |
| `SandboxError::OutputLimitExceeded` | 输出超 `max_output_bytes` |
| `SandboxError::PermissionDenied` | 违反 `SandboxPolicy` |
| `SandboxError::SessionClosed` | 对已关闭 session 操作 |

## 14. 不支持的能力

- Workspace:`GatewayError` 为占位,沙箱↔工作空间网关集成待完善;远程工作空间后端不在范围。
- Sandbox:硬隔离(Docker/VM)、网络隔离、资源限制(CPU/内存)均不支持并显式报告;`LocalSandboxSession` 是参考实现,非生产级沙箱。
- Skill:远程技能加载(URL 获取)、技能依赖、热重载均未实现。
