# Research: Agent 状态持久化（内置 JSON 文件存储 + 可插拔存储后端）

**Feature**: 025-agent-state-persistence | **Date**: 2026-08-03

## 1. 上游基线锁定

**Decision**: 兼容基线锁定为本地 Python AgentScope 参考实现。

**Rationale**: 宪法第二条要求兼容目标绑定不可变上游版本。本地 `agentscope/` 即参考实现源码。

**验证记录**:
- 版本：`agentscope/src/agentscope/_version.py` → `__version__ = "2.0.5"`
- 兼容基线 commit：仓库 HEAD `6698d98`（v2.0.5 基线）
- Python 要求：`pyproject.toml` `requires-python = ">=3.11"`
- 相关模块：`app/storage/_base.py`、`app/storage/_model/_session.py`、`state/_state.py`

**Alternatives considered**: 无——仓库内已有 Python 参考实现 checkout，直接以其为基准。

---

## 2. Python 参考实现的会话存储语义（行为基准）

**Decision**: 以 Python `StorageBase` 的会话接口语义为对齐目标。

**Rationale**: 宪法第一条/第三条要求外部可观察行为与 Python 参考实现一致；`StorageBase` 是 Python 端会话持久化的统一抽象。

**Python 会话接口（`app/storage/_base.py`）**:

| Python 方法 | 语义 | Rust 对应（既有 `SessionStore`） |
|-------------|------|--------------------------------|
| `upsert_session(user_id, agent_id, config, state, ...)` | 创建或更新会话（含初始 state） | `save(&dyn Session)`（幂等 upsert） |
| `update_session_state(user_id, agent_id, session_id, state)` | 更新会话运行时状态 | `save(&dyn Session)`（保存时含状态） |
| `get_session(user_id, agent_id, session_id)` | 按 ID 加载会话，缺失返回 None | `load(id)` → `SessionError::NotFound` |
| `delete_session(user_id, agent_id, session_id)` | 删除会话，返回 bool | `delete(id)`（幂等） |
| `list_sessions(user_id, agent_id)` | 列出会话 | `list_ids()` / `list_meta()` |

**Python 会话记录结构（`_model/_session.py`）**:

```text
SessionRecord
  ├── metadata: id, created_at, updated_at, user_id, agent_id, source, team_id
  ├── config: workspace_id, name, model configs, knowledge config
  └── state: AgentState
        ├── session_id, summary, context: list[Msg]
        ├── reply_context, permission_context, tool_context, tasks_context, middle_context
```

**Python `AgentState` 字段（`state/_state.py:176`）**: `session_id / summary / context / reply_context / permission_context / tool_context / tasks_context / middle_context` —— 与 Rust `agent_scope_state::AgentState` 字段**逐一对应**（见 data-model.md）。

**关键结论**: Rust 端已有 `SessionStore` trait 且方法语义与 Python `StorageBase` 对齐；唯一缺口是**缺少持久化后端实现**（现有 `InMemorySessionStore` 纯内存）与 **ReActAgent 未接入存储**。

**Alternatives considered**: 无——直接复用既有 `SessionStore`，不新建平行接口。

---

## 3. 内置 JSON 文件后端：文件布局与格式

**Decision**: 每会话一个 `{session_id}.json` 文件，单文件内联 `SessionMeta` 与完整 `AgentState`；默认目录为工作区 `sessions/`（可配置）。

**Rationale**:
- Python 参考实现无文件落盘后端（Redis/SQL 均需外部服务）；Rust 库需要"开箱即用"本地持久化，文件后端是最低依赖方案（spec Assumption）。
- 每会话一文件：易于浏览/备份/单会话删除，天然避免单文件随会话数膨胀；example `pi-rust` 已有每会话文件先例。
- 单文件内联元数据 + 状态：`load` 一次性反序列化完整记录，`list_meta` 只需读元数据字段（可仅解析文件头部或反序列化轻量结构），对齐 Python `SessionRecord` 结构。

**文件 JSON 结构（对齐 Python `SessionRecord` 逻辑结构，Rust 侧按需裁剪）**:

```json
{
  "session_id": "uuid-or-user-supplied",
  "status": "Active",
  "message_count": 12,
  "created_at": "2026-08-03T08:00:00Z",
  "last_active": "2026-08-03T08:15:00Z",
  "state": {
    "session_id": "uuid-or-user-supplied",
    "summary": {},
    "context": [],
    "reply_context": { "reply_id": "", "cur_iter": 0 },
    "permission_context": {},
    "tool_context": { "max_cache_files": 100, "max_cache_bytes": 25000 },
    "tasks_context": { "tasks": [] },
    "middle_context": {}
  }
}
```

