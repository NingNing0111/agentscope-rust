# 参考:会话管理(`agent_scope_state`)

> 详细 API 参考:`Session` trait、`SessionImpl`、`AgentState`、`SessionStore`、上下文裁剪(`TokenCounter`/`TrimStrategy`)、权限上下文。

## 1. `Session` trait 与 `SessionImpl`

`Session` 是会话的抽象接口:

| 方法 | 说明 |
|------|------|
| `session_id()` | 唯一 session ID |
| `meta()` | `SessionMeta`(创建时间、状态、user_id) |
| `state()` | `&AgentState` 只读引用 |
| `state_mut()` | `&mut AgentState` 可变引用 |
| `cancel_token()` | `CancellationToken`,用于传播取消 |
| `close()` | 关闭会话,cancel 所有子任务 |

`SessionImpl::new(meta)` 是内置实现。

```rust
use agent_scope_state::{Session, SessionImpl, SessionMeta};

let meta = SessionMeta::new("user-001");
let mut session = SessionImpl::new(meta);
```

## 2. `AgentState`

每个 Session 持有一个 `AgentState`:

| 字段 | 类型 | 说明 |
|------|------|------|
| `context` | `Vec<Msg>` | 对话上下文消息列表 |
| `summary` | `SummaryContent` | 被压缩消息的摘要(`Text` 或 `BulletPoints`) |
| `reply_context` | `ReplyContext` | 当前 reply 上下文(reply_id、tool_call 状态等) |
| `permission_context` | `PermissionContext` | 工具权限上下文 |

关键方法:

- `append_context(msg)` — 追加消息,返回结果。
- `get_messages_for_model()` — 获取适合发给模型的消息列表(含摘要注入)。

## 3. `SessionStore`

| 方法 | 说明 |
|------|------|
| `create(meta)` | 创建并存储新会话 |
| `get(session_id)` | 按 ID 加载 |
| `list()` | 列出所有会话 meta |
| `update_meta(session_id, meta)` | 更新元数据 |
| `delete(session_id)` | 删除会话及其状态 |
| `save_state(session_id, state)` | 持久化 AgentState |
| `load_state(session_id)` | 加载已持久化的 AgentState |

`InMemorySessionStore::new()` 是内置实现。

```rust
use agent_scope_state::{InMemorySessionStore, SessionStore};
use agent_scope_message::factory::user_msg;

let store = InMemorySessionStore::new();
store.create(session.meta().clone())?;
session.state_mut().append_context(user_msg("user-001", "Hello!")?)?;
store.save_state(session.session_id(), session.state())?;

let loaded = store.get(session.session_id())?;
let state = store.load_state(session.session_id())?;
```

## 4. 上下文裁剪

| 类型 | 说明 |
|------|------|
| `TokenCounter` | token 计数 trait;`SimpleTokenCounter` 基于字符数估算 |
| `TrimStrategy` | `KeepLast(n)` 或 `TailPercent(p)` |
| `trim_context(state, strategy, counter)` | 执行裁剪,保留 `SummaryContent` |

```rust
use agent_scope_state::{trim_context, TrimStrategy, SimpleTokenCounter};

let counter = SimpleTokenCounter::default();
let strategy = TrimStrategy::TailPercent(0.7); // 保留后 70%
trim_context(&mut state, &strategy, &counter);
// state.summary 会包含裁剪说明
```

## 5. 结构化并发隔离(CancellationToken)

每个 `SessionImpl` 内部持有 `CancellationToken`:

```rust
let token = session.cancel_token().clone();
tokio::spawn(async move {
    tokio::select! {
        _ = token.cancelled() => { /* 清理资源 */ }
        result = long_running_task() => { /* 正常完成 */ }
    }
});
session.close(); // 触发所有子任务的 cancel
```

## 6. 错误

| 错误 | 原因 |
|------|------|
| `SessionError::Closed` | 对已关闭 session 操作 |
| `SessionError::AlreadyExists` | 重复 session ID |
| `SessionError::NotFound` | store 中不存在 |
| `SessionError::SerializationError` | 状态序列化失败 |
| `SessionError::StorageError` | 存储后端错误 |
| `SessionError::InvalidTrimConfig` | 裁剪配置非法(如百分数 > 1.0) |

## 7. 不支持的能力

- 仅内置 `InMemorySessionStore`;持久化存储(SQLite、Redis)需自定义实现。
- 分布式 session 共享不在范围。
- 无自动 session 过期/TTL 机制。
