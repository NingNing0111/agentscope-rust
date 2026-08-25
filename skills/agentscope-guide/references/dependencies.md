# 参考:依赖配置(Cargo.toml)

> 本文档详细说明如何在你的项目中引入 AgentScope Rust 的 crate,覆盖 GitHub git 依赖、crates.io 版本以及依赖项选型建议。

## 1. 引入方式对比

| 方式 | 适用场景 | 写法 |
|------|---------|------|
| **GitHub git 依赖**(当前推荐,未发布 crates.io) | 外部用户在 crates.io 发布前使用 | `{ git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }` |
| **crates.io 版本**(发布后) | 正式依赖 | `{ version = "0.1" }` |

> **注意**:对外文档和可复制示例不要使用 `path = "../..."` 或 `path = "crates/..."`。统一使用 `https://github.com/NingNing0111/agentscope-rust` 的 git 依赖;仓库内部开发才使用 workspace 自己的 path 关系。

## 2. 通过 GitHub(git 依赖)

项目源码托管在 `https://github.com/NingNing0111/agentscope-rust`。最小依赖集:

```toml
[dependencies]
agent_scope_agent = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_rig = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_tool = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_message = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_event = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }
agent_scope_utils = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master" }

# 非 AgentScope 依赖
tokio = { version = "1", features = ["full"] }
futures = "0.3"
async-trait = "0.1"
schemars = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**可复现构建建议**:git 依赖用 `rev` 固定提交,或用 `tag`:

```toml
agent_scope_agent = { git = "https://github.com/NingNing0111/agentscope-rust", rev = "87e5d37" }
```

## 3. crates.io 发布后(版本号)

发布后(当前版本 `0.1.0`)直接写版本号:

```toml
[dependencies]
agent_scope_agent = "0.1"
agent_scope_rig = "0.1"
agent_scope_tool = "0.1"
agent_scope_message = "0.1"
agent_scope_event = "0.1"
agent_scope_utils = "0.1"
```

## 4. 按能力选依赖

| 需要的能力 | 必须依赖 | 可选依赖 |
|-----------|---------|---------|
| 最小对话 Agent | `agent_scope_agent`, `agent_scope_rig`, `agent_scope_message` | `agent_scope_tool`(加工具) |
| 工具调用 | + `agent_scope_tool` | `schemars`(自动生成 schema) |
| 流式事件渲染 | `agent_scope_event` | `futures`(StreamExt) |
| 长期记忆 | `agent_scope_memory` | — |
| RAG | `agent_scope_rag`, `agent_scope_embedding` | — |
| 工作空间 | `agent_scope_workspace` | `agent_scope_sandbox`(执行隔离) |
| 会话管理 | `agent_scope_state` | `sqlx`(当你直接传入/管理 `SqlitePool` 时) |
| 多步骤规划 / SubAgent | `agent_scope_agent` | — |
| 通用 id/path/env/error helper | `agent_scope_utils` | — |

### 会话存储选择

`agent_scope_state` 内置三类 `SessionStore`:

| 存储 | 适用场景 | 依赖提示 |
|------|----------|----------|
| `InMemorySessionStore` | 测试、短生命周期进程 | 只依赖 `agent_scope_state` |
| `JsonFileSessionStore` | 单机文件持久化、调试可读 | 只依赖 `agent_scope_state` |
| `SqliteSessionStore` | 单机/嵌入式持久化、需要查询 session meta | 通常只依赖 `agent_scope_state`;如果应用要自己创建 `sqlx::SqlitePool`,再直接依赖 `sqlx` |

生成 session id、tool result id、document id 等 UUID 时优先用 `agent_scope_utils::id::generate_uuid()`,不要在业务 crate 里重复写 `uuid::Uuid::new_v4().as_simple().to_string()`。

### Sandbox feature

`agent_scope_sandbox` 默认提供本地进程隔离实现。需要 microsandbox backend 时显式打开 feature:

```toml
agent_scope_sandbox = { git = "https://github.com/NingNing0111/agentscope-rust", branch = "master", features = ["microsandbox"] }
```

microsandbox 依赖宿主运行时能力;Linux CI/容器环境可能还需要安装对应系统库和 runtime。对外示例建议把 microsandbox 标成可选能力,不要作为最小依赖的一部分。

## 5. 其他常用依赖

AgentScope crate 内部使用并 re-export 或要求你的代码也用到的:

| 依赖 | 用途 | 何时需要直接依赖 |
|------|------|-----------------|
| `schemars` + `JsonSchema` derive | `FunctionTool` 自动生成 JSON Schema | 定义工具参数类型时 |
| `serde` / `serde_json` | 工具输入类型反序列化、手写 schema | 定义 `#[derive(Deserialize, JsonSchema)]` 结构体时 |
| `tokio`(full) | 异步运行时 | 几乎所有程序 |
| `futures` | `StreamExt` 消费事件流 | 使用 `reply_stream()` 时 |
| `async-trait` | 自定义 `Middleware` / `Backend` / `ChatModel` | 实现 trait 时 |
| `dotenv` | 加载 `.env` 凭据 | 应用入口 |
| `sqlx` | SQLite pool/迁移/事务集成 | 只有应用层直接操作 `SqlitePool` 时 |

## 6. 常见坑

1. **不要把 `path = "../..."` 或 `path = "crates/..."` 写进对外依赖**——外部用户复制后无法编译;用 `git = "https://github.com/NingNing0111/agentscope-rust"`。
2. **git 依赖不固定 rev/tag**,`cargo update` 后可能拉到破坏性变更。
3. **根 package `agentscope` 不是 facade**——依赖它不会有完整的 `agent_scope_*` API。
4. workspace 使用 **edition 2024**,需要较新的 Rust stable(建议 1.85+,`cargo --version` 确认)。
