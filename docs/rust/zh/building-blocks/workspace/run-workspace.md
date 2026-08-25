---
title: "运行工作空间"
description: "在受控环境执行命令与文件操作"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用。`WorkspaceBackend` 命令执行、内置工具注入与 sandbox workspace adapter 均可用；`MicrosandboxSession` 需要启用 `microsandbox` feature，并依赖本机 microsandbox runtime。
</Note>

工作空间通过 `WorkspaceBackend` 在受控环境执行命令与文件操作。`LocalWorkspace` 的本地后端把操作限定在 workdir 内，路径越界会被拒绝（`ContainedBackend` 做边界防护）。

## 执行命令

```rust
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
    workdir: "/tmp/my-ws".into(),
    ..Default::default()
});
ws.initialize().await?;

let backend = ws.get_backend()?;
let out = backend.exec_shell(&["ls", "-la"], "/tmp/my-ws", None).await?;
println!("{}", String::from_utf8_lossy(&out.stdout));
```

### exec_shell 签名

```rust
async fn exec_shell(
    &self,
    cmd: &[&str],          // argv 数组，如 &["ls", "-la"]
    cwd: &str,             // 执行目录（必须在 workdir 内）
    timeout_secs: Option<f64>, // 可选超时秒数；None 表示不限时
) -> Result<ExecOutput, WorkspaceError>
```

执行时子进程环境变量会被清空，仅注入 `PATH`、`HOME`（指向 cwd）与 `TMPDIR`，避免把宿主机密钥等泄露给被执行的命令。

### ExecOutput

| 字段 | 类型 | 说明 |
|------|------|------|
| `stdout` | `Vec<u8>` | 标准输出，单次上限 1 MiB，超限截断 |
| `stderr` | `Vec<u8>` | 标准错误 |
| `exit_code` | `i32` | 退出码；`ok()` 返回 `exit_code == 0` |

### 其它后端方法

| 方法 | 说明 |
|------|------|
| `read_file(path)` | 读取文件为字节（上限 10 MiB） |
| `write_file(path, data)` | 写入文件，自动创建父目录 |
| `list_dir(path, recursive)` | 列出目录条目，返回完整路径 |
| `delete_path(path)` | 删除文件 / 递归删除目录，幂等 |
| `is_dir(path)` / `file_exists(path)` | 目录 / 存在性判断 |
| `stat_mtime(path)` | 修改时间（Unix 秒，不存在返回 `None`） |
| `join_path` / `basename` / `dirname` / `normpath` / `is_absolute` | 纯字符串路径运算 |

## 内置工具注入

在 `AgentConfig` 中绑定 workspace 后，`Bash` / `Read` / `Write` / `Edit` / `Grep` / `Glob` / `ResetTools` / `Skill` 自动注入（`PowerShell` 在 Windows 注入）。智能体即可在受控工作区内执行文件与命令操作。

其中 `list_tools()` 直接返回的前六个内置工具（含入参）：

| 工具 | 说明 | 关键入参 |
|------|------|----------|
| `Bash` | 在 workdir 执行 shell 命令 | `command` |
| `Read` | 读取工作空间文件 | `path` |
| `Write` | 写文件 | `path`、`content` |
| `Edit` | 精确字符串替换 | `path`、`old`、`new` |
| `Glob` | 按 glob 模式找文件 | `pattern` |
| `Grep` | 在工作空间文件里搜正则 | `pattern` |

## 完整示例

见 [`examples/workspace`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/workspace/)（`cargo run -p workspace`），演示命令执行与内置工具发现。

## 使用 microsandbox

`agent_scope_sandbox` 提供统一的 `SandboxSession` trait。启用 `microsandbox` feature 后，可以用 `MicrosandboxSession` 把命令和文件操作放到 microsandbox microVM runtime 中执行，再通过 `SandboxWorkspaceBackend` 接到 workspace 工具体系。

最小依赖：

```toml
[dependencies]
agent_scope_sandbox = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master", features = ["microsandbox"] }
agent_scope_workspace = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
tokio = { version = "1", features = ["full"] }
```

最小会话：

```rust
use agent_scope_sandbox::{
    ExecutionRequest, MicrosandboxConfig, MicrosandboxSession, SandboxSession,
};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let mut sandbox = MicrosandboxSession::new(MicrosandboxConfig::default())?;
sandbox.initialize().await?;

let result = sandbox
    .execute(ExecutionRequest::new(["python", "-c", "print('hello from microVM')"]))
    .await?;

println!("{}", String::from_utf8_lossy(&result.stdout.inline));
sandbox.close().await?;
# Ok(())
# }
```

