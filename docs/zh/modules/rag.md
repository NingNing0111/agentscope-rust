# RAG 检索增强生成 / RAG

> 一句话定位：`agent_scope_rag` 提供文档解析、分块、向量存储和知识库检索的完整 RAG 管线——通过 `Parser` 解析文档、`Chunker` 分割文本、`VectorStore` 存储向量、`KnowledgeBase` 管理知识库、`RAGMiddleware` 注入 Agent 回复流程。

## 1. 模块概述 (Overview)

| 组件 | 职责 |
|------|------|
| `Parser` / `TextParser` | 解析文档：纯文本、Markdown，输出结构化 `Section` 列表 |
| `Chunker` / `ApproxTokenChunker` | 将 Section 切分为适合模型上下文的 `Chunk` |
| `VectorStore` trait / `TurboVecVectorStore` | 向量存储后端：insert、search、delete、list |
| `KnowledgeBase` | 运行时知识库封装，组合 EmbeddingModel + VectorStore |
| `RAGMiddleware` | Agent 中间件，在 `pre_reply` 阶段触发知识库检索 |
| `TurbovecIndexAdapter` | 将 TurboVec 内存索引桥接到 VectorStore trait |

**适用场景**：基于文档的问答；将私有知识库注入 Agent；为 Agent 提供领域知识检索能力。

**前置阅读**：[模型抽象](./model.md)、[Agent 系统](./agent.md)、[记忆](./memory.md)

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 文档解析 (`parser`)

`Parser` trait 定义：

```rust
pub trait Parser: Send + Sync {
    fn parse(&self, content: &str) -> Result<Vec<Section>, ParserError>;
}
```

`TextParser` 支持两种模式：
- **纯文本**：按自然段落切分
- **Markdown**：按标题层级（`#`、`##`）切分，保留结构信息

`Section` 包含 `SectionContent` 枚举：`Title`（标题节点）和 `Body`（正文节点）。

### 2.2 文本分块 (`chunker`)

`Chunker` trait：

```rust
pub trait Chunker: Send + Sync {
    fn chunk(&self, sections: Vec<Section>) -> Result<Vec<Chunk>, ChunkerError>;
}
```

`ApproxTokenChunker` 基于 token 近似值分割：
- `chunk_size`：每个块的目标 token 数
- `chunk_overlap`：块之间的重叠 token 数
- 在段落边界处分割（paragraph-aware），避免句中断裂

`Chunk` 包含：
| 字段 | 说明 |
|------|------|
| `id` | 唯一 chunk ID |
| `content` | chunk 文本内容 |
| `metadata` | 来源文档信息 |

### 2.3 向量存储 (`vector_store`)

```rust
pub trait VectorStore: Send + Sync {
    async fn insert(&self, records: Vec<VectorRecord>) -> Result<(), VectorStoreError>;
    async fn search(&self, query: &[f64], top_k: usize) -> Result<Vec<VectorSearchResult>, VectorStoreError>;
    async fn delete(&self, ids: &[String]) -> Result<(), VectorStoreError>;
    async fn list(&self) -> Result<Vec<DocumentSummary>, VectorStoreError>;
}
```

`TurboVecVectorStore` 是基于 `turbovec` 库的高性能实现，支持：
- 懒加载 collection 创建
- 向量相似度检索
- 校准状态管理 (`CalibrationState`)

### 2.4 知识库 (`KnowledgeBase`)

```rust
pub struct KnowledgeBase {
    pub name: String,           // 知识库名称
    pub description: String,    // 描述（供 Agent 上下文使用）
    // embedding_model + vector_store + collection + metadata_filter
}
```

核心方法：
| 方法 | 说明 |
|------|------|
| `search(query, top_k)` | 用自然语言查询，自动 embedding + 向量检索 |
| `insert_documents(sections)` | 分块 → 向量化 → 存储 |
| `delete_documents(source)` | 按来源删除文档 |
| `list_documents()` | 列出知识库中所有文档摘要 |

### 2.5 `RAGMiddleware`

将知识库接入 Agent 生命周期：

