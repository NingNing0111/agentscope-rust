# 参考:会话管理(`agent_scope_state`)

> 详细 API 参考:`Session` trait、`SessionImpl`、`AgentState`、`SessionStore`、`InMemorySessionStore`、`JsonFileSessionStore`、`SqliteSessionStore`、上下文裁剪(`TokenCounter`/`TrimStrategy`)。

## 1. `Session` trait 与 `SessionImpl`

`Session` 是一次对话运行状态的抽象包装。它持有 `AgentState`、生命周期状态、创建/活跃时间戳,并用 `CancellationToken` 做结构化取消。

| 方法 | 说明 |
|------|------|
| `id()` | 唯一 session ID,来自 `AgentState.session_id` |
| `status()` | 当前状态:`SessionStatus::Active` / `Closed` |
| `state()` | `&AgentState` 只读引用 |
| `state_mut()` | `&mut AgentState` 可变引用;调用时会自动 `touch()` |
| `close()` | 幂等关闭 session,并取消内部 token |
| `is_closed()` | 是否已关闭 |
| `created_at()` | 创建时间 |
| `last_active()` | 最近活跃时间 |
| `touch()` | 更新 `last_active` |

构造方式:

```rust
use agent_scope_state::{AgentState, Session, SessionImpl};

let mut session = SessionImpl::with_session_id("s1".into());
assert_eq!(session.id(), "s1");

session
    .state_mut()
    .append_context(
        "assistant",
        vec![agent_scope_message::ContentBlock::Text(
            agent_scope_message::TextBlock::new("hello".into()),
        )],
    )?;

session.close().await?;
assert!(session.is_closed());

let state = AgentState::with_session_id("restored".into());
let session = SessionImpl::new(state);
```

持久化后重新加载时,store 会用 `with_persisted_timestamps(created_at, last_active)` 恢复原始时间戳,避免列表排序被加载时间污染。

## 2. `AgentState`

`AgentState` 是实际可序列化的 Agent 运行状态:

| 字段 | 类型 | 说明 |
|------|------|------|
| `session_id` | `String` | 会话 ID;默认自动生成 |
| `context` | `Vec<Msg>` | 对话上下文消息 |
| `summary` | `SummaryContent` | 被压缩上下文的摘要 |
| `reply_context` | `ReplyContext` | 当前/最近一次 reply 的 block、tool-call 状态 |
| `permission_context` | `PermissionContext` | 工具权限规则 |
| `tool_context` | `ToolContext` | 文件读取缓存、激活工具组等工具运行上下文 |
| `tasks_context` | `TaskContext` | 任务/子任务状态 |
| `middle_context` | `HashMap<String, Value>` | middleware 共享状态 |
| `tool_failures` | `HashMap<String, u32>` | 连续工具失败计数 |
| `max_context_messages` | `Option<usize>` | 上下文消息上限 |

常用方法:

| 方法 | 说明 |
|------|------|
| `new()` / `with_session_id(id)` | 创建状态 |
| `append_context(name, blocks)` | 把 assistant 内容块追加进上下文;可与尾部同名 assistant 消息合并 |
| `context_length()` | 当前消息数 |
| `set_max_context_messages(n)` | 设置消息上限 |
| `has_awaiting_tool_calls(name)` | 检测指定 agent 是否还有 asking/submitted 工具调用未完成 |
| `clean_file_cache()` | 清理工具文件缓存 |
| `from_legacy_json(json)` | 从旧版 JSON 做兼容迁移 |

消息通常用 `agent_scope_message::factory::{user_msg, assistant_msg, system_msg}` 构造后直接 push 到 `state.context`;`append_context` 更适合追加 assistant 内容块。

## 3. `SessionStore`

`SessionStore` 是持久化后端 trait:

| 方法 | 语义 |
|------|------|
| `save(&dyn Session)` | upsert,保存完整 `AgentState` 和轻量 meta |
| `load(id)` | 加载为 `SessionImpl`;不存在返回 `SessionError::NotFound` |
| `delete(id)` | 幂等删除;不存在也返回 `Ok(())` |
| `list_ids()` | 列出全部 session ID |
| `list_meta()` | 只列轻量 meta,按 `last_active` 倒序 |

内置实现:

| Store | 适用场景 | 特点 |
|-------|----------|------|
| `InMemorySessionStore` | 单进程测试/临时运行 | 内存 HashMap,进程退出即丢失 |
| `JsonFileSessionStore` | 默认本地持久化 | 每个 session 一个 `{id}.json`,原子写入 |
| `SqliteSessionStore` | 单文件持久化、需要索引列表 | SQLite 表存 meta 列 + 完整 state JSON,atomic upsert |

### 3.1 In-memory store

```rust
use agent_scope_state::{InMemorySessionStore, SessionImpl, SessionStore};

let store = InMemorySessionStore::new();
let session = SessionImpl::with_session_id("s1".into());

store.save(&session).await?;
let loaded = store.load("s1").await?;
assert_eq!(loaded.id(), "s1");
store.delete("s1").await?;
```

