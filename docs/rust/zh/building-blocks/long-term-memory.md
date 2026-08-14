---
title: "长期记忆"
description: "在会话间持久化智能体的记忆，支持语义检索"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L2）。长期记忆在 AgentScope Rust 中可用，提供文件型与向量型双后端。兼容基线为 AgentScope Python v2.0.5。
</Note>

长期记忆（Long-term Memory）让智能体在会话之间记住事实、偏好与项目信息。AgentScope Rust 通过 `Memory` trait（`agent_scope_memory`）抽象，内置两种后端：

| 后端 | 存储 | 检索 |
|------|------|------|
| `FileMemory` | Markdown + YAML frontmatter，`MEMORY.md` 索引 | 基于 frontmatter 的索引注入与相关文件检索 |
| `TurbovecMemory` | 文件记忆 + 向量索引（`turbovec`） | 语义检索（`MemoryVectorIndex`） |

## Memory trait

| 方法 | 说明 |
|------|------|
| `write(entry)` | 写入一条记忆（`MemoryEntry`：name / description / type / content） |
| `read(name)` | 按名称读取 |
| `delete(name)` | 删除 |
| `list()` | 列出所有条目头 |
| `search(query)` | 搜索 |
| `retrieve_relevant(query)` | 检索与查询相关的记忆 |

## FileMemory

文件型记忆：每条记忆是一个带 frontmatter 的 Markdown 文件，`MEMORY.md` 维护索引：

```rust
use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryType};

let config = MemoryConfig { memory_dir: "Memory".into(), ..MemoryConfig::default() };
let memory = FileMemory::new("/path/to/workdir", config, None);

memory.write(MemoryEntry::new("user_name", "用户的名字。", MemoryType::User, "Alice")).await?;
let headers = memory.list().await?;
```

## 与 Agent 联动

通过 `MemoryMiddleware` 将记忆注入 agent 上下文：

```rust
use std::sync::Arc;
use agent_scope_agent::MemoryMiddleware;
use agent_scope_memory::{FileMemory, Memory, MemoryConfig};

let memory = Arc::new(FileMemory::new("/path/to/workdir", MemoryConfig::default(), None)) as Arc<dyn Memory>;
let middleware = Arc::new(MemoryMiddleware::new(memory.clone(), MemoryConfig::default()));

let agent = ReActAgent::new(config, ReActConfig::default(), ContextConfig::default(), vec![middleware])?;
```

## TurbovecMemory

向量型记忆：在文件记忆之上建立向量索引，做语义检索（`turbovec` 向量库），适合需要按语义召回的记忆场景。

## 完整示例

见 [`examples/memory`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/memory/)（`cargo run -p memory`），演示 `FileMemory` 的写入 / 列出 / 读取 / 删除，无需模型凭据。
