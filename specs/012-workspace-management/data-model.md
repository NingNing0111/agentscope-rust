# Data Model: Workspace Management

**Feature**: 012-workspace-management | **Date**: 2026-07-31

## Entity Relationship

```text
┌─────────────────────┐
│   WorkspaceBackend  │ (trait)
│   ├─ exec_shell()   │
│   ├─ read_file()    │
│   ├─ write_file()   │
│   ├─ is_dir()       │
│   ├─ list_dir()     │
│   ├─ delete_path()  │
│   ├─ file_exists()  │
│   ├─ join_path()    │
│   ├─ basename()     │
│   ├─ dirname()      │
│   └─ stat_mtime()   │
└──────────┬──────────┘
           │ implements
┌──────────▼──────────┐
│    LocalBackend     │ (struct)
│  (filesystem I/O)   │
└─────────────────────┘

┌──────────────────────────────────────────────┐
│             WorkspaceBase (trait)             │
│  fields:                                      │
│    workspace_id: String                       │
│    workdir: String                            │
│    is_alive: bool                             │
│  methods:                                     │
│    initialize(), close(), reset()             │
│    list_tools(), get_backend()                │
│    list_mcps(), add_mcp(), remove_mcp()       │
│    list_skills(), add_skill(), remove_skill() │
│    offload_context(), offload_tool_result()   │
│    get_instructions()                         │
└──────┬───────────────────────────────────────┘
       │ implements
┌──────▼───────────────────────────────────────┐
│           LocalWorkspace (struct)             │
│  extra fields:                                │
│    instructions: String                       │
│    default_mcps: Vec<McpClientConfig>          │
│    skill_paths: Vec<String>                   │
│  private state:                               │
│    _backend: LocalBackend                     │
│    _mcps: Vec<McpClientConfig>                │
│    _skill_mgr: SkillManager                   │
│    _mcp_lock: tokio::sync::Mutex<()>          │
│    _skill_lock: tokio::sync::Mutex<()>        │
└──────────────────────────────────────────────┘
```

## Entity Definitions

### 1. WorkspaceBackend (trait)

**Purpose**: 文件系统和进程 I/O 的抽象接口，供 workspace 和内置工具统一使用。

**Fields/Methods**:

| 方法 | 签名 | 说明 |
|------|------|------|
| `exec_shell` | `async fn exec_shell(&self, cmd: &[&str], cwd: &str) -> Result<ExecOutput, WorkspaceError>` | 执行 shell 命令并返回 stdout/stderr/exit_code |
| `read_file` | `async fn read_file(&self, path: &str) -> Result<Vec<u8>, WorkspaceError>` | 读取文件全部内容 |
| `write_file` | `async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), WorkspaceError>` | 写入文件（自动创建父目录） |
| `is_dir` | `async fn is_dir(&self, path: &str) -> Result<bool, WorkspaceError>` | 判断路径是否为目录 |
| `list_dir` | `async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, WorkspaceError>` | 列出目录内容，返回全路径 |
| `delete_path` | `async fn delete_path(&self, path: &str) -> Result<(), WorkspaceError>` | 删除文件或递归删除目录 |
| `file_exists` | `async fn file_exists(&self, path: &str) -> Result<bool, WorkspaceError>` | 判断文件/目录是否存在 |
| `join_path` | `fn join_path(&self, a: &str, b: &str) -> String` | 拼接路径（同步，纯字符串操作） |
| `basename` | `fn basename(&self, path: &str) -> String` | 提取路径的最后一个组件 |
| `dirname` | `fn dirname(&self, path: &str) -> String` | 提取路径的父目录 |
| `stat_mtime` | `async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, WorkspaceError>` | 获取文件修改时间（Unix 时间戳秒） |

**Validation Rules**:
- `exec_shell` 的 `cmd` 不可为空数组
- `delete_path` 对不存在的路径应返回 `Ok(())`（幂等）
- `write_file` 自动创建父目录

**State Transitions**: 无状态 — Backend 是纯函数映射

---

### 2. ExecOutput (struct)

**Purpose**: shell 命令执行结果。

| Field | Type | Description |
|-------|------|-------------|
| `stdout` | `Vec<u8>` | 标准输出 |
| `stderr` | `Vec<u8>` | 标准错误 |
| `exit_code` | `i32` | 退出码 |

---

### 3. McpClientConfig (struct)

**Purpose**: MCP 客户端配置的序列化表示，持久化到 `.mcp` 文件。

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `String` | Yes | MCP 客户端唯一名称 |
| `transport` | `McpTransportConfig` | Yes | 传输配置（URL / 命令 / 等等） |
| `is_stateful` | `bool` | No (default: true) | 是否需要持久连接 |

**Validation Rules**:
- `name` 不能为空
- 同一 workspace 内 `name` 必须唯一

**Serialization Format**: JSON（与 Python 的 `.mcp` 文件格式兼容）

---

### 4. McpTransportConfig (enum)

**Purpose**: MCP 传输方式的类型安全表示。

