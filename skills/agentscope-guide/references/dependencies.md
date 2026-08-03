# 参考:依赖配置(Cargo.toml)

> 本文档详细说明如何在你的项目中引入 AgentScope Rust 的 crate,覆盖 git 依赖、crates.io 版本、workspace 内 path 依赖三种方式,以及依赖项选型建议。

## 1. 三种引入方式对比

| 方式 | 适用场景 | 写法 |
|------|---------|------|
| **git 依赖**(当前推荐,未发布 crates.io) | 外部用户在 crates.io 发布前使用 | `{ git = "...", branch = "master" }` |
| **crates.io 版本**(发布后) | 正式依赖 | `{ version = "0.1" }` |
| **path 依赖** | 仓库内部 / 本地开发 | `{ path = "crates/agent_scope_agent" }` |

> **注意**:`path = "../agentscope-rust/..."` 这类相对路径只适合仓库**内部或本地**引用,不适合作为对外发布/示例的依赖方式。对外使用请用 git 或版本号。

## 2. 通过 GitHub(git 依赖)

项目源码托管在 `https://github.com/ningning0111/agentscope-rust`。最小依赖集:

```toml
[dependencies]
agent_scope_agent = { git = "https://github.com/ningning0111/agentscope-rust", branch = "master" }
agent_scope_dashscope = { git = "https://github.com/ningning0111/agentscope-rust", branch = "master" }
agent_scope_tool = { git = "https://github.com/ningning0111/agentscope-rust", branch = "master" }
agent_scope_message = { git = "https://github.com/ningning0111/agentscope-rust", branch = "master" }
agent_scope_event = { git = "https://github.com/ningning0111/agentscope-rust", branch = "master" }

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
agent_scope_agent = { git = "https://github.com/ningning0111/agentscope-rust", rev = "87e5d37" }
```

## 3. crates.io 发布后(版本号)

发布后(当前版本 `0.1.0`)直接写版本号:

```toml
[dependencies]
agent_scope_agent = "0.1"
agent_scope_dashscope = "0.1"
agent_scope_tool = "0.1"
agent_scope_message = "0.1"
agent_scope_event = "0.1"
```

## 4. 按能力选依赖

| 需要的能力 | 必须依赖 | 可选依赖 |
|-----------|---------|---------|
| 最小对话 Agent | `agent_scope_agent`, `agent_scope_dashscope`, `agent_scope_message` | `agent_scope_tool`(加工具) |
| 工具调用 | + `agent_scope_tool` | `schemars`(自动生成 schema) |
| 流式事件渲染 | `agent_scope_event` | `futures`(StreamExt) |
| 长期记忆 | `agent_scope_memory` | — |
| RAG | `agent_scope_rag`, `agent_scope_embedding` | — |
| 工作空间 | `agent_scope_workspace` | `agent_scope_sandbox`(执行隔离) |
| 会话管理 | `agent_scope_state` | — |
| 多步骤规划 / SubAgent | `agent_scope_agent` | — |

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

## 6. 常见坑

1. **不要把 `path = "../..."` 写进对外依赖**——外部用户 clone 后无法编译。
2. **git 依赖不固定 rev/tag**,`cargo update` 后可能拉到破坏性变更。
3. **根 package `agentscope` 不是 facade**——依赖它不会有完整的 `agent_scope_*` API。
4. workspace 使用 **edition 2024**,需要较新的 Rust stable(建议 1.85+,`cargo --version` 确认)。
