# 参考:RAG 系统(`agent_scope_rag`)

> 详细 API 参考:`Parser`、`Chunker`、`VectorStore`、`KnowledgeBase`、`RAGMiddleware`、`TurbovecVectorStore`。所有签名以当前源码为准。

## 1. 管线总览

```text
原始文档 → Parser → [Section] → Chunker → [Chunk]
→ EmbeddingModel → [VectorRecord] → VectorStore.insert()
→ 查询:KnowledgeBase.search(query) → embedding → 向量检索 → 结果
```

| 组件 | 职责 |
|------|------|
| `Parser` / `TextParser` | 解析文档(纯文本、Markdown),输出 `Section` |
| `Chunker` / `ApproxTokenChunker` | 把 Section 切分为 `Chunk` |
| `VectorStore` trait / `TurbovecVectorStore` | 向量存储后端 |
| `KnowledgeBase` | 运行时知识库封装(Embedding + VectorStore + collection) |
| `RAGMiddleware` | Agent 中间件,接入检索 |
| `TurbovecIndexAdapter` | 把 TurboVec 内存索引桥接到 `VectorStore` trait |

## 2. `Parser` / `TextParser`

```rust
pub trait Parser: Send + Sync {
    fn parse(&self, content: &str) -> Result<Vec<Section>, ParserError>;
}
```

`TextParser::new()` 支持两种模式:

- **纯文本**:按自然段落切分。
- **Markdown**:按标题层级(`#`、`##`)切分,保留结构。

`Section` 包含 `SectionContent` 枚举:`Title`(标题节点)、`Body`(正文节点)。

## 3. `Chunker` / `ApproxTokenChunker`

```rust
pub trait Chunker: Send + Sync {
    fn chunk(&self, sections: Vec<Section>) -> Result<Vec<Chunk>, ChunkerError>;
}
```

`ApproxTokenChunker::new(chunk_size, chunk_overlap)`:

- 基于 token 近似值分割。
- `chunk_size`:每块目标 token 数。
- `chunk_overlap`:块间重叠 token 数。
- 在段落边界处分割(paragraph-aware),避免句中断裂。

`Chunk` 字段:`id`、`content`、`metadata`。

## 4. `VectorStore` trait

```rust
pub trait VectorStore: Send + Sync {
    async fn insert(&self, records: Vec<VectorRecord>) -> Result<(), VectorStoreError>;
    async fn search(&self, query: &[f64], top_k: usize) -> Result<Vec<VectorSearchResult>, VectorStoreError>;
    async fn delete(&self, ids: &[String]) -> Result<(), VectorStoreError>;
    async fn list(&self) -> Result<Vec<DocumentSummary>, VectorStoreError>;
}
```

`TurbovecVectorStore::new(bit_width: usize) -> Result<Self, VectorStoreError>` 是基于 `turbovec` 的高性能实现,支持懒加载 collection、向量相似度检索、`CalibrationState` 校准状态。

## 5. `KnowledgeBase`

```rust
pub fn new(
    name: String,                       // 知识库名称
    description: String,                // 描述(Agent 上下文用)
    embedding_model: Arc<dyn EmbeddingModel>,
    vector_store: Arc<dyn VectorStore>,
    collection: String,                 // 后端 collection 名
    metadata_filter: Option<HashMap<String, String>>, // 强制元数据过滤
) -> Self
```

底层 collection **懒创建**(首次操作时,经 `OnceCell` 保证一次性初始化)。

核心方法:

| 方法 | 说明 |
|------|------|
| `search(queries, top_k)` | 自然语言查询,自动 embedding + 向量检索;按 `(document_id, chunk_index)` 去重,分数降序 |
| `insert_documents(...)` | 分块 → 向量化 → 存储 |
| `delete_documents(source)` | 按来源删除 |
| `list_documents()` | 列出所有文档摘要 |
| `model_card()` | 返回 embedding 模型卡片 |

## 6. `RAGMiddleware`

```rust
pub fn new(
    knowledge_bases: Vec<Arc<KnowledgeBase>>,
    mode: RAGMode,
    top_k: usize,
    score_threshold: Option<f32>,
) -> Self
```

> **注意**:实际签名的 `mode` 是 `RAGMode`,取值 **`Static`** 或 **`Agentic`**(不是旧的 `Dynamic`;`docs/zh/modules/rag.md` 中的旧示例已过时)。

