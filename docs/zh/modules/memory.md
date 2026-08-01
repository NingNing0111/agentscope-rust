# 记忆系统 / Memory

> 一句话定位：`agent_scope_memory` 提供跨会话持久化的长期记忆——用 `Memory` trait 抽象读写、搜索、索引和相关记忆检索，用 `FileMemory` 将记忆保存为 Markdown 文件，并通过 `MemoryMiddleware` 注入 Agent 回复流程。

## 1. 模块概述 (Overview)

本模块覆盖两个协作部分：

| 部分 | 职责 |
|------|------|
| `agent_scope_memory` | 长期记忆的数据模型、文件存储、索引、搜索、相关记忆检索 |
| `agent_scope_agent::MemoryMiddleware` | 将长期记忆接入 Agent 生命周期：系统提示词注入、异步检索、`HintBlock` 注入 |

**适用场景**：保存用户偏好、项目事实、反馈规则、外部参考资料；让 Agent 在后续对话中使用已保存事实；在不把全部记忆塞入上下文的前提下，通过 `MEMORY.md` 索引和检索选择相关文件。

**前置阅读**：建议先阅读 [Agent 系统](./agent.md)、[消息与基础类型](./message-types.md) 和 [模型抽象](./model.md)。如果只想跑通集成示例，可直接运行 `examples/memory_test.rs`。

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 `Memory` trait

`Memory` 是长期记忆后端的统一接口：

| 方法 | 说明 |
|------|------|
| `write(entry)` | 写入或更新一条记忆；同名记忆采用 upsert 语义 |
| `read(name)` | 按名称读取完整记忆；不存在时返回 `Ok(None)` |
| `delete(name)` | 删除记忆文件并移除索引行 |
| `list()` | 只列出记忆头信息，不加载完整正文 |
| `search(query, type_filter)` | 在描述和正文中做大小写不敏感的子串搜索，可按记忆类型过滤 |
| `get_index_content()` | 读取 `MEMORY.md` 索引内容 |
| `retrieve_relevant(query, model, max_results)` | 使用绑定的 `ChatModel` 选择相关记忆文件，并返回拼接后的正文片段 |

`Memory` 要求 `Send + Sync`，因此可作为 `Arc<dyn Memory>` 注入 middleware。

### 2.2 `MemoryEntry` 与 `MemoryMetadata`

一条记忆由四个核心字段组成：

| 字段 | 说明 |
|------|------|
| `name` | 唯一 slug；`FileMemory` 要求匹配 `[A-Za-z0-9_-]+` |
| `description` | 一行描述，用于 `MEMORY.md` 索引和相关性选择 |
| `metadata` | 类型、创建时间、更新时间、可选标签 |
| `content` | 记忆正文，Markdown 文本 |

`MemoryType` 当前内置四类：

| 类型 | 用途 |
|------|------|
| `User` | 用户身份、偏好、长期背景 |
| `Feedback` | 用户给出的工作方式反馈 |
| `Project` | 当前项目事实、约束、状态 |
| `Reference` | 外部资料、链接、文档摘要 |

未知类型字符串会进入 `MemoryType::Unknown(String)`，便于未来扩展。

### 2.3 Markdown 文件格式

`FileMemory` 将每条记忆保存为 `<name>.md`，前置 frontmatter，正文为 Markdown：

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

`MEMORY.md` 是同目录下的索引文件，每条记忆一行：

```markdown
- [user-favorite-color](user-favorite-color.md) — The user's favorite color preference
```

### 2.4 `FileMemory`

`FileMemory::new(workdir, config, backend)` 是当前内置实现：

- 如果 `config.memory_dir` 是相对路径，则解析为 `workdir / memory_dir`
- 如果是绝对路径，则直接使用该目录
- `backend = None` 时使用 `LocalBackend`
- `write()` 会创建父目录、写入 `<name>.md`，并更新 `MEMORY.md`
- `delete()` 对不存在文件是幂等的，并会移除索引行
- `list()` 会跳过 `MEMORY.md`，只解析带 frontmatter 的 `.md` 文件，并按修改时间倒序截断到 `retrieval_max_files`

