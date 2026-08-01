# Data Model: Sandbox Feature

**Feature**: 017-sandbox-feature | **Date**: 2026-08-01

## Entity Relationship

```text
┌────────────────────┐
│   SandboxPolicy    │
└─────────┬──────────┘
          │ configures
┌─────────▼──────────┐        owns        ┌────────────────────┐
│   SandboxSession   │───────────────────▶│   SandboxMount     │
│  lifecycle + root  │                    │ path/access policy │
└─────────┬──────────┘                    └────────────────────┘
          │ produces
┌─────────▼──────────┐        references  ┌────────────────────┐
│  ExecutionRecord   │───────────────────▶│     OutputRef      │
└─────────┬──────────┘                    └────────────────────┘
          │ latest result
┌─────────▼──────────┐
│  ExecutionResult   │
└────────────────────┘

┌──────────────────────────┐       adapts        ┌────────────────────┐
│ SandboxWorkspaceBackend  │────────────────────▶│  WorkspaceBackend  │
└──────────────────────────┘                     └────────────────────┘

┌────────────────────┐
│  CapabilityReport  │
└────────────────────┘
```

## Entity Definitions

### 1. SandboxSession

**Purpose**: 一个隔离执行环境的生命周期边界。所有命令、文件访问、挂载和审计历史都绑定到该会话。

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | `String` | Yes | 全局唯一会话标识，建议 UUID |
| `root_dir` | `PathBuf` | Yes | 沙箱根目录或后端根引用 |
| `workdir` | `PathBuf` | Yes | 命令默认工作目录，必须位于授权范围内 |
| `state` | `SandboxState` | Yes | 当前生命周期状态 |
| `policy` | `SandboxPolicy` | Yes | 资源与安全策略 |
| `mounts` | `Vec<SandboxMount>` | Yes | 授权挂载列表 |
| `history` | `Vec<ExecutionRecord>` | Yes | 按 sequence 排序的执行历史 |
| `created_at` | `DateTime<Utc>` | Yes | 创建时间 |
| `closed_at` | `Option<DateTime<Utc>>` | No | 关闭时间 |

**Validation Rules**:
- `session_id` 不可为空。
- `root_dir` 和 `workdir` 必须可 canonicalize，且 `workdir` 位于 `root_dir` 或显式挂载范围内。
- `state == Closed` 后必须拒绝新的命令和文件操作。
- `initialize()`、`close()`、`cleanup()` 必须幂等。

### 2. SandboxState

| Variant | Description |
|---------|-------------|
| `Created` | 配置已构造但资源尚未 provision |
| `Ready` | 可执行命令与文件操作 |
| `Closing` | 正在终止运行中进程并清理资源 |
| `Closed` | 已关闭；拒绝新操作 |
| `Failed` | 初始化或运行时系统故障；仅允许查询诊断和清理 |

**State Transitions**:

```text
Created ── initialize() ──▶ Ready
Created ── init failure ─▶ Failed ── cleanup() ─▶ Closed
Ready ──── close() ──────▶ Closing ── cleanup done ─▶ Closed
Ready ──── fatal error ──▶ Failed
Closed ─── any op ───────▶ LifecycleError
```

### 3. SandboxPolicy

**Purpose**: 描述资源限制和安全策略。

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `default_timeout` | `Duration` | 30s | 未指定单次命令 timeout 时使用 |
| `max_timeout` | `Duration` | 300s | 单次命令允许的最大 timeout |
| `max_output_bytes` | `usize` | 1 MiB | stdout+stderr 内联上限 |
| `network` | `NetworkPolicy` | `Disabled` | 网络访问策略 |
| `writable_roots` | `Vec<PathBuf>` | `[workdir]` | 可写路径范围 |
| `readonly_roots` | `Vec<PathBuf>` | `[]` | 只读路径范围 |
| `keep_on_close` | `bool` | `false` | close 后是否保留临时文件供调试 |
| `cpu_limit` | `Option<CpuLimit>` | `None` | 可选 CPU 限制；不可用时显式报错 |
| `memory_limit_bytes` | `Option<u64>` | `None` | 可选内存限制；不可用时显式报错 |
| `process_limit` | `Option<u32>` | `None` | 可选进程数限制；不可用时显式报错 |

**Validation Rules**:
- `default_timeout <= max_timeout`。
- `max_output_bytes > 0`。
- 所有 root/mount 路径必须规范化后比较。
- 不支持的资源限制不得静默忽略。

### 4. NetworkPolicy

| Variant | Description |
|---------|-------------|
| `Disabled` | 默认策略；不允许外部网络访问。若后端无法强制禁网，必须在能力报告中声明并返回能力不可用或 UnsupportedFeature。 |
| `LoopbackOnly` | 只允许本机 loopback。 |
| `Allowlist { hosts: Vec<String> }` | 仅允许指定主机。 |
| `Unrestricted` | 调用方显式允许无限制网络；需在审计记录中标记。 |

### 5. SandboxMount