| 模式 | 行为 |
|------|------|
| `Static` | 每次 `pre_reply` 提取最新用户消息,检索所有 KB,把匹配 chunk 作为 `HintBlock` 注入输入消息 |
| `Agentic` | 通过 `pre_reasoning` 为每个 KB 添加工具 schema,由模型决定何时检索 |

Agentic 模式需要把搜索工具注册进 `ToolKit`:

```rust
use agent_scope_rag::{RAGMiddleware, RAGMode, KnowledgeBase};
use agent_scope_tool::ToolKit;
use std::sync::Arc;

let rag = Arc::new(RAGMiddleware::new(vec![kb], RAGMode::Agentic, 5, None));
let search_tools = rag.into_search_tools();  // Vec<Arc<dyn Tool>>,每个 KB 一个
let mut toolkit = ToolKit::new();
for tool in search_tools { toolkit.register(tool); }

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![rag],
)?;
```

Static 模式直接作为 middleware 注入即可。

## 7. 完整接入示例(参考 pi-rust)

```rust
use std::sync::Arc;
use agent_scope_embedding::EmbeddingModelCard;
use agent_scope_dashscope::DashScopeEmbeddingModel;
use agent_scope_rag::{KnowledgeBase, RAGMiddleware, RAGMode, TurbovecVectorStore};

// Embedding 模型
let embedding = Arc::new(DashScopeEmbeddingModel::new(
    api_key.clone(),
    EmbeddingModelCard::new("text-embedding-v3", 1024, false),
));

// 向量存储
let vector_store = Arc::new(TurbovecVectorStore::new(4)?);

// 知识库
let kb = Arc::new(KnowledgeBase::new(
    "project".to_string(),
    "Project documents indexed for retrieval".to_string(),
    embedding,
    vector_store,
    "project".to_string(),
    None,
));

// RAG middleware
let rag = Arc::new(RAGMiddleware::new(vec![kb], RAGMode::Static, 5, None));

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![rag],
)?;
```

## 8. 多知识库

`RAGMiddleware` 可管理多个 KB。Agentic 模式下每个 KB 生成独立搜索工具(名称经 `sanitize_kb_name` 规范化);Static 模式下每次注入合并所有 KB 的匹配结果。

## 9. 文档预处理与导入

```rust
use agent_scope_rag::{TextParser, ApproxTokenChunker, KnowledgeBase};
use agent_scope_embedding::{EmbeddingModel, EmbeddingInput};

let parser = TextParser::new();
let chunker = ApproxTokenChunker::new(512, 64);

let content = std::fs::read_to_string("docs/company-policy.md")?;
let sections = parser.parse(&content)?;
let chunks = chunker.chunk(sections)?;

let kb = KnowledgeBase::new(
    "company-policy",
    "Company HR policies and guidelines",
    embedding_model,
    vector_store,
    "default".into(),
    None,
);
kb.insert_documents(chunks).await?;

let results = kb.search(vec![EmbeddingInput::Text("remote work policy".into())], 3).await?;
```

## 10. 错误

| 错误 | 触发条件 |
|------|----------|
| `ParserError::UnsupportedFormat` | 文档格式不支持 |
| `ParserError::ParseError` | 解析过程错误 |
| `ChunkerError::EmptyInput` | 输入为空 |
| `ChunkerError::ChunkSizeTooSmall` | chunk_size 配置非法 |
| `VectorStoreError::CollectionNotFound` | 集合未创建 |
| `VectorStoreError::InsertError` | 插入向量失败 |
| `KnowledgeBaseError::NotInitialized` | 知识库未初始化 |
| `KnowledgeBaseError::VectorStoreError` | 底层向量存储错误 |

## 11. 不支持的能力

- 多模态文档(PDF、图片)解析当前不支持。
- 增量更新(仅更新变化文档)未实现。
- 向量存储为进程内实现,不支持分布式存储。
- `TurboVecVectorStore` 是 Rust 特有实现;`Parser`/`Chunker` 当前仅支持文本与 Markdown。

## 12. Memory vs RAG

- **Memory**(`agent_scope_memory`):跨会话长期事实(用户偏好、项目状态),`MEMORY.md` 索引 + 检索。
- **RAG**(`agent_scope_rag`):文档知识库,向量检索注入上下文。
- 可组合:同一个 Agent 的 middlewares 可同时挂 `MemoryMiddleware` 与 `RAGMiddleware`。