`MicrosandboxConfig::default()` 的默认值：

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `session_id` | `None` | 自动生成 UUID；显式 ID 只能包含 `[A-Za-z0-9_-]` |
| `image` | `"python"` | microsandbox image 名称，不能为空 |
| `workdir` | `"/workspace"` | guest 工作目录；必须是绝对路径，不能是 `/` |
| `policy.network` | `NetworkPolicy::Disabled` | 默认禁用 guest 网络 |
| `mounts` | `[]` | 默认不挂载 host 目录 |
| `env` | `{}` | 只注入显式环境变量；`MSB_` 前缀保留给 runtime，不能作为 guest env |
| `replace_existing` | `false` | 是否替换同名 session |
| `persist` | `false` | 是否保留 runtime session |
| `startup_timeout` | `120s` | 创建 microVM 的超时 |
| `stop_timeout` | `30s` | 停止 microVM 的超时 |

### 挂载 host workspace

真实 Agent 场景通常需要把 host 工作目录挂载到 guest `/workspace`。只挂载需要暴露给智能体的目录，不要挂载 `.ssh`、`.aws`、`.config`、`.kube`、`credentials`、`tokens`、`secrets` 等敏感目录；实现会拒绝这些路径及指向它们的符号链接。

```rust
use std::path::PathBuf;
use std::time::Duration;

use agent_scope_sandbox::{
    ExecutionRequest, MicrosandboxConfig, MicrosandboxSession, MountAccess,
    MountOwner, NetworkPolicy, SandboxMount, SandboxPolicy, SandboxSession,
};

# async fn example(host_workspace: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
let mut sandbox = MicrosandboxSession::new(MicrosandboxConfig {
    image: "python".into(),
    workdir: "/workspace".into(),
    mounts: vec![SandboxMount {
        mount_id: "workspace".into(),
        host_path: host_workspace,
        sandbox_path: PathBuf::from("/workspace"),
        access: MountAccess::ReadWrite,
        persist: false,
        owner: MountOwner::Workspace,
    }],
    policy: SandboxPolicy {
        network: NetworkPolicy::Disabled,
        default_timeout: Duration::from_secs(10),
        max_timeout: Duration::from_secs(30),
        max_output_bytes: 1024 * 1024,
        ..Default::default()
    },
    ..Default::default()
})?;

sandbox.initialize().await?;
sandbox.write_file("note.txt", b"hello").await?;
let result = sandbox
    .execute(ExecutionRequest {
        argv: vec!["python".into(), "-c".into(), "print(open('note.txt').read())".into()],
        cwd: None,
        env: Default::default(),
        timeout: Some(Duration::from_secs(10)),
        stdin: None,
    })
    .await?;
println!("{}", String::from_utf8_lossy(&result.stdout.inline));
sandbox.cleanup().await?;
# Ok(())
# }
```

### 接入 WorkspaceBackend

`SandboxWorkspaceBackend` 把任意 `SandboxSession` 适配成 `WorkspaceBackend`。这样 `Bash` / `Read` / `Write` / `Edit` / `Grep` / `Glob` 等 workspace 工具可以复用同一套接口。

```rust
use agent_scope_sandbox::{MicrosandboxConfig, MicrosandboxSession, SandboxWorkspaceBackend};
use agent_scope_workspace::WorkspaceBackend;

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let session = MicrosandboxSession::new(MicrosandboxConfig::default())?;
let backend = SandboxWorkspaceBackend::from_session(session);
backend.initialize().await?;

let out = backend
    .exec_shell(&["python", "-c", "print('via WorkspaceBackend')"], "/workspace", Some(10.0))
    .await?;
assert_eq!(out.exit_code, 0);

backend.close().await?;
# Ok(())
# }
```

如果要把 microsandbox 后端绑定到完整 `LocalWorkspace` / `AgentConfig`，可参考 [`examples/microsandbox-agent-cli`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/microsandbox-agent-cli/)：模型调用发生在 host 进程，workspace 工具的文件和命令操作通过 microsandbox backend 执行。

### SandboxSession API

`MicrosandboxSession` 实现完整 `SandboxSession`：

