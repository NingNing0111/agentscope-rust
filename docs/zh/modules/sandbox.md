# 沙箱 / Sandbox

> 一句话定位：`agent_scope_sandbox` 提供 Agent 代码执行的安全隔离——`LocalSandboxSession` 实现路径遍历防护、命令执行超时、输出限制和显式能力报告，拒绝伪兼容。

## 1. 模块概述 (Overview)

本模块实现 `SandboxSession` trait 的本地参考实现，严格遵循宪法规约第五条（禁止伪兼容）：

| 组件 | 职责 |
|------|------|
| `SandboxSession` trait | 沙箱生命周期接口：`initialize()`、`execute()`、`read_file()`、`write_file()`、`close()` |
| `LocalSandboxSession` | 本地参考实现，提供文件隔离和可控命令执行 |
| `CapabilityReport` | **显式报告不支持的能力**（如网络隔离、资源限制），不假装支持 |
| `SandboxPolicy` | 执行策略：允许/拒绝的路径、命令白名单、超时/输出限制 |
| `SandboxMount` | 只读/读写挂载点配置 |
| 路径安全 | `normpath` 规范化、符号链接逃逸检测、工作目录限定 |

**适用场景**：Agent 需要执行任意代码或 Shell 命令时；需要限制 Agent 对文件系统的访问时；需要记录命令执行历史时。

**前置阅读**：[工作空间](./workspace.md)、[Agent 系统](./agent.md)

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 `SandboxSession` trait

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

### 2.2 `SandboxState` 生命周期

```
Created → initialize() → Ready → close() → Closing → Closed
                                ↘ 执行失败 → Failed
```

### 2.3 `SandboxPolicy`

| 字段 | 说明 |
|------|------|
| `allow_unrestricted_filesystem` | 是否允许无限制文件访问 |
| `command_timeout_seconds` | 命令执行超时（默认 30s） |
| `max_output_bytes` | 最大输出字节数（默认 100KB） |
| `allow_network` | 是否允许网络（**当前不支持**，显式报告） |

### 2.4 `ExecutionRequest` 与 `ExecutionResult`

```rust
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

### 2.5 `CapabilityReport` — 伪兼容对策

```rust
pub struct CapabilityReport {
    pub hard_isolation: bool,  // false — 明确告知无硬隔离
    pub network_isolation: bool, // false
    pub resource_limits: bool,  // false
    pub filesystem_isolation: bool, // true — 路径遍历防护
    pub sandbox_type: String,  // "local-reference"
}
```

**关键设计原则**：`LocalSandboxSession` 不会假装拥有它不能提供的能力。`network_isolation: false` 明确告诉调用方这是软隔离。

### 2.6 路径安全

- `normpath()` — 消除 `.`、`..`，解析相对路径
- 符号链接逃逸检测 — 拒绝指向工作目录外的符号链接
- 所有文件操作限定在 root_dir 范围内

## 3. 快速示例 (Quick Example)

```rust
use agent_scope_sandbox::{LocalSandboxSession, LocalSandboxConfig, SandboxSession};

let config = LocalSandboxConfig::default();
let mut session = LocalSandboxSession::new(config)?;

session.initialize().await?;

// 文件操作（带路径安全检查）
session.write_file("notes/result.txt", b"hello").await?;
let data = session.read_file("notes/result.txt").await?;
assert_eq!(data, b"hello");

// 命令执行（带超时和输出限制）
use agent_scope_sandbox::ExecutionRequest;
let result = session.execute(ExecutionRequest {
    command: "echo".into(),
    args: vec!["hello world".into()],
    env: Default::default(),
    working_dir: None,
}).await?;
assert_eq!(result.stdout.trim(), "hello world");

// 显式检查能力
let caps = session.get_capabilities().await;
assert!(!caps.network_isolation); // 本地参考实现无网络隔离

session.close().await?;
```

## 4. 关键用法模式 (Usage Patterns)

### 4.1 路径安全验证

`LocalSandboxSession` 内部对所有路径进行规范化检查：
- 拒绝绝对路径
- 拒绝包含 `..` 的路径遍历
- 拒绝指向工作目录外的符号链接

```rust
// ✅ 允许
session.read_file("notes/data.txt").await

// ❌ 拒绝（路径遍历）
session.read_file("../../etc/passwd").await  // → SandboxError::PathTraversal
```

### 4.2 命令执行与超时

```rust
let result = session.execute(ExecutionRequest {
    command: "sleep".into(),
    args: vec!["60".into()],  // 超过 policy.command_timeout_seconds
    ..Default::default()
}).await;
// → 命令被超时终止，返回 SandboxError::Timeout
```

### 4.3 执行历史

`LocalSandboxSession` 内部维护 `Vec<ExecutionRecord>`，记录每次命令执行的时间、参数、结果。

### 4.4 挂载点

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

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误 | 原因 |
|------|------|
| `SandboxError::PathTraversal` | 路径越界或符号链接逃逸 |
| `SandboxError::Timeout` | 命令执行超时 |
| `SandboxError::OutputLimitExceeded` | 输出超过 `max_output_bytes` |
| `SandboxError::PermissionDenied` | 违反 `SandboxPolicy` |
| `SandboxError::SessionClosed` | 对已关闭 session 操作 |
| `SandboxError::IoError` | 底层文件系统错误 |

### 明确不支持的能力

`LocalSandboxSession` 通过 `CapabilityReport` **显式报告**以下不支持的功能：

| 能力 | 状态 | 说明 |
|------|------|------|
| 硬隔离 (hard_isolation) | ❌ | 进程级而非容器/VM 级 |
| 网络隔离 | ❌ | 无网络命名空间隔离 |
| 资源限制 (CPU/内存) | ❌ | 无 cgroup 级限制 |
| 文件系统隔离 | ✅ | 路径规范化 + 符号链接检测 |

这遵循宪法规约第五条——**不伪兼容，不静默降级**。

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L2**（核心沙箱行为参考实现）
- **权威来源**: `specs/017-sandbox-feature/spec.md`
- **已知偏差**:
  - `LocalSandboxSession` 是参考实现，非生产级沙箱
  - 不支持硬隔离（Docker/VM），且显式报告
  - Python 侧可能采用 Docker 等外部沙箱

## 7. 相关模块 (See Also)

- [工作空间](./workspace.md) — workspace 提供文件/工具抽象，sandbox 提供隔离
- [Agent 系统](./agent.md) — Agent 通过 sandbox 安全执行代码
- [工具系统](./tool.md) — 沙箱内的 Bash/Edit/Write 以 Tool 形式暴露