| Variant | Fields | Description |
|---------|--------|-------------|
| `Stdio` | `{ command: String, args: Vec<String> }` | 标准输入/输出传输 |
| `Sse` | `{ url: String, headers: HashMap<String,String> }` | Server-Sent Events 传输 |
| `StreamableHttp` | `{ url: String, headers: HashMap<String,String> }` | HTTP 流式传输 |

---

### 5. Skill (struct)

**Purpose**: 可复用的知识/代码模块，基于 SKILL.md 文件描述。

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Agent 可见的 skill 名称 |
| `description` | `String` | 技能描述（来自 SKILL.md frontmatter） |
| `dir` | `String` | Skill 目录的绝对路径 |
| `markdown` | `String` | SKILL.md 的 markdown 正文（不含 frontmatter） |
| `updated_at` | `f64` | SKILL.md 最后修改的 Unix 时间戳 |

**Validation Rules**:
- `name` 不能为空
- `description` 不能为空
- `dir` 必须存在且包含有效的 `SKILL.md`

---

### 6. SkillManager (struct)

**Purpose**: 管理 skills/ 目录的索引和 CRUD 操作。

| Field | Type | Description |
|-------|------|-------------|
| `skills_dir` | `String` | `{workdir}/skills/` 的绝对路径 |
| `backend` | `LocalBackend` | 文件系统操作后端 |
| `index` | `SkillsIndex` | 缓存的 `.skills` 索引文件内容 |

**Methods**:
- `load_index()` — 从 `.skills` 文件加载或创建空索引
- `save_index(data: &SkillsIndex)` — 持久化 `.skills` 文件
- `reconcile()` — 检测 mtime 变化并重建索引
- `add_skill(skill_path: &str)` — 复制+索引新 skill
- `remove_skill(name: &str)` — 按名称删除
- `list_skills()` — 返回所有 Skill 对象
- `validate_skill(skill_path: &str)` — 验证 SKILL.md

---

### 7. SkillsIndex (struct)

**Purpose**: `.skills` 文件的 Rust 表示。

| Field | Type | Description |
|-------|------|-------------|
| `skills_dir_mtime` | `f64` | skills 目录的 mtime（用于变化检测） |
| `skills` | `HashMap<String, SkillEntry>` | dir_name → SkillEntry 的映射 |

### 8. SkillEntry (struct)

| Field | Type | Description |
|-------|------|-------------|
| `hash` | `String` | SKILL.md 内容的 SHA-256 十六进制 |
| `skill_name` | `String` | Agent-facing 名称 |

---

### 9. WorkspaceManager (struct)

**Purpose**: 多租户 workspace 生命周期管理。

| Field | Type | Description |
|-------|------|-------------|
| `entries` | `Arc<RwLock<HashMap<String, ManagerEntry>>>` | key → entry 映射 |
| `factory` | `Box<dyn Fn(String) -> LocalWorkspace + Send + Sync>` | workspace 构造函数 |
| `ttl` | `Option<Duration>` | 空闲淘汰 TTL（None = 永不淘汰） |
| `cleanup_handle` | `Option<JoinHandle<()>>` | 后台清理任务句柄 |

### 10. ManagerEntry (struct)

| Field | Type | Description |
|-------|------|-------------|
| `workspace` | `Arc<dyn WorkspaceBase>` | workspace 实例 |
| `last_access` | `Instant` | 最后访问时间 |

---

### 11. WorkspaceError (enum)

**Purpose**: workspace 操作的所有错误类型。

| Variant | Fields | Description |
|---------|--------|-------------|
| `BackendError` | `{ message: String }` | 后端 I/O 错误 |
| `NotInitialized` | — | workspace 未初始化 |
| `AlreadyInitialized` | — | 重复初始化 |
| `InvalidSkill` | `{ path: String, reason: String }` | SKILL.md 无效 |
| `SkillNotFound` | `{ name: String }` | skill 名称未找到 |
| `McpNotFound` | `{ name: String }` | MCP 未找到 |
| `McpAlreadyExists` | `{ name: String }` | 同名 MCP 已存在 |
| `PathTraversal` | `{ path: String }` | 路径遍历攻击检测 |
| `CorruptMcpFile` | `{ path: String, message: String }` | .mcp 文件损坏 |
| `GatewayError` | `{ message: String }` | 沙箱网关错误（预留） |
| `OffloadError` | `{ message: String }` | 上下文卸载错误 |

## State Transitions

### Workspace Lifecycle

```text
  [Created]
      │
      ▼
  initialize()
      │
      ▼
  [Alive] ◄────────────┐
      │                 │
      │   reset()       │
      ▼                 │
  [Dirty] ──────────────┘
      │
      ▼
  close()
      │
      ▼
  [Closed]
```

- **Created → Alive**: `initialize()` 成功
- **Alive → Dirty**: 用户执行了文件操作/添加 MCP/添加 skill
- **Dirty → Alive**: `reset()` 完成清理
- **Alive → Closed**: `close()` 成功
- **Closed → Alive**: 重新 `initialize()` 从 `.mcp` 恢复配置
