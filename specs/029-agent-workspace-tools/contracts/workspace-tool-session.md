# Contract: WorkspaceToolSession（共享会话状态）

**Feature**: 029-agent-workspace-tools | **Status**: Draft

## 概述

`WorkspaceToolSession` 是 workspace 内置工具间共享的会话级状态。它承载两个关注点：
1. **Read-state**：当前工具会话中已成功读取的文件路径集合（读-改守卫前提）。
2. **激活组视图**：ResetTools 管理的激活工具组（授权边界内）。

激活状态的权威存储是 `agent_scope_state::ToolContext.activated_groups`（随 AgentState 持久化）；`WorkspaceToolSession` 持有其读写视图或同步镜像。

## 结构

```rust
/// Per-agent tool-session state shared by workspace built-in tools.
pub struct WorkspaceToolSession {
    /// Workspace this state belongs to.
    workspace_id: String,
    /// Normalized paths successfully read during the current tool session.
    read_files: std::collections::BTreeSet<String>,
    /// Currently activated tool groups (managed by ResetTools).
    active_tool_groups: std::collections::BTreeSet<String>,
}
```

## 行为契约

| 方法 | 行为 |
|------|------|
| `record_read(path)` | Read 成功后插入归一化且 workspace 内的路径 |
| `is_read(path)` | Edit/Write 前检查路径是否已读 |
| `clear_reads()` | session 重置时清空 |
| `record_groups(groups)` | ResetTools 写入激活组（final-state 语义） |
| `list_groups()` | 当前激活组集合 |
| `group_authorized(name)` | 校验组是否在授权范围内 |

## Read-state 守卫规则（FR-008/FR-012）

| 工具 | 触发 | 要求 |
|------|------|------|
| Read | 成功读取文件 | 路径记入 `read_files` |
| Edit | 已存在文件 | 路径必须在 `read_files`，否则拒绝（`read_before_modify_required`） |
| Write | 已存在文件覆盖 | 路径必须在 `read_files`，否则拒绝 |
| Write | 新文件 | 无需读，直接创建 |

**路径归一化**：插入前必须归一化（resolve `..`）并验证 workspace 内（避免不同路径写法绕过守卫）。

## 隔离

- session 状态 MUST NOT 跨无关 agent 或 workspace 共享。
- 每个启用 workspace 的 agent 一个 `WorkspaceToolSession`。

## 授权边界（FR-019）

- `active_tool_groups` 仅能在当前 workspace 授权范围内变化。
- ResetTools 不得借此创建新权限或逃逸 workspace。

## 交叉引用

- Spec: FR-008, FR-012, FR-019
- 相关: `contracts/read.md`、`contracts/edit.md`、`contracts/write.md`、`contracts/reset-tools.md`
- Python: `tool_context.cache_file`（`_read.py:244`）——等价守卫，但 Rust 用 BTreeSet 路径集合更轻量
