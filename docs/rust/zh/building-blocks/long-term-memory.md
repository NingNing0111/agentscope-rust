---
title: "长期记忆"
description: "在会话间持久化智能体的记忆，支持语义检索"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用。长期记忆提供文件型与向量型双后端。
</Note>

长期记忆（Long-term Memory）让智能体在会话之间记住事实、偏好与项目信息。AgentScope Rust 通过 `Memory` trait（`agent_scope_memory` crate）抽象，内置两种后端：

| 后端 | 存储 | 检索 |
|------|------|------|
| `FileMemory` | Markdown + YAML frontmatter，`MEMORY.md` 索引 | 基于 frontmatter 的索引注入与关键字搜索 |
| `TurbovecMemory` | 文件记忆 + 向量索引（`turbovec`） | 语义检索（`MemoryVectorIndex`） |

## Memory trait

所有记忆后端实现同一个 `Memory` trait：

| 方法 | 说明 |
|------|------|
| `write(entry)` | 写入一条记忆（`MemoryEntry`） |
| `read(name)` | 按名称读取，返回 `Option<MemoryEntry>` |
| `delete(name)` | 删除一条记忆 |
| `list()` | 列出所有条目头，返回 `Vec<MemoryFileHeader>` |
| `search(query, type_filter)` | 关键字搜索，可按类型过滤 |
| `get_index_content()` | 读取 `MEMORY.md` 索引内容 |
| `retrieve_relevant(query, model, max_results)` | 用模型挑选与查询最相关的记忆 |

> 记忆名必须匹配 `[A-Za-z0-9_-]+`，任何包含 `/`、`.`、`..` 的名称都会被拒绝，防止路径越界。

## 核心数据结构

### MemoryEntry

| 字段 | 说明 |
|------|------|
| `name` | 记忆唯一名（用于文件命名与索引） |
| `description` | 一句话描述，出现在索引里 |
| `metadata` | `MemoryMetadata`（类型、时间戳、标签） |
| `content` | 记忆正文 |

构造函数：`MemoryEntry::new(name, description, mem_type, content)`。

### MemoryType

| 变体 | 说明 |
|------|------|
| `User` | 用户相关 |
| `Feedback` | 反馈 |
| `Project` | 项目信息 |
| `Reference` | 参考资料 |
| `Unknown(String)` | 其它类型（保留原始字符串） |

### MemoryFileHeader（`list()` 返回的条目头）

| 字段 | 说明 |
|------|------|
| `filename` | 文件名（如 `user_name.md`） |
| `path` | 完整路径 |
| `description` | 描述（可为空） |
| `mem_type` | 类型（可为空） |
| `mtime` | 修改时间 |

## MemoryConfig

`MemoryConfig` 控制文件记忆的行为：

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `memory_dir` | `"Memory"` | 记忆目录名（相对 workdir 或绝对路径） |
| `max_index_tokens` | `4000` | `MEMORY.md` 索引最大 token 数 |
| `retrieval_async` | `true` | 检索是否异步 |
| `retrieval_max_files` | `200` | `list()` 最多列出的条目数 |
| `retrieval_max_tokens_per_file` | `2000` | 每条记忆检索时的 token 上限 |
| `retrieval_max_tokens_per_frontmatter` | `256` | frontmatter 的 token 上限 |
| `memory_instructions` | 内置默认 | 注入系统提示的记忆使用说明 |
| `retrieval_instructions` | 内置默认 | 让模型挑选相关记忆的提示 |

## FileMemory

文件型记忆：每条记忆是一个带 frontmatter 的 Markdown 文件，`MEMORY.md` 维护索引：

```rust
use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryType};

let config = MemoryConfig { memory_dir: "Memory".into(), ..MemoryConfig::default() };
let memory = FileMemory::new("/path/to/workdir", config, None);

memory.write(MemoryEntry::new("user_name", "用户的名字。", MemoryType::User, "Alice")).await?;
let headers = memory.list().await?;
let entry = memory.read("user_name").await?;   // Option<MemoryEntry>
```

构造函数签名：`FileMemory::new(workdir: &str, config: MemoryConfig, backend: Option<Arc<dyn Backend>>)`；`backend` 传 `None` 时使用本地文件系统后端。

## TurbovecMemory

向量型记忆：在文件记忆之上建立向量索引，做语义检索（`turbovec` 向量库），适合需要按语义召回的记忆场景。

```rust
use agent_scope_memory::{TurbovecMemory, TurbovecMemoryConfig, Memory, MemoryType};
use agent_scope_rag::TurbovecIndexAdapter;
use std::sync::Arc;

let config = TurbovecMemoryConfig::default();
let memory = TurbovecMemory::new(
    "/path/to/workdir",
    config,
    embedding,                     // Arc<dyn EmbeddingModel>
    Arc::new(TurbovecIndexAdapter::new(4)?),  // Arc<dyn MemoryVectorIndex>
    None,                          // backend 缺省用本地后端
).await?;
```

`TurbovecMemoryConfig` 在 `MemoryConfig` 之上增加向量索引设置：

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `bit_width` | `4` | turbovec 压缩位宽（2 / 3 / 4） |
| `collection_name` | `"memories"` | 向量集合名 |
| `retrieval_top_k` | `10` | 每次检索最多返回条数 |
| `retrieval_score_threshold` | `None` | 最小相似度阈值 |
| `auto_rebuild` | `false` | 索引缺失 / 维度不匹配时是否自动重建 |
| `vector_index_dir` | `".turbovec"` | 向量索引子目录 |

`TurbovecMemory` 还提供维护方法：

| 方法 | 说明 |
|------|------|
| `semantic_search(query, type_filter, top_k)` | 语义检索，返回按相似度排序的 `MemorySearchResult` |
| `save_index()` | 把向量索引持久化到磁盘 |
| `rebuild_index()` | 从 Markdown 文件重建向量索引，返回 `MemoryRebuildReport` |
| `vector_index_status()` | 返回索引健康状态（`Clean` / `Missing` / `Corrupted` / `DimensionMismatch`） |

> 向量索引需要 64 位目标（x86_64 / aarch64）；WASM 与 32 位目标不支持（`turbovec` 的指针宽度要求）。

## 与 Agent 联动

通过 `MemoryMiddleware` 把记忆注入 agent 上下文（`MEMORY.md` 索引进入系统提示，并在每轮回复前用模型挑选相关记忆）：

```rust
use std::sync::Arc;
use agent_scope_agent::MemoryMiddleware;
use agent_scope_memory::{FileMemory, Memory, MemoryConfig};

let memory = Arc::new(FileMemory::new("/path/to/workdir", MemoryConfig::default(), None)) as Arc<dyn Memory>;
let middleware = Arc::new(MemoryMiddleware::new(memory.clone(), MemoryConfig::default()));

let agent = ReActAgent::new(config, ReActConfig::default(), ContextConfig::default(), vec![middleware])?;
```

也可用 `MemoryMiddleware::with_config(workdir, memory_dir, config)` 一步创建基于 `FileMemory` 的中间件。

## 完整示例

见 [`examples/memory`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/memory/)（`cargo run -p memory`），演示 `FileMemory` 的写入 / 列出 / 读取 / 删除，无需模型凭据。