| 模式 | 行为 |
|------|------|
| `Static` | 初始化时将知识库内容注入系统提示词 |
| `Dynamic` | 每次 `pre_reply` 时用用户查询检索，注入 `HintBlock` |

### 2.6 `TurbovecIndexAdapter`

桥接 `agent_scope_memory` 的 `MemoryVectorIndex` 到 `VectorStore` trait，实现记忆系统与 RAG 系统的互操作。

## 3. 快速示例 (Quick Example)

```rust
use agent_scope_rag::{TextParser, ApproxTokenChunker, KnowledgeBase, RAGMiddleware};
use agent_scope_embedding::EmbeddingModel; // your embedding impl

// 1. 创建解析器和分块器
let parser = TextParser::new();
let chunker = ApproxTokenChunker::new(512, 64);

// 2. 加载文档
let content = std::fs::read_to_string("docs/company-policy.md")?;
let sections = parser.parse(&content)?;
let chunks = chunker.chunk(sections)?;

// 3. 构建知识库
let kb = KnowledgeBase::new(
    "company-policy",
    "Company HR policies and guidelines",
    embedding_model,
    vector_store,
    "default".into(),
    None,
);
kb.insert_documents(chunks).await?;

// 4. 查询
let results = kb.search("What is our remote work policy?", 3).await?;
```

## 4. 关键用法模式 (Usage Patterns)

### 4.1 文档预处理流水线

```
原始文档 → Parser → [Section, Section, ...] → Chunker → [Chunk, Chunk, ...]
→ EmbeddingModel → [VectorRecord, ...] → VectorStore.insert()
```

### 4.2 将 RAG 接入 Agent

```rust
use agent_scope_rag::{RAGMiddleware, RAGMode};
use std::sync::Arc;

let rag = Arc::new(RAGMiddleware::new(
    RAGMode::Dynamic,
    vec![kb],
    embedding_model,
));
let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![rag],
)?;
```

### 4.3 多知识库支持

`RAGMiddleware` 可以管理多个知识库，为每个知识库生成独立的 Tool：

```rust
let kbs = vec![hr_kb, tech_kb, legal_kb];
let rag = RAGMiddleware::new(RAGMode::Dynamic, kbs, embedding_model);
// Agent 会看到 kb_search_hr、kb_search_tech、kb_search_legal 等工具
```

### 4.4 静态 vs 动态模式

| 模式 | 何时使用 | 权衡 |
|------|---------|------|
| `Static` | 知识库小且不变 | 每次请求都携带完整知识，无检索延迟 |
| `Dynamic` | 知识库大或频繁变化 | 按需检索，节省上下文，但增加一次 embedding 调用 |

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误 | 原因 |
|------|------|
| `ParserError::UnsupportedFormat` | 文档格式不支持 |
| `ParserError::ParseError` | 解析过程中错误 |
| `ChunkerError::EmptyInput` | 输入为空 |
| `ChunkerError::ChunkSizeTooSmall` | chunk_size 配置非法 |
| `VectorStoreError::CollectionNotFound` | 集合未创建 |
| `VectorStoreError::InsertError` | 插入向量失败 |
| `KnowledgeBaseError::NotInitialized` | 知识库未初始化 |

**不支持**：
- 多模态文档（PDF、图片）的解析在当前版本不支持
- 增量更新（仅更新变化文档）未实现
- 向量存储为进程内实现，不支持分布式存储

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L2**（核心 RAG 行为）
- **权威来源**: `specs/011-rag-system/spec.md`、`specs/016-turbovec-rag/spec.md`
- **已知偏差**:
  - `TurboVecVectorStore` 是 Rust 特有的高性能实现
  - Rust 侧 Parser/Chunker 当前仅支持文本和 Markdown，Python 侧支持更多格式

## 7. 相关模块 (See Also)

- [模型抽象](./model.md) — EmbeddingModel 和 ChatModel 在 RAG 中的使用
- [Agent 系统](./agent.md) — RAGMiddleware 在 Agent 中的位置
- [记忆](./memory.md) — Memory vs RAG（长期记忆 vs 文档知识库）
- [DashScope Provider](./dashscope.md) — embedding 模型来源