**原子写入**: 写 `{session_id}.json.tmp` → `fsync` → `rename` 到 `{session_id}.json`。崩溃不留下半写文件（spec FR-004 / Edge Case）。

**Alternatives considered**:
- 单文件集合（`sessions.json`）——会话数增长后全量读写、删除需重写整文件，放弃。
- 每会话子目录（`{id}/state.json + meta.json`）——文件数翻倍、浏览不便，放弃。

---

## 4. ReActAgent 接入方式

**Decision**: 增量接入。`AgentConfig` 新增三个可选字段，构建时加载、reply 结束后保存。

**Rationale**:
- spec 用户决策 1（自动落盘 + 恢复）要求 Agent 运行时可注入后端与会话标识、reply 后自动持久化。
- 增量式（新增可选字段）保证既有调用完全向后兼容（spec Assumption / 宪法第十六条）。

**设计**:
- `AgentConfig.session_store: Option<Arc<dyn SessionStore>>` —— 未指定时使用默认 `JsonFileSessionStore`（默认目录 `sessions/`）；为 `None` 且 `auto_persist=false` 时行为与当前完全一致。
- `AgentConfig.session_id: Option<String>` —— 指定时构建期从存储加载既有状态（存在则恢复，不存在则新建）；未指定时生成新 ID。
- `AgentConfig.auto_persist: bool`（默认 true）—— 控制 reply 结束后是否自动保存；false 时零磁盘写入。
- 保存时机：reply 正常结束、以及被中断/取消时；保存失败经 `AgentError` 上报但不中断推理循环（spec FR-006）。

**模式复用**: `FileMemory::new(workdir, config, backend: Option<Arc<dyn Backend>>)` 的"可选注入 + 默认实现"模式（探索确认）；`AgentConfig` builder 既有 `Option<Arc<dyn ChatModel>>` 注入先例。

**Alternatives considered**:
- 仅存储 API 不接入 Agent——不满足用户决策 1，放弃。
- 存储层 + Agent 显式 save/load 方法但不自动落盘——用户已明确选择自动落盘，放弃。

---

## 5. 自定义后端扩展点

**Decision**: `SessionStore` trait 本身即自定义后端（SQLite/MySQL/Redis 等）的唯一扩展点；不提供这些后端的实现、不加入内置配置枚举。

**Rationale**: spec 用户决策 2（仅 trait 扩展点）。复用既有 `SessionStore`，开发者实现该 trait 即可接入，无需改框架代码（spec FR-008 / SC-004）。

**Alternatives considered**:
- 配置枚举变体（`SessionStoreConfig::Sqlite { dsn }` 等）返回 `UnsupportedFeature`——用户明确选择"仅 trait 扩展点"，放弃。

---

## 6. 错误模型

**Decision**: 复用既有 `SessionError`，I/O 错误映射为 `StorageError` 保留根因。

**Rationale**: 宪法第十三条要求稳定错误模型。既有 `SessionError` 已含 `NotFound` / `SerializationError` / `StorageError`，满足需求，零新增错误类型。

**映射**:
- 文件不存在 → `SessionError::NotFound`
- JSON 解析失败 / 损坏文件 → `SessionError::SerializationError`（含 session_id + reason）
- 文件读写 I/O 错误 / 目录创建失败 → `SessionError::StorageError`（含 session_id + reason）
- 非法会话标识（路径穿越字符）→ 校验失败返回 `StorageError` 或专门的 ValidationError（见 Edge Case 契约）

**Alternatives considered**: 新增独立 `StateStoreError` 类型——增加 API 表面积，既有 `SessionError` 已充分，放弃。

---

## 7. 并发与一致性

**Decision**: 保存/加载为原子短操作，同会话并发安全。

**Rationale**: 宪法第十条（结构化并发）禁止无归属后台任务；保存为 reply 结束时的同步 await，无后台写入。原子 rename 保证并发读者要么看到旧文件要么看到新文件，不出现半写。

**Alternatives considered**: 引入写队列/后台 flush——增加复杂度与孤儿任务风险，会话写为低频操作无需背压，放弃。

---

## 8. 序列化稳定性

**Decision**: 复用 `AgentState` 既有 serde 布局（字段零变更），新增外层记录结构 `SessionRecord`（含 meta + state）。

**Rationale**: 宪法第十二条要求稳定数据协议。`AgentState` 字段已全部 `#[serde(default)]`，未知字段/缺省字段向后兼容；新增 `SessionRecord` 作为存储外壳不影响既有 `AgentState` 序列化。

**Alternatives considered**: 修改 `AgentState` 结构——破坏既有序列化布局，放弃。
