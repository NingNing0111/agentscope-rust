---
title: "概述"
description: "把智能体的执行隔离到受控工作空间"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用。当前实现为 `LocalWorkspace`（本地文件系统工作空间）；Docker / E2B / K8s 等远程沙箱后端为「计划中」。
</Note>

工作空间（Workspace）是智能体的执行环境：文件读写、命令执行、MCP 客户端与技能都发生在工作空间内，边界受控。AgentScope Rust 通过 `WorkspaceBase` trait（`agent_scope_workspace` crate）统一抽象：

| 类型 | 职责 |
|------|------|
| `WorkspaceBase` | 工作空间统一接口：初始化、工具发现、MCP 管理、技能管理、卸载与后端访问 |
| `LocalWorkspace` | 本地文件系统实现（workdir 隔离 + 路径越界防护） |
| `WorkspaceBackend` | 后端执行接口：`exec_shell` / `read_file` / `write_file` / `list_dir` 等 |
| `WorkspaceManager` | 多租户管理：按 ID 创建/获取工作空间 + TTL 清理 |

## WorkspaceBase trait

`WorkspaceBase` 定义了一个工作空间该有的全部能力，按用途分为几组：

| 分组 | 方法 | 说明 |
|------|------|------|
| 生命周期 | `initialize()` | 准备资源、恢复 MCP 配置、播种技能；幂等，已存活时是空操作 |
| 生命周期 | `close()` | 释放所有资源与连接 |
| 生命周期 | `reset()` | 把工作空间清空：删除 `skills/`、`sessions/`、`data/` 与 `.mcp`（不重新播种 `default_mcps` / `skill_paths`） |
| 访问器 | `workspace_id()` / `workdir()` / `is_alive()` | 返回工作空间 ID、工作目录路径、是否已初始化 |
| 工具发现 | `list_tools()` | 列出工作空间的内置工具（返回 `ToolInfo`） |
| 工具发现 | `get_instructions()` | 返回工作空间专属的系统提示片段 |
| MCP 管理 | `list_mcps()` / `add_mcp(config)` / `remove_mcp(name)` | 列出 / 注册 / 移除 MCP 客户端配置，配置持久化到 `.mcp` |
| 技能管理 | `list_skills()` / `add_skill(path)` / `remove_skill(name)` | 列出 / 复制入 / 移除技能 |
| 卸载 | `offload_context(session_id, msgs)` | 把消息追加到 `sessions/{id}/context.jsonl`，base64 数据提取到 `data/` |
| 卸载 | `offload_tool_result(session_id, result)` | 把工具结果写入 `tool_result-{id}.txt` |
| 后端访问 | `get_backend()` / `get_backend_arc()` | 获取执行后端（借用 / 持有 `Arc`） |

其中 `list_tools()` 返回的 `ToolInfo` 包含三个字段：

| 字段 | 说明 |
|------|------|
| `name` | 工具唯一名，如 `Bash`、`Read`、`Write` |
| `description` | 面向人类的一句话描述 |
| `input_schema` | 工具入参的 JSON Schema |

## 创建工作空间

`LocalWorkspace` 是本地文件系统实现，通过 `LocalWorkspaceConfig` 配置：

| 字段 | 类型 | 说明 |
|------|------|------|
| `workdir` | `String` | 工作目录（绝对路径或相对路径，创建时会被规范化） |
| `workspace_id` | `Option<String>` | 工作空间 ID，缺省时自动生成 UUID |
| `default_mcps` | `Vec<McpClientConfig>` | 初始化时自动注册的 MCP 客户端配置 |
| `skill_paths` | `Vec<String>` | 初始化时自动复制进工作空间的技能目录 |
| `instructions` | `Option<String>` | 自定义工作空间系统提示，缺省用内置默认提示 |

```rust
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
    workdir: "/tmp/my-ws".into(),
    workspace_id: Some("demo".into()),
    default_mcps: vec![],
    skill_paths: vec![],
    instructions: None,
});
ws.initialize().await?;

let tools = ws.list_tools().await?;   // 内置工具
let backend = ws.get_backend()?;      // 后端执行接口
let out = backend.exec_shell(&["echo", "hi"], "/tmp/my-ws", None).await?;
```

## WorkspaceBackend 与 ExecOutput

`WorkspaceBackend` 是文件系统与进程 I/O 的抽象接口，主要方法：

| 方法 | 说明 |
|------|------|
| `exec_shell(cmd, cwd, timeout_secs)` | 执行 shell 命令，`cmd` 是 argv 数组 |
| `read_file(path)` | 读取整个文件为字节（单次上限 10 MiB） |
| `write_file(path, data)` | 写入文件，自动创建父目录 |
| `is_dir(path)` | 判断是否为目录 |
| `list_dir(path, recursive)` | 列出目录条目，返回完整路径；`recursive` 控制是否递归 |
| `delete_path(path)` | 删除文件或递归删除目录，幂等 |
| `file_exists(path)` | 判断文件或目录是否存在 |
| `join_path(a, b)` / `basename(p)` / `dirname(p)` | 纯字符串路径运算 |
| `stat_mtime(path)` | 获取修改时间（Unix 秒时间戳，不存在返回 `None`） |
| `normpath(path)` / `is_absolute(path)` | 路径规范化 / 是否绝对路径 |

`exec_shell` 返回 `ExecOutput`：

| 字段 | 类型 | 说明 |
|------|------|------|
| `stdout` | `Vec<u8>` | 标准输出（单次上限 1 MiB，超限截断） |
| `stderr` | `Vec<u8>` | 标准错误 |
| `exit_code` | `i32` | 退出码，`ok()` 方法判断是否为 0 |