### 2.5 `MemoryConfig`

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `memory_dir` | `"Memory"` | 记忆目录 |
| `max_index_tokens` | `4000` | 注入系统提示词前，索引允许的最大 token 预算 |
| `retrieval_async` | `true` | 是否在 `pre_reply` 异步启动相关记忆检索 |
| `retrieval_max_files` | `200` | 最多列出/检索的文件数 |
| `retrieval_max_tokens_per_file` | `2000` | 每个被检索文件正文的最大 token 预算 |
| `retrieval_max_tokens_per_frontmatter` | `256` | frontmatter token 预算（保留配置项） |
| `memory_instructions` | 默认长期记忆提示词 | 注入系统提示词，告诉模型如何使用索引 |
| `retrieval_instructions` | 默认检索提示词 | 相关文件选择时使用 |

`validate()` 会确保目录非空，且所有 token / 文件数上限大于 0。

### 2.6 `MemoryMiddleware`

`MemoryMiddleware` 将 `Memory` 接入 Agent 生命周期：

| Hook | 行为 |
|------|------|
| `pre_reply` | 保存当前 `ChatModel` 引用；若 `retrieval_async = true` 且用户输入非空，启动相关记忆检索任务 |
| `on_system_prompt` | 读取 `MEMORY.md`；若有模型引用则按 `max_index_tokens` 截断；追加记忆使用说明和索引 |
| `pre_reasoning` | 如果异步检索任务已完成，将检索结果作为 `HintBlock` 注入最后一条用户消息 |

如果索引为空或读取失败，middleware 会注入 `Your MEMORY.md is currently empty.`，不会让 Agent 回复流程失败。

## 3. 快速示例 (Quick Example)

仓库示例中，带记忆 Agent 的标准构造如下：

<!-- source: examples/common.rs:L368-L406 -->
```rust
pub fn create_memory_agent(
    api_key: &str,
    model_name: &str,
    workdir: &str,
) -> Result<ReActAgent, Box<dyn std::error::Error>> {
    let model = create_model(api_key, model_name);

    // Build FileMemory with default config
    let memory_config = MemoryConfig {
        memory_dir: "memory_data".into(),
        ..Default::default()
    };
    let memory: Arc<dyn agent_scope_memory::Memory> =
        Arc::new(FileMemory::new(workdir, memory_config.clone(), None));

    // Wrap in MemoryMiddleware
    let middleware = Arc::new(MemoryMiddleware::new(memory, memory_config));
```

完整示例继续构造 `AgentConfig` 和 `ReActAgent`，并把 `middleware` 作为 `vec![middleware]` 注入。

## 4. 关键用法模式 (Usage Patterns)

### 4.1 运行内置记忆集成示例

`examples/memory_test.rs` 会执行三组端到端检查：写入记忆、搜索记忆、基于记忆回答问题。

```bash
cargo run --example memory_test -- --api-key sk-xxxxx
cargo run --example memory_test -- --api-key sk-xxxxx --model qwen-max
cargo run --example memory_test -- --api-key sk-xxxxx --keep-dir
```

该示例也支持从环境变量读取 API key：

```bash
API_KEY=sk-xxxxx cargo run --example memory_test
```

`--keep-dir` 会保留临时记忆目录，便于查看实际生成的 `<name>.md` 与 `MEMORY.md`。

### 4.2 直接写入和读取记忆

适合在应用代码中显式保存用户偏好、项目事实或外部参考：

```rust
use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryType};

let config = MemoryConfig {
    memory_dir: "memory_data".into(),
    ..Default::default()
};
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

写入同名 `name` 会覆盖原文件并更新索引行，不会生成重复索引。

### 4.3 搜索和类型过滤

`search()` 是本地子串搜索：匹配 `description + content`，大小写不敏感：

```rust
let all = memory.search("Hangzhou", None).await?;
let only_user = memory
    .search("favorite", Some(MemoryType::User))
    .await?;
```

如果你需要语义相关性选择，而不是简单子串匹配，请使用 `retrieve_relevant()` 或让 `MemoryMiddleware` 自动检索。

### 4.4 将记忆接入 Agent

`MemoryMiddleware` 是推荐的 Agent 集成方式：

```rust
use std::sync::Arc;
use agent_scope_agent::MemoryMiddleware;
use agent_scope_memory::{FileMemory, MemoryConfig};