### 3.2 JSON file store

```rust
use agent_scope_state::{JsonFileSessionStore, SessionImpl, SessionStore};

let store = JsonFileSessionStore::new("./sessions");
let session = SessionImpl::with_session_id("s1".into());

store.save(&session).await?;
let ids = store.list_ids().await?;
let meta = store.list_meta().await?;
```

磁盘记录形态等价于:

```json
{
  "session_id": "s1",
  "status": "Active",
  "message_count": 12,
  "created_at": "2026-08-25T00:00:00Z",
  "last_active": "2026-08-25T00:01:00Z",
  "state": { "...": "AgentState JSON" }
}
```

JSON store 会校验 session id,拒绝空字符串、`.`、路径分隔符(`/`/`\`),避免路径遍历。

### 3.3 SQLite store

`SqliteSessionStore` 适合把 session 都放在一个数据库文件里,同时快速列出 meta:

```rust
use agent_scope_state::{SessionImpl, SessionStore, SqliteSessionStore};

let store = SqliteSessionStore::connect("./sessions.sqlite").await?;
let session = SessionImpl::with_session_id("s1".into());

store.save(&session).await?;
let loaded = store.load("s1").await?;
let metas = store.list_meta().await?;
```

测试或临时运行可用内存库:

```rust
let store = SqliteSessionStore::connect_in_memory().await?;
```

高级用法:如果应用已有 `sqlx::SqlitePool`,用 `SqliteSessionStore::from_pool(pool).await?` 包装并初始化 schema。schema 为 `sessions` 表,包含 `session_id`、`status_json`、`message_count`、`created_at`、`last_active`、`state_json`,并为 `last_active` 建索引。

## 4. `SessionMeta` 与状态

`SessionMeta` 是列表视图:

| 字段 | 说明 |
|------|------|
| `session_id` | 会话 ID |
| `status` | `Active` / `Closed` |
| `message_count` | `AgentState.context_length()` |
| `created_at` | 创建时间 |
| `last_active` | 最近活跃时间 |

`SessionStatus` 只有两种:`Active` 和 `Closed`。关闭 session 会取消 token,但是否立刻保存关闭状态取决于调用方是否随后 `store.save(&session).await`。

## 5. 上下文裁剪

`trim_context(state, strategy, counter)` 用于裁剪过长上下文,并把裁剪说明写入 `state.summary`。

常用类型:

| 类型 | 说明 |
|------|------|
| `TokenCounter<'a>` | token 计数函数类型:`dyn Fn(&[Msg]) -> usize` |
| `TrimStrategy` | 结构体配置:`max_messages`、`max_tokens`、`keep_recent`、`keep_system_messages` |
| `TrimResult` | 裁剪前后消息数/token 数等结果 |

示意:

```rust
use agent_scope_state::{trim_context, AgentState, TrimStrategy};

let mut state = AgentState::new();
let strategy = TrimStrategy {
    max_messages: Some(100),
    max_tokens: Some(8_000),
    keep_recent: 20,
    keep_system_messages: true,
};
let counter = |messages: &[agent_scope_message::Msg]| -> usize {
    messages
        .iter()
        .map(|msg| msg.get_text_content("\n").unwrap_or_default().len() / 4)
        .sum()
};

if let Some(result) = trim_context(&mut state, &strategy, Some(&counter)) {
    println!(
        "messages: {} -> {}",
        result.messages_before,
        result.messages_after
    );
}
```

具体字段以 `crates/agent_scope_state/src/trim.rs` 为准;不同策略都会尽量保留工具调用链的完整性,避免留下孤立的 tool call / tool result。

## 6. 结构化取消

`SessionImpl::cancel_token()` 返回 child token,适合给 session-scoped 后台任务:

```rust
let token = session.cancel_token();
tokio::spawn(async move {
    tokio::select! {
        _ = token.cancelled() => { /* 清理资源 */ }
        result = long_running_task() => { /* 正常完成 */ }
    }
});

session.close().await?; // 触发 token.cancelled()
```

即使 `SessionImpl` 被 drop 而没有显式 close,内部 token 也会取消,防止后台任务无限悬挂。

## 7. 错误

| 错误 | 原因 |
|------|------|
| `SessionError::Closed` | 对已关闭 session 执行不允许的操作 |
| `SessionError::AlreadyExists` | 保留错误类型;当前内置 store 的 `save` 是 upsert |
| `SessionError::NotFound` | `load(id)` 找不到 session |
| `SessionError::SerializationError` | `AgentState` / meta 序列化或反序列化失败 |
| `SessionError::StorageError` | 文件/数据库/校验等存储层错误 |
| `SessionError::InvalidTrimConfig` | 裁剪配置非法 |

## 8. 当前边界

- Store 不自动做 TTL、压缩、归档或跨进程锁编排。
- SQLite store 负责 schema 初始化和 upsert,但高并发跨进程写入策略由 SQLite 自身与调用方连接池配置决定。
- `SessionStore` 与 `ReActAgent` 当前不是自动绑定关系;应用层负责在合适时机 save/load session。