| 方法 | 说明 |
|------|------|
| `session_id()` | 返回会话 ID |
| `state()` | 返回 `Created` / `Ready` / `Closing` / `Closed` / `Failed` |
| `policy()` | 返回当前 `SandboxPolicy` |
| `initialize()` | 创建并启动 microsandbox session；幂等处理已 ready / closed 状态 |
| `execute(request)` | 执行 argv 命令，支持 cwd、env、timeout、stdin |
| `read_file(path)` | 读取 guest 文件，单次上限 10 MiB |
| `write_file(path, data)` | 写 guest 文件，自动创建父目录；只读挂载会拒绝 |
| `delete_path(path)` | 删除 guest 文件或目录；拒绝删除 workdir 根 |
| `is_dir(path)` / `path_exists(path)` | 查询 guest 路径 |
| `stat_mtime(path)` | 返回修改时间（Unix 秒），不可用时返回 `None` |
| `list_dir(path, recursive)` | 列目录，递归模式会返回嵌套条目 |
| `history()` | 返回命令执行审计记录 |
| `capability_report()` | 返回 backend 能力与不支持项 |
| `close()` / `cleanup()` | 停止 session，并按 `persist` / `keep_on_close` 清理输出引用 |

`ExecutionRequest` 字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `argv` | `Vec<String>` | 命令和参数；不能为空 |
| `cwd` | `Option<PathBuf>` | guest 执行目录；缺省为 `config.workdir` |
| `env` | `HashMap<String, String>` | 注入给本次命令的环境变量 |
| `timeout` | `Option<Duration>` | 本次命令超时；不能超过 `policy.max_timeout` |
| `stdin` | `Option<Vec<u8>>` | 写入命令 stdin 的字节 |

`ExecutionResult` 字段：

| 字段 | 说明 |
|------|------|
| `execution_id` | 本次执行 ID |
| `status` | `Exited { code }` / `TimedOut` / `PermissionDenied` / `UnsupportedFeature` / `SandboxError` / `Cancelled` |
| `exit_code` | 进程退出码；超时等非正常状态为 `None` |
| `stdout` / `stderr` | `OutputSummary`，包含 inline 字节、是否截断、完整输出引用 |
| `started_at` / `finished_at` / `duration` | 执行时间信息 |
| `resource_hits` | `Timeout` / `OutputTruncated` 等资源命中记录 |

### 策略边界

microsandbox 后端坚持精确语义，不会把不支持的策略悄悄放宽：

| 策略 | microsandbox 当前行为 |
|------|------------------------|
| `NetworkPolicy::Disabled` | 支持，默认值；创建时调用 runtime 的 disable network |
| `NetworkPolicy::Unrestricted` | 支持，显式允许 guest 网络 |
| `NetworkPolicy::LoopbackOnly` | 不支持，返回 `UnsupportedFeature` |
| `NetworkPolicy::Allowlist` | 不支持，返回 `UnsupportedFeature` |
| `memory_limit_bytes` | 支持，按 MiB 向上取整传给 runtime |
| `cpu_limit.cpu_shares` | 不支持，`cpu_shares` 不是 microsandbox vCPU 数的等价映射 |
| `process_limit` | 不支持，当前 SDK 没有稳定映射 |
| `max_output_bytes` | 支持 inline 截断；完整输出写入 `OutputRef` |

此外，`MicrosandboxSession` 会显式使用 local microsandbox backend，并拒绝 `MSB_API_KEY`、`MSB_BACKEND`、`MSB_PROFILE`、`MSB_CONFIG_PATH` 这类环境变量触发 SDK 的 ambient backend/profile 选择。runtime、image 或平台不可用时，会返回 `SandboxUnavailable` / `SandboxError`，不会 fallback 到 `LocalSandboxSession`。

### 运行命令

默认 local sandbox 示例无需 microsandbox runtime：

```bash
cargo run -p sandbox
```

只检查 microsandbox feature 能否编译：

```bash
cargo check -p agent_scope_sandbox --features microsandbox
```

编译并运行示例，但不启动真实 runtime：

```bash
cargo run -p sandbox --features microsandbox
```

显式运行真实 microsandbox 路径：

```bash
AGENTSCOPE_RUN_MICROSANDBOX_EXAMPLE=1 \
  cargo run -p sandbox --features microsandbox
```

运行真实 runtime integration tests：

```bash
AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1 \
  cargo test -p agent_scope_sandbox --features microsandbox --test microsandbox_tests -- --ignored
```
