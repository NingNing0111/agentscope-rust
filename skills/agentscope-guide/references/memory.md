# 参考:记忆系统(`agent_scope_memory` / `agent_scope_agent::MemoryMiddleware`)

> 详细 API 参考:`Memory` trait、`MemoryEntry`、`FileMemory`、`MemoryConfig`、`MemoryMiddleware`、`Backend`、`TurbovecMemory`。

## 1. `Memory` trait

长期记忆后端的统一接口(`Send + Sync`,可作 `Arc<dyn Memory>` 注入 middleware):

| 方法 | 说明 |
|------|------|
| `write(entry)` | 写入或更新一条记忆;同名 upsert |
| `read(name)` | 按名称读取;不存在返回 `Ok(None)` |
| `delete(name)` | 删除记忆文件并移除索引行 |
| `list()` | 只列记忆头信息,不加载正文 |
| `search(query, type_filter)` | 描述+正文的大小写不敏感子串搜索,可按类型过滤 |
| `get_index_content()` | 读取 `MEMORY.md` 索引 |
| `retrieve_relevant(query, model, max_results)` | 用绑定 `ChatModel` 选择相关记忆文件,返回拼接正文片段 |

## 2. `MemoryEntry` 与 `MemoryMetadata`

| 字段 | 说明 |
|------|------|
| `name` | 唯一 slug;`FileMemory` 要求 `[A-Za-z0-9_-]+` |
| `description` | 一行描述,用于索引与相关性选择 |
| `metadata` | 类型、创建/更新时间、可选标签 |
| `content` | 记忆正文(Markdown) |

`MemoryType` 内置四类:`User`、`Feedback`、`Project`、`Reference`。未知类型进 `MemoryType::Unknown(String)`。

构造:`MemoryEntry::new(name, description, type, content)`。

## 3. 磁盘格式(`FileMemory`)

每条记忆保存为 `<name>.md`,前置 frontmatter:

```markdown
---
name: user-favorite-color
description: The user's favorite color preference
type: user
created_at: 2026-08-01T00:00:00Z
updated_at: 2026-08-01T00:00:00Z
---

The user's favorite color is cerulean blue.
```

`MEMORY.md` 是同目录索引,每条一行:

```markdown
- [user-favorite-color](user-favorite-color.md) — The user's favorite color preference
```

## 4. `FileMemory`

`FileMemory::new(workdir, config, backend)`:

- `config.memory_dir` 相对路径 → 解析为 `workdir / memory_dir`;绝对路径直接用。
- `backend = None` 时用 `LocalBackend`。
- `write()` 创建父目录、写 `<name>.md`、更新 `MEMORY.md`。
- `delete()` 对不存在文件幂等,并移除索引行。
- `list()` 跳过 `MEMORY.md`,只解析带 frontmatter 的 `.md`,按修改时间倒序截断到 `retrieval_max_files`。

```rust
use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryType};

let config = MemoryConfig { memory_dir: "memory_data".into(), ..Default::default() };
let memory = FileMemory::new(".", config, None);

let entry = MemoryEntry::new(
    "user-favorite-color",
    "The user's favorite color preference",
    MemoryType::User,
    "The user's favorite color is cerulean blue.",
);
memory.write(entry).await?;
let loaded = memory.read("user-favorite-color").await?;
```

## 5. `MemoryConfig`

| 字段 | 默认 | 说明 |
|------|------|------|
| `memory_dir` | `"Memory"` | 记忆目录 |
| `max_index_tokens` | `4000` | 注入系统提示词前,索引最大 token 预算 |
| `retrieval_async` | `true` | 是否在 `pre_reply` 异步启动检索 |
| `retrieval_max_files` | `200` | 最多列出/检索文件数 |
| `retrieval_max_tokens_per_file` | `2000` | 每个被检索文件正文最大 token 预算 |
| `retrieval_max_tokens_per_frontmatter` | `256` | frontmatter token 预算 |
| `memory_instructions` | 默认提示词 | 告诉模型如何使用索引 |
| `retrieval_instructions` | 默认提示词 | 相关文件选择提示 |

