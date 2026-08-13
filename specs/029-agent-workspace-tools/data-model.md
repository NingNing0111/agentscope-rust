# Data Model: Agent Workspace Built-in Tools

## WorkspaceEnabledAgent

Represents an agent configured with explicit access to a bounded workspace.

**Fields**:
- `agent_id: String` — stable agent identifier.
- `workspace_id: String` — active workspace identifier.
- `workspace_root: String` — canonical workspace root used for containment.
- `toolkit: ToolKit` — agent tool registry after workspace built-ins are injected.
- `workspace_tool_session: WorkspaceToolSession` — shared session state for built-in workspace tools.

**Relationships**:
- Owns or references one active workspace.
- Registers many `BuiltInToolDefinition` entries into its `ToolKit`.
- Produces many `ToolInvocation` records during an agent run.

**Validation rules**:
- Workspace built-ins are injected only when workspace access is explicitly configured.
- Workspace root must be canonical and contained by the active workspace backend.
- Agents without workspace access must not expose default file mutation or command execution tools.

## BuiltInToolDefinition

Stable public contract for one built-in workspace tool.

**Fields**:
- `name: String` — public tool name, such as `Bash`, `Edit`, `Write`, `Grep`, `Glob`, `PowerShell`, `ResetTools`, or `Skill`.
- `description: String` — human-readable model-facing description.
- `input_schema: serde_json::Value` — machine-readable JSON Schema object.
- `availability: ToolAvailability` — workspace and platform requirements.
- `read_only: bool` — whether the tool has no observable mutation side effects.
- `concurrency_safe: bool` — whether parallel calls are supported.

**Relationships**:
- Exported through workspace `ToolInfo` discovery and `ToolKit` schema export.
- Referenced by `ToolInvocation.tool_name`.

**Validation rules**:
- Names must be stable and unique within a `ToolKit`.
- Input schemas must include all required parameters from the contract.
- `PowerShell` availability depends on platform support.
- `ResetTools` must not expand permissions beyond current authorization.

## WorkspaceToolSession

Per-agent tool-session state shared by workspace built-in tools.

**Fields**:
- `workspace_id: String` — workspace this state belongs to.
- `read_files: BTreeSet<String>` — normalized paths that were successfully read during the current tool session.
- `active_tool_groups: BTreeSet<String>` — currently active tool groups managed by `ResetTools`.

**Relationships**:
- Updated by successful `Read` calls.
- Consulted by `Edit` and overwrite `Write`.
- Updated by `ResetTools` within authorized group boundaries.

**Validation rules**:
- Read paths must be normalized and workspace-contained before insertion.
- Existing-file `Edit` requires membership in `read_files`.
- Existing-file `Write` overwrite requires membership in `read_files`.
- Session state must not be shared across unrelated agents or workspaces.

## ToolInvocation

A single invocation of a workspace built-in tool.

**Fields**:
- `tool_name: String` — invoked built-in tool name.
- `arguments_summary: serde_json::Value` — redacted/summarized arguments suitable for traces.
- `started_at: String` — RFC3339 timestamp.
- `finished_at: Option<String>` — RFC3339 timestamp when completed.
- `result_state: ToolResultState` — success or error.
- `error_category: Option<ToolErrorCategory>` — validation, permission, unsupported capability, timeout, execution, or internal.
- `trace_sequence: u64` — ordering relative to agent events.

**Relationships**:
- References one `BuiltInToolDefinition` by name.
- Appears in agent trace output in invocation order.

**Validation rules**:
- Rejections must include an actionable error category.
- Sensitive command output or arguments must be redacted where appropriate.
- Timeout results must be distinguishable from command non-zero exit results.

## ToolAvailability

Defines when a built-in tool can be exposed.

**Fields**:
- `requires_workspace: bool` — true for workspace file and command tools.
- `requires_windows_shell: bool` — true for `PowerShell`.
- `requires_skill_catalog: bool` — true for `Skill` content lookup.

**Validation rules**:
- Tools with `requires_workspace = true` are unavailable without workspace access.
- Tools with unsupported platform requirements are omitted from default injection or fail with unsupported capability when explicitly requested.

## ToolErrorCategory

Typed error category for rejected or failed invocations.