> **边界说明**：`LocalBackend` 本身不强制 workdir 边界；`LocalWorkspace` 内部用 `ContainedBackend` 包一层，把绝对路径、`..` 穿越与符号链接逃逸全部拒绝，从而保证所有 `get_backend()` 调用方都自动受边界保护。

## 绑定到 Agent

在 `AgentConfig` 中绑定 workspace 后，内置工具（Bash / Read / Write / Edit / Grep / Glob / ResetTools / Skill）自动注入：

```rust
use std::sync::Arc;

let ws: Arc<dyn WorkspaceBase> = Arc::new(ws);
let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .workspace(ws)
    .build()?;
```

## 完整示例

见 [`examples/workspace`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/workspace/)（`cargo run -p workspace`），演示工作空间初始化、内置工具发现与命令执行，无需模型凭据。

## 沙箱执行

沙箱（Sandbox）在工作空间内提供**受控命令执行**。`agent_scope_sandbox` crate 通过统一的 `SandboxSession` trait 抽象后端：默认可用的是 local-process reference backend；启用 `microsandbox` feature 后，可以显式使用 microsandbox microVM backend。

| 类型 | 职责 |
|------|------|
| `SandboxSession` | 沙箱统一接口：`initialize` / `execute` / 文件操作 / `history` / `capability_report` |
| `LocalSandboxSession` | 默认本地进程实现，使用临时根目录、路径 containment、命令 timeout 与输出上限；不是强隔离 |
| `MicrosandboxSession` | feature-gated microsandbox 实现，使用 microVM runtime；默认 image 为 `python`，默认 guest workdir 为 `/workspace` |
| `SandboxPolicy` | 超时、输出上限、网络、cpu/memory/process 限制等策略；后端必须通过 `CapabilityReport` 报告可支持边界 |
| `SandboxPathResolver` | 路径规范化与边界检查（拒绝 `..`、符号链接逃逸） |
| `SandboxWorkspaceBackend` | 把任意 `SandboxSession` 适配成 workspace backend |
| `ExecutionResult` | 一次命令执行的结果与资源命中记录 |
| `CapabilityReport` | 能力报告：列出支持 / 不支持的能力 |

| 后端 | 默认可用性 | 隔离边界 | 网络策略 | 资源限制 |
|------|------------|----------|----------|----------|
| `LocalSandboxSession` | 默认编译，无需外部 runtime | 本地进程 + 临时根目录；Rust 文件 API 防 traversal/symlink escape；命令执行仍是宿主本地进程语义 | 跟随 `SandboxPolicy::default()` 的 host/unrestricted 语义 | 本地后端不伪装强制 CPU / memory / process limit |
| `MicrosandboxSession` | 需 `--features microsandbox`，真实运行还需安装 microsandbox runtime | microsandbox microVM；示例与测试默认只 mount 新建 host tempdir 到 guest `/workspace` | `MicrosandboxConfig::default()` 默认 `NetworkPolicy::Disabled`；支持显式 `Disabled` / `Unrestricted` | `memory_limit_bytes` 转换为 MiB；`cpu_limit.cpu_shares` 和 `process_limit` 当前返回 unsupported |

> **边界**：local-process backend 是 reference backend，不是 microVM/container 级强隔离。microsandbox backend 不会在 runtime、平台或 image 不可用时 fallback 到 local-process；失败会以 sandbox 错误返回。

当前 microsandbox policy 保持精确语义：`LoopbackOnly` / `Allowlist` 不会被自动放宽成 unrestricted，而是返回 unsupported；`cpu_limit.cpu_shares` 与 `process_limit` 也不会做不等价映射。调用方应读取 `CapabilityReport`，不要把“后端存在”理解为“所有 policy 均可强制执行”。

默认 local 示例无需模型凭据或 microsandbox runtime：

```bash
cargo run -p sandbox
```

只检查 feature-gated microsandbox 后端能否编译：

```bash
cargo check -p agent_scope_sandbox --features microsandbox
```

启用 feature 但不设置运行 gate 时，`examples/sandbox` 仍只打印 skip 提示，不启动真实 runtime：

```bash
cargo run -p sandbox --features microsandbox
```

显式运行真实 microsandbox 示例需本机 runtime 可用，并设置环境变量：

```bash
AGENTSCOPE_RUN_MICROSANDBOX_EXAMPLE=1 \
  cargo run -p sandbox --features microsandbox
```

真实 runtime integration tests 默认被 `#[ignore]` 和环境变量双重保护：

```bash
AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1 \
  cargo test -p agent_scope_sandbox --features microsandbox --test microsandbox_tests -- --ignored
```

更多使用说明见 [`examples/sandbox`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/sandbox/) 和 [`specs/035-microsandbox-sandbox-backend/quickstart.md`](https://github.com/NingNing0111/agentscope-rust/blob/master/specs/035-microsandbox-sandbox-backend/quickstart.md)。

## 下一步

<CardGroup :cols="2">
  <Card title="管理资源" icon="folder" href="/building-blocks/workspace/manage-resources">
    workdir、技能、MCP 配置与多租户管理。
  </Card>
  <Card title="运行工作空间" icon="play" href="/building-blocks/workspace/run-workspace">
    在受控环境执行命令与文件操作。
  </Card>
  <Card title="MCP Gateway" icon="plug" href="/building-blocks/workspace/mcp-gateway">
    MCP 客户端的接入方式。
  </Card>
</CardGroup>
