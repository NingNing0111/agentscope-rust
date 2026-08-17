---
title: "运行工作空间"
description: "在受控环境执行命令与文件操作"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用。`WorkspaceBackend` 命令执行与内置工具注入均可用。
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
