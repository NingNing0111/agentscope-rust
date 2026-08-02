# 会话管理 / Session

> 一句话定位：`agent_scope_state` 提供 Agent 运行时状态管理——`Session` 封装 AgentState 生命周期与结构化并发隔离，`AgentState` 维护上下文消息、摘要与 ReplyContext，`SessionStore` 实现会话持久化与跨请求恢复。

## 1. 模块概述 (Overview)

本模块位于 `agent_scope_state` crate 中，为 Agent 提供：

| 组件 | 职责 |
|------|------|
| `Session` / `SessionImpl` | 会话生命周期管理（创建、激活、关闭），CancellationToken 隔离，AgentState 访问 |
| `AgentState` | 上下文消息列表、摘要（`SummaryContent`）、`ReplyContext`、权限上下文 |
| `SessionStore` / `InMemorySessionStore` | 会话持久化存储，支持 CRUD 和跨请求恢复 |
| `TokenCounter` / `TrimStrategy` | 基于 token 估算的上下文裁剪，防止超窗口 |
| `PermissionContext` / `PermissionRule` | 工具调用权限的运行时上下文与规则 |

**适用场景**：多轮对话上下文管理；跨请求恢复会话状态；为不同用户/Tenant 隔离 AgentState；上下文 token 超限时自动裁剪。

**前置阅读**：建议先阅读 [Agent 系统](./agent.md) 和 [消息与基础类型](./message-types.md)。

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 `Session` trait 与 `SessionImpl`

`Session` 是会话的抽象接口：

| 方法 | 说明 |
|------|------|
| `session_id()` | 返回唯一 session ID |
| `meta()` | 返回 `SessionMeta`（创建时间、状态、user_id） |
| `state()` | 返回 `&AgentState` 的只读引用 |
| `state_mut()` | 返回 `&mut AgentState` 的可变引用 |
| `cancel_token()` | 返回 `CancellationToken`，用于传播取消信号 |
| `close()` | 关闭会话，cancel 所有子任务 |

`SessionImpl` 是内置实现，通过 `SessionImpl::new(meta)` 构造。

### 2.2 `AgentState`

每个 Session 持有一个 `AgentState`，包含：

| 字段 | 类型 | 说明 |
|------|------|------|
| `context` | `Vec<Msg>` | 对话上下文消息列表 |
| `summary` | `SummaryContent` | 被压缩消息的摘要（`Text` 或 `BulletPoints`） |
| `reply_context` | `ReplyContext` | 当前 reply 的上下文（reply_id、tool_call 状态等） |
| `permission_context` | `PermissionContext` | 工具权限上下文 |

**关键方法**：
- `append_context(msg)` — 追加消息到上下文，返回结果
- `get_messages_for_model()` — 获取适合发送给模型的消息列表（含摘要注入）

### 2.3 `SessionStore`

| 方法 | 说明 |
|------|------|
| `create(meta)` | 创建并存储新会话 |
| `get(session_id)` | 按 ID 加载会话 |
| `list()` | 列出所有会话的 meta |
| `update_meta(session_id, meta)` | 更新会话元数据 |
| `delete(session_id)` | 删除会话及其状态 |
| `save_state(session_id, state)` | 持久化 AgentState |
| `load_state(session_id)` | 加载已持久化的 AgentState |

### 2.4 上下文裁剪

| 类型 | 说明 |
|------|------|
| `TokenCounter` | token 计数 trait，`SimpleTokenCounter` 基于字符数估算 |
| `TrimStrategy` | 裁剪策略：`KeepLast(n)`、`TailPercent(p)` |
| `trim_context(state, strategy, counter)` | 执行裁剪，保留 `SummaryContent` |

裁剪操作会记录 `SummaryContent::Text` 说明移除了多少条消息。

## 3. 快速示例 (Quick Example)

```rust
use agent_scope_state::{
    SessionImpl, SessionMeta, Session, SessionStore, InMemorySessionStore,
    AgentState,
};

// 创建会话
let meta = SessionMeta::new("user-001");
let mut session = SessionImpl::new(meta);

// 向上下文追加消息
use agent_scope_message::factory::user_msg;
session.state_mut().append_context(
    user_msg("user-001", "Hello!").unwrap()
).unwrap();

// 持久化到 store
let store = InMemorySessionStore::new();
store.create(session.meta().clone())?;
store.save_state(session.session_id(), session.state())?;

// 从 store 恢复
let loaded = store.get("session-id")?;
let state = store.load_state("session-id")?;
```

## 4. 关键用法模式 (Usage Patterns)

### 4.1 上下文裁剪

当上下文超过模型窗口时，使用裁剪策略：

```rust
use agent_scope_state::{trim_context, TrimStrategy, SimpleTokenCounter};

let counter = SimpleTokenCounter::default();
let strategy = TrimStrategy::TailPercent(0.7); // 保留后 70%
trim_context(&mut state, &strategy, &counter);
// state.summary 会包含裁剪说明
```

### 4.2 权限检查

```rust
use agent_scope_state::{PermissionContext, PermissionRule};

let ctx = PermissionContext::builder()
    .rule(PermissionRule::AllowTool { tool_name: "read".into() })
    .rule(PermissionRule::DenyTool { tool_name: "delete".into() })
    .build();
// 注入 AgentConfig 后，Agent 在每次工具执行前检查权限
```

### 4.3 结构化并发隔离

每个 `SessionImpl` 内部持有 `CancellationToken`：

```rust
let token = session.cancel_token().clone();
tokio::spawn(async move {
    tokio::select! {
        _ = token.cancelled() => {
            // 清理资源
        }
        result = long_running_task() => {
            // 正常完成
        }
    }
});
session.close(); // 触发所有子任务的 cancel
```

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误 | 原因 | 处理建议 |
|------|------|----------|
| `SessionError::Closed` | 对已关闭 session 操作 | 创建新 session |
| `SessionError::AlreadyExists` | 重复 session ID | 使用不同 ID |
| `SessionError::NotFound` | store 中不存在 | 检查 ID 是否正确 |
| `SessionError::SerializationError` | 状态序列化失败 | 检查状态数据结构 |
| `SessionError::StorageError` | 存储后端错误 | 检查存储可用性 |
| `SessionError::InvalidTrimConfig` | 裁剪配置非法（如百分数 > 1.0） | 修正配置 |

**不支持的能力**：
- 当前仅内置 `InMemorySessionStore`；持久化存储（SQLite、Redis）需自定义实现。
- 分布式 session 共享不在当前范围内。
- 没有自动的 session 过期/TTL 机制。

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L1**（`SessionMeta`、`SessionStatus`、`AgentState` 数据协议）；**L2**（session CRUD、上下文裁剪、权限模型）
- **权威来源**: `specs/010-session-management/spec.md`
- **已知偏差**:
  - Rust 侧用 `SessionImpl` 而非 Python 的继承体系
  - 上下文裁剪当前使用字符数估算而非精确 tokenizer
  - `SessionStore` 是 Rust 侧新增的抽象层

## 7. 相关模块 (See Also)

- [Agent 系统](./agent.md) — Session 在 Agent 中的使用
- [记忆 / memory](./memory.md) — 长期记忆（session 管短期上下文）
- [消息与基础类型](./message-types.md) — `Msg` 和 `ContentBlock`
