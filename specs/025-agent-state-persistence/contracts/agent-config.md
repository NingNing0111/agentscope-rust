# Contract: AgentConfig 接入契约（session_store / session_id / auto_persist）

**Feature**: 025-agent-state-persistence | **Date**: 2026-08-03

本契约定义 ReActAgent 通过 `AgentConfig` 接入持久化的方式。

> **对应 spec**: FR-005 / FR-006 / FR-007

## 新增配置字段（全部可选，向后兼容）

```rust
pub struct AgentConfig {
    // ... 既有字段不变 ...
    /// 会话存储后端。未指定时使用默认 JsonFileSessionStore（目录 sessions/）。
    pub session_store: Option<Arc<dyn SessionStore>>,

    /// 会话标识。指定时构建期从存储加载既有状态；未指定时生成新会话 ID。
    pub session_id: Option<String>,

    /// 是否在 reply 结束后自动持久化。默认 true。
    pub auto_persist: bool,
}
```

Builder 新增方法：

```rust
pub fn session_store(mut self, store: Arc<dyn SessionStore>) -> Self;
pub fn session_id(mut self, id: impl Into<String>) -> Self;
pub fn auto_persist(mut self, enabled: bool) -> Self;
```

**默认值**:

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `session_store` | `None` → 内部用默认 `JsonFileSessionStore::with_default_dir()` | 开箱即用 |
| `session_id` | `None` → 新建会话 | 复用 `AgentState::new()` 的 UUID |
| `auto_persist` | `true` | 自动落盘 |

## 构建时行为

1. 解析 `session_id`：
   - `Some(id)` → 尝试 `session_store.load(id)`：
     - `Ok(session)` → 以恢复的 `AgentState` 构建 Agent（会话恢复）
     - `Err(NotFound)` → 以 `session_id=id` 新建 `AgentState`（spec Edge Case：不存在的标识 = 新建）
     - `Err(其他)`（损坏/IO）→ 返回 `AgentError`（构建失败，不静默）
   - `None` → `AgentState::new()` 生成新 ID
2. 解析 `session_store`：
   - `Some(store)` → 使用注入的后端
   - `None` → 使用默认 `JsonFileSessionStore`（默认目录 `sessions/`）
3. `auto_persist=false` 时：即使配置了后端也不自动保存（但显式 save/load 仍可用）

## 回复结束后行为（自动持久化）

- **时机**：reply / reply_stream 正常结束、以及被中断/取消时
- **动作**：`session_store.save(&session)` 保存最新状态
- **失败处理**：保存失败返回/上报 `AgentError`（包装 `SessionError`），但**不中断**已完成的推理循环结果（spec FR-006）
- **关闭开关**：`auto_persist=false` 时跳过保存，零磁盘写入（spec FR-007 / SC-007）

## 错误映射（AgentError）

| 场景 | 错误 |
|------|------|
| 构建期加载会话失败（损坏） | `AgentError`（含 `SessionError::SerializationError` 根因） |
| 自动保存失败 | `AgentError`（含 `SessionError::StorageError` 根因），不阻断回复结果 |
| 配置非法（如无法创建默认存储目录） | `AgentError::InvalidConfig` |

## 兼容性保证

- 既有构建代码（未设置任何新字段）行为**完全不变**：`session_store=None` 但 `auto_persist` 默认 true 时……**注意**：为保证向后兼容，默认 `auto_persist=true` 会引入磁盘写入。

**设计决策**：`auto_persist` 默认 `true`（spec Assumption：自动持久化默认开启）。但为**不改变既有行为**，需实现以下一条：

> **选项 A（推荐）**：`session_store` 为 `None` 且用户未显式启用持久化时，构建 Agent 内部仍创建默认 `JsonFileSessionStore`，因此自动落盘对既有调用同样生效（即默认启用持久化，与 spec Assumption 一致）。
>
> **选项 B**：`session_store=None` 表示完全禁用持久化，仅当显式注入 store 时自动落盘才生效——但这与 spec "默认启用"矛盾。

**本特性采用选项 A**：默认 `JsonFileSessionStore`（`sessions/` 目录）为开箱即用行为，`auto_persist` 可显式关闭。既有示例若无需落盘，需显式设置 `auto_persist(false)` 或忽略（落盘是安全副作用）。

## 验收要点

- [ ] 未设置任何新字段的既有代码可正常构建运行（默认启用 JSON 文件落盘，或按选项决策）
- [ ] 指定 `session_id` 时从存储恢复完整状态；不存在则新建
- [ ] reply 结束后状态落盘；`auto_persist(false)` 时零写入
- [ ] 保存失败上报但不破坏回复结果
- [ ] 显式注入自定义 `SessionStore` 后端后，行为与内置后端一致