let memory_config = MemoryConfig {
    memory_dir: "memory_data".into(),
    ..Default::default()
};
let memory = Arc::new(FileMemory::new(workdir, memory_config.clone(), None));
let middleware = Arc::new(MemoryMiddleware::new(memory, memory_config));

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![middleware],
)?;
```

接入后：

1. 每次回复前，Agent 系统提示词会带上长期记忆说明和 `MEMORY.md` 索引。
2. 用户输入非空且 `retrieval_async = true` 时，会启动一个相关记忆检索任务。
3. 如果检索在 reasoning 前完成，结果会以 `HintBlock` 追加到最后一条用户消息。

### 4.5 控制索引和正文截断

`MEMORY.md` 可能随着记忆增多而变长。middleware 在有模型引用时会用 `model.count_tokens(...)` 估算 token，并在超过 `max_index_tokens` 时追加截断提示：

```text
<<<TRUNCATED: 12 memory index lines omitted>>>
```

被检索文件正文也会按 `retrieval_max_tokens_per_file` 截断，并追加：

```text
<<<TRUNCATED>>>
```

### 4.6 自定义存储后端

`Backend` 是底层存储抽象，当前内置 `LocalBackend`。如果需要远程存储，可实现：

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

然后通过 `FileMemory::new(workdir, config, Some(Arc::new(my_backend)))` 注入。

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误 | 常见原因 | 处理建议 |
|------|----------|----------|
| `MemoryError::IoError` | 文件读写、目录遍历或元数据读取失败 | 检查路径、权限和磁盘状态 |
| `MemoryError::ParseError` | 预留的解析错误类型 | 当前多数 malformed 文件会被跳过而不是抛出 |
| `MemoryError::ValidationError` | `name`/`description`/配置项非法，或空 query | 修正输入；`name` 使用 `[A-Za-z0-9_-]+` |
| `MemoryError::NotFound` | 预留的未找到错误类型 | 当前 `read()` 对不存在返回 `Ok(None)` |
| `MemoryError::IndexError` | 索引管理失败 | 检查 `MEMORY.md` 是否可写 |
| `MemoryError::RetrievalError` | 构造检索提示或解析检索结果失败 | 通常可降级为无相关记忆 |

**不支持的能力**：

- 当前仅内置本地文件后端 `LocalBackend`；远程后端需要用户实现 `Backend`。
- `search()` 不是向量检索或语义搜索，只做本地子串匹配。
- `retrieve_relevant()` 依赖 `ChatModel::generate_structured_output()`；如果模型或 provider 不支持结构化输出，会降级为无相关记忆，而不是伪造结果。
- 记忆写入不会自动从对话中抽取事实；应用或工具需要显式调用 `write()`。

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L1**（Markdown frontmatter、`MEMORY.md` 索引和记忆类型数据协议）；**L2**（读写、搜索、索引更新、middleware 注入和相关记忆检索行为）
- **权威来源**: `specs/001-compatibility-baseline/capability-matrix.json`
- **已知偏差**:
  - 矩阵 `status` 字段当前全部为 `NOT_ANALYZED`；本页等级以 `memory` 相关条目的 `target_level` + `specs/009-memory-system` + 当前代码状态交叉核实。
  - `search()` 当前是确定性的本地子串搜索；语义相关性选择只在 `retrieve_relevant()` / middleware 路径中通过模型结构化输出完成。
  - `MemoryMiddleware` 的检索任务只在任务已完成时注入 `HintBlock`；未完成时本轮 reasoning 不阻塞等待，避免拖慢回复路径。
  - malformed frontmatter 文件在 `list()`/`search()` 中通常会被跳过，保持记忆系统鲁棒性。
- **不支持的能力**: 内置远程存储后端、自动对话事实抽取、向量语义搜索均未作为本模块内置能力提供。

## 7. 相关模块 (See Also)

- [Agent 系统 / agent](./agent.md) — `MemoryMiddleware` 的执行位置和 Agent 生命周期
- [消息与基础类型 / message-types](./message-types.md) — `HintBlock` 的数据协议
- [模型抽象 / model](./model.md) — `retrieve_relevant()` 使用的 `ChatModel` 和结构化输出
- 会话管理 — 短期上下文与长期记忆的边界
- RAG — 与文档知识库、向量检索的区别和组合方式
