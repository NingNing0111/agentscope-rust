---
title: "管理资源"
description: "管理工作空间内的文件、技能、MCP 配置与多租户"
---

<Note>
**Rust 实现状态**: 部分支持。
- 已支持：`LocalWorkspace` 管理 workdir / 技能 / MCP 配置（`.mcp` 持久化，敏感头脱敏）/ 会话数据卸载，`WorkspaceManager` 多租户 + TTL 清理。
- 尚未实现：更细粒度的资源配额与资源列表模型，远程（Docker / E2B / K8s）后端。
</Note>

工作空间内的资源包括文件、技能、MCP 配置与多租户工作空间实例。

## 文件与目录

`LocalWorkspace` 的工作目录（workdir）即文件边界，所有文件操作（`WorkspaceBackend` 的 `read_file` / `write_file` / `list_dir` / `delete_path` / `exec_shell`）都被限定在 workdir 内，绝对路径、`..` 穿越与符号链接逃逸会被拒绝（由 `ContainedBackend` 保证）。

工作空间内部目录结构：

| 目录 / 文件 | 用途 |
|-------------|------|
| `skills/` | 技能目录，每个技能一个子目录，内含 `SKILL.md` |
| `sessions/` | 会话数据，`context.jsonl` 与 `tool_result-*.txt` |
| `data/` | 从消息中提取出来的 base64 数据文件 |
| `.mcp` | MCP 客户端配置（JSON，敏感头脱敏后持久化） |

## 技能管理

`WorkspaceBase` 提供技能管理：

| 方法 | 说明 |
|------|------|
| `list_skills()` | 列出工作空间技能，返回 `Vec<Skill>` |
| `add_skill(skill_path)` | 把本地技能目录复制进 `skills/` |
| `remove_skill(name)` | 按名称移除技能 |

技能以 `SKILL.md` 目录形式存放于工作空间内（见 [Skill](../tool/skill)）。`Skill` 结构体字段：

| 字段 | 说明 |
|------|------|
| `name` | 技能名（智能体可见） |
| `description` | 技能描述 |
| `dir` | 技能目录路径 |
| `markdown` | `SKILL.md` 正文内容 |
| `updated_at` | 最后更新时间（Unix 秒时间戳） |

同名技能会按 `name (1)`、`name (2)` 依次去重；复制时跳过符号链接，避免越界。

## MCP 配置

MCP 客户端配置持久化在工作空间（`.mcp` 文件），敏感头（`authorization`、`x-api-key`、`cookie` 等）在持久化与 `list_mcps()` 返回时会被替换为 `[REDACTED]`：

| 方法 | 说明 |
|------|------|
| `list_mcps()` | 列出已注册的 MCP 客户端配置（脱敏后） |
| `add_mcp(config)` / `remove_mcp(name)` | 注册 / 移除（自动保存到 `.mcp`） |

`McpClientConfig` 字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `String` | MCP 客户端唯一名 |
| `transport` | `McpTransportConfig` | 传输方式，见下 |
| `is_stateful` | `bool` | 是否维护有状态连接（默认 `true`） |

`McpTransportConfig` 支持 Stdio / SSE / StreamableHttp（见 [MCP](../tool/mcp)）。

## 多租户管理

`WorkspaceManager`（`agent_scope_workspace::manager`）按 ID 创建/获取工作空间，支持 TTL 清理，适合服务化场景下的多租户隔离：

| 方法 | 说明 |
|------|------|
| `new(ttl, factory)` | 创建管理器；`ttl` 为 `None` 表示永不过期，`factory` 是「key → 配置」的工厂闭包 |
| `get(key)` | 按 key 获取或创建工作空间，返回 `Arc<dyn WorkspaceBase>` |

```rust
use std::time::Duration;
use agent_scope_workspace::{LocalWorkspaceConfig, WorkspaceManager};

let manager = WorkspaceManager::new(
    Some(Duration::from_secs(3600)),  // TTL：空闲 1 小时后清理
    |id| LocalWorkspaceConfig {
        workdir: format!("/data/ws/{id}"),
        workspace_id: Some(id),
        ..Default::default()
    },
);

let ws = manager.get("tenant-a").await?;  // 创建或复用
```

TTL 清理在后台以 `ttl / 2` 的间隔轮询，把超过 `ttl` 未访问的工作空间逐出；管理器析构时会自动中止清理任务。

## 卸载上下文

`WorkspaceBase` 还提供两个卸载方法，用于把长会话的数据落到磁盘、释放内存：

| 方法 | 说明 |
|------|------|
| `offload_context(session_id, msgs)` | 把消息追加到 `sessions/{id}/context.jsonl`，其中的 base64 数据块提取为 `data/` 下的文件并替换为 `file://` URL |
| `offload_tool_result(session_id, result)` | 把工具结果写到 `sessions/{id}/tool_result-{id}.txt` |

## 完整示例

见 [`examples/workspace`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/workspace/)。
