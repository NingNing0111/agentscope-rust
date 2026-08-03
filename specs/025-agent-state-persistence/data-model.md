# Data Model: Agent 状态持久化

**Feature**: 025-agent-state-persistence | **Date**: 2026-08-03

本文档描述本特性涉及的数据实体、字段、关系与校验规则。实体复用既有 `agent_scope_state` 类型为主，新增一个存储记录结构。

## 实体关系总览

```text
SessionStore (trait, 已存在 = 自定义后端扩展点)
  ├── JsonFileSessionStore (新增, 内置实现)
  │     └── 写入/读取: SessionRecordFile  (新增, 每会话一个 {session_id}.json)
  │           ├── SessionMeta   (已存在, 轻量元数据)
  │           └── AgentState    (已存在, 会话运行时状态)
  └── <CustomBackend> (用户实现: Sqlite/MySQL/Redis ...)
```

---

## 1. SessionStore（trait，复用，扩展点）

**来源**: `crates/agent_scope_state/src/session_store.rs`（已存在，零改动）

**定义**:

```rust
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn save(&self, session: &dyn Session) -> Result<(), SessionError>;
    async fn load(&self, id: &str) -> Result<SessionImpl, SessionError>;
    async fn delete(&self, id: &str) -> Result<(), SessionError>;
    async fn list_ids(&self) -> Result<Vec<String>, SessionError>;
    async fn list_meta(&self) -> Result<Vec<SessionMeta>, SessionError>;
}
```

**语义契约**（对齐 Python `StorageBase`）:
- `save`: upsert——同一 ID 重复保存即覆盖
- `load`: 缺失返回 `SessionError::NotFound`
- `delete`: 幂等——ID 不存在不报错
- `list_ids`: 返回所有持久化会话 ID
- `list_meta`: 返回轻量元数据（不加载完整状态），按 `last_active` 降序

**作为自定义后端扩展点**: SQLite/MySQL/Redis 等由开发者实现本 trait 接入，无需改框架代码。

---

## 2. SessionRecordFile（新增，JSON 文件格式外壳）

**来源**: 新增结构，位于 `crates/agent_scope_state/src/json_file_store.rs`，为 `JsonFileSessionStore` 的磁盘格式。

**用途**: 每个持久化会话对应一个 `{session_id}.json` 文件，内容为会话元数据 + 完整状态（对齐 Python `SessionRecord` 逻辑结构：`id/created_at/updated_at/status/message_count/state`）。

**字段**:

| 字段 | 类型 | 来源 | 说明 |
|------|------|------|------|
| `session_id` | `String` | `Session::id()` / `AgentState::session_id` | 会话标识（= 文件名，去掉 `.json`） |
| `status` | `SessionStatus` | `Session::status()` | 会话生命周期状态（Active/Closed） |
| `message_count` | `usize` | `state().context_length()` | 上下文消息数 |
| `created_at` | `DateTime<Utc>` | `Session::created_at()` | 创建时间 |
| `last_active` | `DateTime<Utc>` | `Session::last_active()` | 最后活跃时间 |
| `state` | `AgentState` | `Session::state()` | 完整会话运行时状态（核心载荷） |

**JSON 示例**:

```json
{
  "session_id": "a1b2c3d4",
  "status": "Active",
  "message_count": 5,
  "created_at": "2026-08-03T08:00:00Z",
  "last_active": "2026-08-03T08:15:00Z",
  "state": {
    "session_id": "a1b2c3d4",
    "summary": {},
    "context": [],
    "reply_context": { "reply_id": "", "cur_iter": 0 },
    "permission_context": {},
    "tool_context": { "max_cache_files": 100, "max_cache_bytes": 25000, "read_file_cache": [], "activated_groups": [] },
    "tasks_context": { "tasks": [] },
    "middle_context": {}
  }
}
```

