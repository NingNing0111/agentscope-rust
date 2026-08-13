# Contract: ResetTools 元工具

**Feature**: 029-agent-workspace-tools | **Status**: Draft
**兼容基准**: Python `agentscope/tool/_builtin/_meta.py`（上游 `9d1026fa`）

## 工具名

`ResetTools`（Python 端为 `reset_tools`，Rust 侧按 spec FR-019 命名 `ResetTools`——兼容性矩阵记录此命名偏差）

## 描述（对齐 Python `_meta.py:26`）

允许 agent 基于当前任务需求重置其已装配工具。工具按组组织，可通过为每组指定布尔值来激活/停用。**输入布尔值是工具组的最终激活状态，不是增量变化**——任何未显式置 true 的组都会被停用（无论先前状态）。最佳实践：按需激活、及时停用以节省上下文。返回激活工具组的使用说明，agent **必须注意并遵循**。

## 输入 Schema

```json
{
  "type": "object",
  "properties": {
    "<group_name>": {
      "type": "boolean",
      "description": "The group's agent-oriented description.",
      "default": false
    }
  },
  "required": []
}
```

> 动态生成：每个非 "basic" 工具组一个布尔字段。`basic` 组始终激活，不在此列出。

## 行为契约（对齐 Python `_meta.py:90`）

| 步骤 | 行为 |
|------|------|
| 前置 | 需要 AgentState（工具上下文）——`is_state_injected = true` |
| 激活重置 | `activated_groups.clear()` 后按 true 的组重新填充（final-state 语义） |
| 授权边界 | **仅能激活当前 workspace 授权范围内的组；不创建新权限**（FR-019） |
| 输入校验 | 非布尔参数 → 拒绝：`Invalid arguments: the argument {key} should be a bool value` |
| 返回 | 激活工具组的 instructions 渲染结果；未激活组无 instructions |
| basic 组 | 始终激活（Python `_meta.py:66` 跳过） |

## 状态

`is_read_only = false`（修改激活状态） | `is_concurrency_safe = true` | `is_state_injected = true`

## 激活状态落点

- 写入 `agent_scope_state::ToolContext.activated_groups: Vec<String>`（`crates/agent_scope_state/src/agent_state.rs:85`）。
- 随 AgentState 持久化（会话恢复后激活状态仍在）。
- 影响：`ToolKit.get_tool_schemas()` 按激活组过滤返回集合（工具可见性）。

## 错误契约

| 场景 | 类别 | 错误码 |
|------|------|--------|
| 缺少 AgentState | `ExecutionFailure` | `internal_error` |
| 非布尔参数 | `ValidationFailure` | `invalid_arguments` |
| 请求越权组 | `PermissionDenied` | `permission_denied`（FR-019：不得超出授权） |

## 交叉引用

- Python: `_meta.py`
- Spec: FR-019
- 相关: `contracts/workspace-tool-session.md`（激活组 + read-state 共享 session）