**Variants**:
- `ValidationFailure` — invalid or missing parameters, empty `old_string`, ambiguous edit match.
- `PermissionDenied` — workspace boundary or authorization failure.
- `UnsupportedCapability` — platform-dependent tool unavailable, such as `PowerShell` on non-Windows.
- `Timeout` — command exceeded configured timeout.
- `ExecutionFailure` — command failed or filesystem operation failed after validation.
- `InternalFailure` — unexpected framework error.

## State Transitions

### Workspace built-in injection

```text
AgentConfig(no workspace) -> ToolKit(no workspace built-ins)
AgentConfig(workspace configured) -> initialize workspace -> ToolKit(with workspace built-ins)
```

### Read-before-modify

```text
UnseenFile -> Read success -> ReadFile
ReadFile -> Edit success -> ModifiedFile
ReadFile -> Write overwrite success -> ModifiedFile
UnseenExistingFile -> Edit/Write overwrite -> Rejected(ValidationFailure)
```

### Command timeout

```text
CommandStarted -> Completed(exit_code) -> SuccessResult
CommandStarted -> TimeoutReached -> KillProcess -> ErrorResult(Timeout)
```

### ResetTools

```text
ActiveToolGroups(current authorized subset)
  -> ResetTools(default)
  -> ActiveToolGroups(default authorized subset)

ActiveToolGroups(current authorized subset)
  -> ResetTools(requested groups)
  -> ActiveToolGroups(requested ∩ authorized)
```

---

## ToolErrorCategory → 宪法 Art.13 错误模型映射

| 029 错误类别 | 宪法 Art.13 类别 | 触发场景 | 示例错误码 |
|--------------|------------------|----------|-----------|
| `ValidationFailure` | `ValidationError` | 参数缺失/非法、`old_string` 为空/未找到/非唯一、路径为空 | `invalid_arguments`, `pattern_not_found`, `ambiguous_edit` |
| `PermissionDenied` | `PermissionDenied` | workspace 边界逃逸（`..`/符号链接）、未读先改 | `path_outside_workspace`, `read_before_modify_required` |
| `UnsupportedCapability` | `UnsupportedFeature` | 非 Windows 平台请求 PowerShell | `unsupported_capability` |
| `Timeout` | `TimeoutError` | 命令超过配置超时窗口 | `command_timeout` |
| `ExecutionFailure` | `ToolError` | 命令非零退出、文件系统操作失败（校验通过后） | `command_failed`, `permission_denied`, `file_not_found` |
| `InternalFailure` | `InternalError` | 框架内部意外错误 | `internal_error` |

> 映射依据：宪法 Art.13 要求 typed error + 机器可读错误码 + 区分类别。029 的 6 类错误覆盖文件/命令工具的失败面，均可在 Art.13 十类中找到对应，无新增类别。

## ToolGroup 激活状态落点

**决策**: 激活状态复用 `agent_scope_state::ToolContext.activated_groups: Vec<String>`（`crates/agent_scope_state/src/agent_state.rs:85` 已存在，`#[serde(default)]`，随会话持久化），而非新建 session 级字段。

**理由**:
- 与 Python 参考实现一致——Python `ResetTools` 直接读写 `_agent_state.tool_context.activated_groups`（`_meta.py:103,120`）。
- 避免重复定义激活语义，且随 AgentState 持久化（会话恢复后激活状态仍在）。
- 029 的 `WorkspaceToolSession` 仅承载 read-state（读-改守卫），激活状态归属 AgentState 层。

**ResetTools 对激活状态的操作**:
- 输入：每个非 "basic" 工具组一个布尔字段（final state 语义，非增量）。
- 执行：`activated_groups.clear()` 后按 true 的组重新填充（对齐 Python `_meta.py:102-120`）。
- 授权边界：仅能激活属于当前 workspace 授权范围的工具组；不创建新权限（FR-019）。

## ToolInfo 扩展（工具契约元数据）

当前 `agent_scope_workspace::base::ToolInfo`（`base.rs:12`）仅含 `{name, description, input_schema}`。029 的 `BuiltInToolDefinition` 需要 `availability`、`read_only`、`concurrency_safe` 字段，**超出现有 ToolInfo**。

**决策**: 在 `agent_scope_tool` crate 内新增 `builtin::BuiltInToolInfo` 契约类型（携带上述字段），由各内置工具的静态属性派生；`WorkspaceBase::list_tools()` 的 `ToolInfo` 保持轻量元数据不动（兼容既有 012 契约），两者通过工具名关联。这样既不破坏 workspace 的 012 契约，又让 tool crate 提供机器可读的完整契约（FR-022）。