**校验规则**:
- 文件名 = 会话标识 + `.json`；会话标识非法字符（路径分隔符、`.`、空）必须在保存/加载前拒绝，防止路径穿越（spec Edge Case）
- `state.session_id` 与文件名的 `session_id` 必须一致（加载后以文件名为准）
- 损坏/截断/无法解析的 JSON → `SessionError::SerializationError`

---

## 3. AgentState（复用，零改动）

**来源**: `crates/agent_scope_state/src/agent_state.rs`（已存在）

**说明**: 会话的运行时状态，作为持久化核心载荷。已实现 `Serialize`/`Deserialize`，字段与 Python 参考实现 `AgentState`（`state/_state.py`）逐一对应：

| Rust 字段 | Python 字段 | 说明 |
|-----------|-------------|------|
| `session_id: String` | `session_id` | 会话标识 |
| `summary: SummaryContent` | `summary` | 压缩摘要 |
| `context: Vec<Msg>` | `context: list[Msg]` | 对话上下文消息 |
| `max_context_messages: Option<usize>` | （Python 无，Rust 扩展） | 上下文上限 |
| `reply_context: ReplyContext` | `reply_context` | 回复上下文 |
| `permission_context: PermissionContext` | `permission_context` | 权限上下文 |
| `tool_context: ToolContext` | `tool_context` | 工具上下文 |
| `tasks_context: TaskContext` | `tasks_context` | 任务清单上下文 |
| `middle_context: HashMap<String, Value>` | `middle_context: dict` | 中间件共享上下文 |

**校验**: 所有字段 `#[serde(default)]`，旧版本文件（缺新增字段）按默认值兼容加载（宪法第十二条 / spec FR-011）。

---

## 4. SessionMeta（复用，零改动）

**来源**: `crates/agent_scope_state/src/session.rs`

**字段**: `session_id` / `status` / `message_count` / `created_at` / `last_active`（均 `Serialize`/`Deserialize`）。

**用途**: `list_meta` 返回轻量元数据，不要求反序列化完整 `AgentState`（spec FR-010）。在 JSON 文件后端中，`list_meta` 从每个文件的 `SessionRecordFile` 中读取元数据字段（可只解析外层结构）。

---

## 5. JsonFileSessionStore（新增，内置后端）

**来源**: 新增，位于 `crates/agent_scope_state/src/json_file_store.rs`。

**字段**:

| 字段 | 类型 | 说明 |
|------|------|------|
| `dir: PathBuf` | 存储目录（默认工作区 `sessions/`） | 会话文件所在目录 |

**构造**:
- `JsonFileSessionStore::new(dir: impl Into<PathBuf>)` —— 指定存储目录
- `Default`/`with_default_dir()` —— 默认 `sessions/` 目录（相对当前工作目录，可配置）

**行为**:
- `save` → 原子写 `{dir}/{session_id}.json`（临时文件 + rename）
- `load` → 读文件 → 反序列化 `SessionRecordFile` → 组装 `SessionImpl`
- `delete` → 删除文件（幂等，文件不存在不报错）
- `list_ids` → 扫描目录下 `*.json` 文件名
- `list_meta` → 逐个读文件解析外层元数据字段

**并发安全**: 原子 rename 保证读者看到一致文件；目录扫描与文件写分离。

---

## 6. 状态与生命周期

- **会话生命周期**: `Active` → `Closed`（既有 `SessionStatus`，Python 端对应 `running/idle` 等派生状态——本特性只持久化会话记录，不派生运行状态）
- **持久化时机**: reply 正常结束、被中断/取消时自动保存最新状态（spec FR-006）
- **恢复时机**: 构建 Agent 时按 `session_id` 从存储加载（spec FR-005）
- **删除**: 显式 `delete`，幂等

## 关系总结

- `SessionStore`（接口）是自定义后端的**唯一扩展点**
- `JsonFileSessionStore`（内置）实现 `SessionStore`，磁盘格式为 `SessionRecordFile`
- `SessionRecordFile` 组合 `SessionMeta` 字段 + `AgentState`
- `AgentState` / `SessionMeta` / `SessionError` 全部复用既有类型，**零改动**