**Purpose**: 将宿主或临时资源映射到沙箱内路径。

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mount_id` | `String` | Yes | 唯一挂载标识 |
| `host_path` | `PathBuf` | Yes | 宿主路径或后端资源路径 |
| `sandbox_path` | `PathBuf` | Yes | 沙箱内可见路径 |
| `access` | `MountAccess` | Yes | `ReadOnly` 或 `ReadWrite` |
| `persist` | `bool` | Yes | 会话清理后是否保留宿主侧内容 |
| `owner` | `MountOwner` | Yes | `Session`、`Workspace` 或 `User` |

**Validation Rules**:
- `sandbox_path` 不得为空，不得逃逸 `root_dir`。
- 只读挂载下的写入、删除、重命名必须返回权限错误。
- 多个挂载路径重叠时，最长前缀匹配优先；冲突必须在初始化时报错。

### 6. ExecutionRequest

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `argv` | `Vec<String>` | Yes | 命令及参数；不可为空 |
| `cwd` | `Option<PathBuf>` | No | 相对或绝对工作目录，必须在授权范围内 |
| `env` | `HashMap<String, String>` | No | 额外环境变量；敏感值不写入审计明文 |
| `timeout` | `Option<Duration>` | No | 单次命令 timeout，受 policy 限制 |
| `stdin` | `Option<Vec<u8>>` | No | 可选标准输入 |

**Validation Rules**:
- `argv[0]` 不可为空。
- `timeout <= policy.max_timeout`。
- `cwd` 解析后不得越界。

### 7. ExecutionResult

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `execution_id` | `String` | Yes | 单次执行 ID |
| `status` | `ExecutionStatus` | Yes | 成功、非零退出、超时、权限拒绝或系统错误 |
| `exit_code` | `Option<i32>` | No | 进程退出码；超时或 spawn 失败时为空 |
| `stdout` | `OutputSummary` | Yes | stdout 内联摘要和完整输出引用 |
| `stderr` | `OutputSummary` | Yes | stderr 内联摘要和完整输出引用 |
| `started_at` | `DateTime<Utc>` | Yes | 开始时间 |
| `finished_at` | `DateTime<Utc>` | Yes | 结束时间 |
| `duration` | `Duration` | Yes | 执行耗时 |
| `resource_hits` | `Vec<ResourceLimitHit>` | Yes | 命中的限制，如 timeout/output_truncated |

### 8. ExecutionStatus

| Variant | Description |
|---------|-------------|
| `Exited { code: i32 }` | 命令正常结束；code 可为非零 |
| `TimedOut` | 超过时限并已触发终止 |
| `PermissionDenied` | 被路径、挂载或策略拒绝 |
| `UnsupportedFeature` | 请求了当前后端不支持的能力 |
| `SandboxError` | spawn、I/O、后端故障等系统错误 |
| `Cancelled` | 会话 close/reset 或调用方取消导致终止 |

### 9. OutputSummary / OutputRef

| Entity | Field | Type | Description |
|--------|-------|------|-------------|
| `OutputSummary` | `inline` | `Vec<u8>` | 截断后的内联内容 |
| `OutputSummary` | `truncated` | `bool` | 是否因输出上限截断 |
| `OutputSummary` | `full_ref` | `Option<OutputRef>` | 完整输出引用 |
| `OutputRef` | `path` | `PathBuf` | 沙箱内输出文件路径 |
| `OutputRef` | `sha256` | `String` | 完整输出内容哈希 |
| `OutputRef` | `bytes` | `u64` | 完整输出字节数 |

### 10. ExecutionRecord

**Purpose**: 审计历史条目，是 ExecutionResult 的稳定审计视图。

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `sequence` | `u64` | Yes | 会话内单调递增序号 |
| `execution_id` | `String` | Yes | 对应执行 ID |
| `command_summary` | `String` | Yes | 脱敏后的命令摘要 |
| `cwd` | `PathBuf` | Yes | 执行目录 |
| `status` | `ExecutionStatus` | Yes | 执行状态 |
| `duration` | `Duration` | Yes | 耗时 |
| `failure_category` | `Option<String>` | No | 稳定错误类别 |
| `stdout_ref` | `Option<OutputRef>` | No | stdout 完整输出引用 |
| `stderr_ref` | `Option<OutputRef>` | No | stderr 完整输出引用 |

### 11. SandboxWorkspaceBackend

**Purpose**: 将 `SandboxSession` 适配成 `WorkspaceBackend`。

| Field | Type | Description |
|-------|------|-------------|
| `session` | `Arc<dyn SandboxSessionHandle>` | 沙箱会话句柄 |
| `workspace_root` | `PathBuf` | Workspace 视角根目录 |

**Contract Mapping**:
- `exec_shell(cmd, cwd, timeout)` → `SandboxSession::execute(ExecutionRequest)`
- `read_file(path)` → 沙箱文件读取，路径受 mount/policy 校验
- `write_file(path, data)` → 沙箱文件写入，需可写权限
- `delete_path(path)` → 沙箱删除，禁止越界和只读挂载删除
- `list_dir(path, recursive)` → 沙箱目录列表，返回 workspace 可见路径

### 12. CapabilityReport

| Field | Type | Description |
|-------|------|-------------|
| `backend_name` | `String` | 后端名称，如 `local-process`、`docker`、`opensandbox` |
| `compatibility_level` | `CompatibilityLevel` | L1/L2/L3/L4 目标或实际等级 |
| `supported` | `Vec<SandboxCapability>` | 已支持能力 |
| `unsupported` | `Vec<UnsupportedCapability>` | 不支持能力及原因 |
| `known_deviations` | `Vec<String>` | 与 Python AgentScope/OpenSandbox 的已知偏差 |

**Validation Rules**:
- 每个不支持但用户可请求的能力必须出现在 `unsupported` 中。
- 禁止在报告中声称支持无法强制执行的隔离能力。
