# Contract: SessionStore 接口语义（自定义后端扩展点）

**Feature**: 025-agent-state-persistence | **Date**: 2026-08-03

本契约定义 `SessionStore` trait 的语义，作为自定义存储后端（SQLite、MySQL、Redis 等）的接入规范。开发者实现本接口即可让 Agent 状态持久化走自有后端，无需修改框架代码。

> **对应 spec**: FR-001 / FR-008 / FR-009 / FR-012 / FR-013

## 接口定义

```rust
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// 持久化会话完整状态。幂等 upsert——同一 ID 重复保存即覆盖。
    async fn save(&self, session: &dyn Session) -> Result<(), SessionError>;

    /// 按 ID 加载会话。缺失返回 `SessionError::NotFound`。
    async fn load(&self, id: &str) -> Result<SessionImpl, SessionError>;

    /// 删除会话。幂等——ID 不存在不报错。
    async fn delete(&self, id: &str) -> Result<(), SessionError>;

    /// 列出所有持久化会话 ID。
    async fn list_ids(&self) -> Result<Vec<String>, SessionError>;

    /// 列出会话元数据（轻量，不加载完整状态），按 last_active 降序。
    async fn list_meta(&self) -> Result<Vec<SessionMeta>, SessionError>;
}
```

`Session` / `SessionImpl` / `SessionMeta` / `SessionError` 定义见 `crates/agent_scope_state/src/session.rs`。

## 语义契约

### 1. 保存（upsert）

- **输入**: `&dyn Session`（含 `AgentState` 与生命周期元数据）
- **行为**: 以 `session.id()` 为主键保存完整状态；同一 ID 再次保存必须**覆盖**旧值（不报错、不重复）
- **对齐**: Python `StorageBase.upsert_session` + `update_session_state`

### 2. 加载

- **输入**: 会话 ID
- **成功**: 返回 `SessionImpl`，其中 `AgentState` 必须**全字段无损**（session_id、summary、context、reply/permission/tool/tasks/middle context）
- **失败**: 缺失返回 `SessionError::NotFound { session_id }`；数据损坏返回 `SessionError::SerializationError`
- **对齐**: Python `StorageBase.get_session`（缺失返回 None → Rust 以 NotFound 表达）

### 3. 删除

- **行为**: 幂等——ID 不存在也返回 `Ok(())`（不报错）
- **对齐**: Python `StorageBase.delete_session`

### 4. 列出

- `list_ids`: 返回所有会话 ID（顺序不要求）
- `list_meta`: 返回 `Vec<SessionMeta>`，按 `last_active` 降序；**不得**要求反序列化每个会话的完整 `AgentState`
- **对齐**: Python `StorageBase.list_sessions`

## 错误契约

| 场景 | 返回错误 | 说明 |
|------|----------|------|
| 会话不存在（load） | `SessionError::NotFound` | 携带 session_id |
| JSON 解析失败 / 数据损坏 | `SessionError::SerializationError` | 携带 session_id + reason |
| 底层 I/O / 连接失败 | `SessionError::StorageError` | 携带 session_id + reason，保留根因 |
| 会话标识非法 | `SessionError::StorageError`（或 ValidationError） | 携带 reason，防止路径穿越 |

错误必须类型明确（typed），禁止依赖字符串内容判断错误类型（宪法第十三条 / spec FR-012）。

## 自定义后端实现指南（SQLite / MySQL 示例）

### SQLite

- **表结构建议**（对齐 `SessionRecordFile` 字段）:
  ```sql
  CREATE TABLE sessions (
      session_id   TEXT PRIMARY KEY,
      status       TEXT NOT NULL,
      message_count INTEGER NOT NULL,
      created_at   TEXT NOT NULL,
      last_active  TEXT NOT NULL,
      state_json   TEXT NOT NULL
  );
  ```
- `save` → `INSERT ... ON CONFLICT(session_id) DO UPDATE SET ...`（upsert）
- `load` → `SELECT * FROM sessions WHERE session_id = ?`；无行返回 `NotFound`
- `list_meta` → `SELECT session_id, status, message_count, created_at, last_active FROM sessions ORDER BY last_active DESC`
- `state_json` 列存 `AgentState` 的 `serde_json` 序列化结果

### MySQL

- 同上表结构，`state_json` 用 `LONGTEXT` / `JSON` 列
- upsert 用 `INSERT ... ON DUPLICATE KEY UPDATE ...`
- 其余语义与 SQLite 一致

### 通用注意

- 必须满足 `Send + Sync`（跨线程共享）
- 会话标识作为主键；同名覆盖
- 不泄露 API Key 等敏感信息到错误信息中

## 验收要点

- [ ] 实现本接口的后端通过 `save → load` 往返，`AgentState` 全字段无损
- [ ] 同一 ID 重复 save 后 load 得到最新状态（upsert）
- [ ] load 不存在会话返回 `NotFound`；delete 不存在会话幂等
- [ ] list_meta 不加载完整状态，按 last_active 降序
- [ ] 错误均返回 typed `SessionError`，无字符串匹配判错