`validate()` 确保目录非空、所有上限 > 0。

## 6. `MemoryMiddleware`

将 `Memory` 接入 Agent 生命周期:

| Hook | 行为 |
|------|------|
| `pre_reply` | 保存 `ChatModel` 引用;若 `retrieval_async = true` 且输入非空,启动检索任务 |
| `on_system_prompt` | 读 `MEMORY.md`;按 `max_index_tokens` 截断;追加记忆说明与索引 |
| `pre_reasoning` | 检索任务完成时,结果作为 `HintBlock` 注入最后一条用户消息 |

索引为空/读取失败时注入 `Your MEMORY.md is currently empty.`,不使回复失败。

```rust
use std::sync::Arc;
use agent_scope_agent::MemoryMiddleware;
use agent_scope_memory::{FileMemory, MemoryConfig};

let memory_config = MemoryConfig { memory_dir: "memory_data".into(), ..Default::default() };
let memory = Arc::new(FileMemory::new(workdir, memory_config.clone(), None));
let middleware = Arc::new(MemoryMiddleware::new(memory, memory_config));

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![middleware],
)?;
```

便捷构造:

```rust
let middleware = Arc::new(MemoryMiddleware::with_config(
    &workdir,           // 工作目录
    &memory_dir,        // 记忆目录
    MemoryConfig::default(),
));
```

## 7. 截断行为

- `MEMORY.md` 超 `max_index_tokens` 时追加 `<<<TRUNCATED: N memory index lines omitted>>>`。
- 被检索文件正文超 `retrieval_max_tokens_per_file` 时追加 `<<<TRUNCATED>>>`。

## 8. 自定义存储后端

`Backend` trait 是底层存储抽象:

```rust
#[async_trait::async_trait]
pub trait Backend: Send + Sync {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, MemoryError>;
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), MemoryError>;
    async fn delete_file(&self, path: &str) -> Result<(), MemoryError>;
    async fn file_exists(&self, path: &str) -> Result<bool, MemoryError>;
    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, MemoryError>;
    fn join_path(&self, a: &str, b: &str) -> String;
    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, MemoryError>;
    fn normpath(&self, path: &str) -> String;
    fn isabs(&self, path: &str) -> bool;
}
```

注入:`FileMemory::new(workdir, config, Some(Arc::new(my_backend)))`。

## 9. `TurbovecMemory`(向量化长期记忆)

Feature 022 提供基于 TurboVec 的向量化长期记忆:

- `TurbovecMemory`、`TurbovecMemoryConfig`:`MemoryVectorIndex`、`MemoryVectorRecord`、`MemorySearchResult`、`MemoryRebuildReport`、`VectorIndexStatus`。
- 将记忆内容向量化,支持语义检索(优于 `search()` 的子串匹配)。
- 与 `Memory` trait 同一接口,可作为 `Arc<dyn Memory>` 注入 `MemoryMiddleware`。

使用方式:用 `TurbovecMemory` 构造后同样通过 `MemoryMiddleware::new(memory, config)` 接入,替换 `FileMemory` 即可升级为向量检索。

## 10. 错误

| 错误 | 常见原因 |
|------|----------|
| `MemoryError::IoError` | 文件读写/遍历/元数据失败 |
| `MemoryError::ValidationError` | `name`/`description`/配置非法,或空 query |
| `MemoryError::NotFound` | 预留;当前 `read()` 对不存在返回 `Ok(None)` |
| `MemoryError::IndexError` | 索引管理失败 |
| `MemoryError::RetrievalError` | 检索提示构造或结果解析失败 |

## 11. 不支持的能力

- 仅内置本地文件后端 `LocalBackend`;远程后端需实现 `Backend`。
- `search()` 是本地子串搜索,不是向量/语义搜索(语义检索用 `retrieve_relevant()` 或 `TurbovecMemory`)。
- 记忆写入不会自动从对话中抽取事实;应用/工具需显式调用 `write()`。
