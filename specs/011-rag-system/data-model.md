# Data Model: RAG System (Feature 011)

**Feature**: 011-rag-system
**Date**: 2026-07-31
**Source**: [spec.md](./spec.md) | [research.md](./research.md)

## Entity Overview

```
┌────────────────────┐     ┌──────────────────────┐     ┌─────────────────────┐
│  EmbeddingModel    │     │  EmbeddingCache      │     │  EmbeddingModelCard │
│  (trait)           │────>│  (trait)             │     │  (struct)           │
│  embed()           │     │  lookup()/store()    │     │  name/dimensions/   │
└────────┬───────────┘     └──────────────────────┘     │  supports_multimodal│
         │                                               └─────────────────────┘
         │ produces
         ▼
┌────────────────────┐
│  EmbeddingResponse │
│  (struct)          │
└────────┬───────────┘
         │
         │ drives
         ▼
┌────────────────────┐     ┌──────────────────────┐     ┌─────────────────────┐
│  KnowledgeBase     │────>│  VectorStore         │     │  RAGMiddleware      │
│  (struct)          │     │  (async trait)       │     │  (struct)           │
│  search/insert/    │     │  has_collection()    │     │  Mode: Static|      │
│  delete/list       │     │  create_collection() │     │  Agentic            │
└────────┬───────────┘     │  search()            │     └─────────────────────┘
         │                 │  insert()            │               │
         │ processes       │  delete()            │               │ implements
         ▼                 │  list_documents()    │               ▼
┌────────────────────┐     └──────────────────────┘     ┌─────────────────────┐
│  Parser → Chunker  │                                  │  Middleware         │
│  (pipeline)        │                                  │  (trait, Feature 7) │
│                    │                                  └─────────────────────┘
└────────┬───────────┘
         │
         │ produces
         ▼
┌────────────────────┐     ┌──────────────────────┐
│  Section           │────>│  Chunk               │
│  (struct)          │     │  (struct)            │
│  content/source/   │     │  content/source/     │
│  metadata          │     │  chunk_index/        │
└────────────────────┘     │  total_chunks/metadata│
                           └──────────┬───────────┘
                                      │
                                      │ embedded → inserted as
                                      ▼
                           ┌──────────────────────┐
                           │  VectorRecord        │
                           │  (struct)            │
                           │  vector/document_id/ │
                           │  chunk               │
                           └──────────────────────┘
```

---

## Entity 1: EmbeddingInput (enum)

**Crate**: `agent_scope_embedding`
**File**: `agent_scope_embedding/src/embedding.rs`

```rust
/// Input to an embedding model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmbeddingInput {
    /// Text input.
    Text(String),
    /// Multi-modal input (image bytes, audio, etc.).
    /// The model MUST have `supports_multimodal = true` to accept this variant.
    DataBlock(DataBlockData),
}

impl From<String> for EmbeddingInput {
    fn from(s: String) -> Self { EmbeddingInput::Text(s) }
}

impl From<&str> for EmbeddingInput {
    fn from(s: &str) -> Self { EmbeddingInput::Text(s.to_string()) }
}
```

**Fields**:
- `Text(String)`: 文本输入
- `DataBlock(DataBlockData)`: 多媒体数据块（复用 `agent_scope_types` 的 `DataBlock` 模式）

**Validation**:
- 当模型 `supports_multimodal = false` 时，传入 `DataBlock` 变体返回 `EmbeddingError::MultimodalNotSupported`

---

## Entity 2: EmbeddingResponse (struct)

**Crate**: `agent_scope_embedding`
**File**: `agent_scope_embedding/src/embedding.rs`

```rust
/// Embedding result from a model call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// One vector per input, each of length `model_card().dimensions`.
    pub embeddings: Vec<Vec<f32>>,
    /// Token usage statistics.
    pub usage: EmbeddingUsage,
}
```

**Fields**:
- `embeddings: Vec<Vec<f32>>` — 每个输入一个等长向量
- `usage: EmbeddingUsage` — token 统计

**Invariants**:
- `embeddings.len()` MUST equal 输入数量
- 每个 `embeddings[i].len()` MUST equal `EmbeddingModelCard.dimensions`

---

## Entity 3: EmbeddingUsage (struct)

**Crate**: `agent_scope_embedding`
**File**: `agent_scope_embedding/src/embedding.rs`

```rust
/// Token usage for an embedding request.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    /// Total tokens consumed.
    pub total_tokens: u32,
}
```

---

## Entity 4: EmbeddingModelCard (struct)

**Crate**: `agent_scope_embedding`
**File**: `agent_scope_embedding/src/embedding.rs`

```rust
/// Static metadata describing an embedding model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModelCard {
    /// Model identifier (e.g., "text-embedding-v3").
    pub name: String,
    /// Output vector dimensionality.
    pub dimensions: u32,
    /// Whether this model supports multi-modal inputs (images, etc.).
    pub supports_multimodal: bool,
}
```

**Fields**:
- `name: String` — 模型标识名称
- `dimensions: u32` — 输出向量维度
- `supports_multimodal: bool` — 是否支持多模态输入

---

## Entity 5: EmbeddingModel (trait)

**Crate**: `agent_scope_embedding`
**File**: `agent_scope_embedding/src/embedding.rs`

```rust
/// Trait for embedding model providers.
/// Semantically equivalent to Python AgentScope `EmbeddingModelBase.__call__`.
#[async_trait::async_trait]
pub trait EmbeddingModel: Send + Sync {
    /// Convert inputs to dense vectors.
    async fn embed(
        &self,
        inputs: Vec<EmbeddingInput>,
    ) -> Result<EmbeddingResponse, EmbeddingError>;

    /// Return static model metadata.
    fn model_card(&self) -> &EmbeddingModelCard;

    /// Whether this model handles multi-modal (DataBlock) inputs.
    fn supports_multimodal(&self) -> bool {
        self.model_card().supports_multimodal
    }
}
```

**Methods**:
- `embed(inputs: Vec<EmbeddingInput>) -> Result<EmbeddingResponse, EmbeddingError>` — 主嵌入方法（async）
- `model_card() -> &EmbeddingModelCard` — 返回静态模型元信息（sync）
- `supports_multimodal() -> bool` — 便利方法，默认委托给 model_card（sync）

**Lifecycle**:
- Provider 负责初始化（HTTP client、auth 等）
- 调用方通过 `Arc<dyn EmbeddingModel>` 共享
- 无显式 close/shutdown 方法——依赖 Drop

---

## Entity 6: EmbeddingCache (trait)

**Crate**: `agent_scope_embedding`
**File**: `agent_scope_embedding/src/cache.rs`

```rust
/// Trait for embedding result caches.
/// Key is typically a content hash (SHA-256).
pub trait EmbeddingCache: Send + Sync {
    /// Look up cached embeddings by key.
    fn lookup(&self, key: &str) -> Option<Vec<Vec<f32>>>;
    /// Store embeddings under a key.
    fn store(&self, key: &str, embeddings: Vec<Vec<f32>>);
}
```

**Methods**:
- `lookup(key: &str) -> Option<Vec<Vec<f32>>>` — 缓存查询
- `store(key: &str, embeddings: Vec<Vec<f32>>)` — 缓存写入

---

## Entity 7: FileEmbeddingCache (struct)

**Crate**: `agent_scope_embedding`
**File**: `agent_scope_embedding/src/cache.rs`

```rust
/// File-system backed embedding cache.
/// Each key → `{cache_dir}/{key}.json`.
pub struct FileEmbeddingCache {
    cache_dir: PathBuf,
}
```

**Fields**:
- `cache_dir: PathBuf` — 缓存目录路径

**Methods**:
- `new(cache_dir: PathBuf) -> Self` — 构造函数
- `impl EmbeddingCache for FileEmbeddingCache` — trait 实现

**Persistence**:
- 每个缓存项存储为独立 JSON 文件
- 文件内容：`Vec<Vec<f32>>` 的 JSON 数组
- 同时支持 `lookup` 和 `store` 操作
- 不实现 TTL/LRU——V1 范围外

---

## Entity 8: Section (struct)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/parser.rs`

```rust
/// A logical boundary unit within a source document.
/// Produced by Parser, consumed by Chunker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Section content (text or data block).
    pub content: SectionContent,
    /// Source filename identifier.
    pub source: String,
    /// Format-specific metadata (page number, slide index, sheet name, etc.).
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SectionContent {
    Text(String),
    DataBlock(DataBlockData),
}
```

**Fields**:
- `content: SectionContent` — Text(String) 或 DataBlock
- `source: String` — 源文件名
- `metadata: HashMap<String, String>` — 格式特定元数据（当前版本一般为空）

**Invariants**:
- 不同 Section 不可被 Chunker 跨越合并
- 空文件解析产生空 Vec（无 Section）

---

## Entity 9: Parser (trait)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/parser.rs`

```rust
/// Trait for document parsers.
/// Takes raw bytes + filename, returns logical sections.
pub trait Parser: Send + Sync {
    /// Parse raw file content into sections.
    fn parse(&self, file: Vec<u8>, filename: &str) -> Result<Vec<Section>, ParserError>;
}
```

**Methods**:
- `parse(file: Vec<u8>, filename: &str) -> Result<Vec<Section>, ParserError>` — 同步解析

**Implementations**:
- `TextParser` — 处理 `.txt`/`.md`，将整个文件作为单个 Section 输出
- 其他格式（PDF/PPT/Word/Excel/Image）返回 `ParserError::UnsupportedFormat`

---

## Entity 10: Chunk (struct)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/chunker.rs`

```rust
/// An indexable text fragment produced by Chunker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// The chunk's text content.
    pub content: String,
    /// Source filename (inherited from Section).
    pub source: String,
    /// Zero-based position within the document.
    pub chunk_index: usize,
    /// Total number of chunks from the same document.
    pub total_chunks: usize,
    /// Additional metadata (inherited + chunker-specific).
    pub metadata: HashMap<String, String>,
}
```

**Fields**:
- `content: String` — chunk 文本内容
- `source: String` — 源文件名
- `chunk_index: usize` — 文档内位置（0-based）
- `total_chunks: usize` — 同文档的总 chunk 数
- `metadata: HashMap<String, String>` — 额外元数据

**Invariants**:
- 同一 document 的所有 Chunk 的 `total_chunks` 相等
- `chunk_index` 从 0 连续递增
- Chunk 之间可能重叠（由 `overlap` 参数控制）

---

## Entity 11: Chunker (trait)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/chunker.rs`

```rust
/// Trait for text chunkers.
/// Consumes Sections, produces Chunks.
pub trait Chunker: Send + Sync {
    /// Split sections into indexable chunks.
    fn chunk(&self, sections: Vec<Section>) -> Result<Vec<Chunk>, ChunkerError>;
}
```

**Methods**:
- `chunk(sections: Vec<Section>) -> Result<Vec<Chunk>, ChunkerError>` — 同步切分

**Implementations**:
- `ApproxTokenChunker` — 基于近似 token 计数的滑动窗口切分

---

## Entity 12: ApproxTokenChunker (struct)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/chunker.rs`

```rust
/// Approximate token-count based chunker.
/// Uses heuristic token counting (English: words, CJK: chars/4).
pub struct ApproxTokenChunker {
    /// Target tokens per chunk.
    pub chunk_size: usize,
    /// Overlap tokens between adjacent chunks.
    pub overlap: usize,
}
```

**Fields**:
- `chunk_size: usize` — 目标 token 数
- `overlap: usize` — 相邻 chunk 重叠 token 数

**Validation**:
- `chunk_size > overlap`（否则报错）

---

## Entity 13: VectorStore (async trait)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/vector_store.rs`

```rust
/// Abstract trait for vector database backends.
/// Semantically equivalent to Python AgentScope `VectorStoreBase`.
/// No concrete implementation in this crate — downstream crates implement this trait.
#[async_trait::async_trait]
pub trait VectorStore: Send + Sync {
    /// Check if a collection exists.
    async fn has_collection(&self, name: &str) -> Result<bool, VectorStoreError>;

    /// Create a collection with the given vector dimensions.
    async fn create_collection(&self, name: &str, dimensions: u32) -> Result<(), VectorStoreError>;

    /// Search for similar vectors.
    async fn search(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        top_k: usize,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<VectorSearchResult>, VectorStoreError>;

    /// Insert vector records.
    async fn insert(
        &self,
        collection: &str,
        records: Vec<VectorRecord>,
    ) -> Result<(), VectorStoreError>;

    /// Delete all records for a document.
    async fn delete(
        &self,
        collection: &str,
        document_id: &str,
    ) -> Result<(), VectorStoreError>;

    /// List documents in a collection.
    async fn list_documents(
        &self,
        collection: &str,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<DocumentSummary>, VectorStoreError>;
}
```

**Methods**:
- `has_collection(name) -> Result<bool, VectorStoreError>` — 检查 collection 是否存在
- `create_collection(name, dimensions) -> Result<(), VectorStoreError>` — 创建 collection
- `search(collection, query_vector, top_k, metadata_filter) -> Result<Vec<VectorSearchResult>, VectorStoreError>` — 向量搜索
- `insert(collection, records) -> Result<(), VectorStoreError>` — 插入向量记录
- `delete(collection, document_id) -> Result<(), VectorStoreError>` — 删除文档所有记录
- `list_documents(collection, metadata_filter) -> Result<Vec<DocumentSummary>, VectorStoreError>` — 列出文档摘要

**Lifecycle**:
- 无显式 init/close 方法
- 实现者负责连接管理（内部维护 HTTP/gRPC client）

---

## Entity 14: VectorRecord (struct)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/vector_store.rs`

```rust
/// A single vector record to be inserted into the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Owning document identifier.
    pub document_id: String,
    /// The original chunk (content + metadata).
    pub chunk: Chunk,
}
```

**Fields**:
- `vector: Vec<f32>` — 嵌入向量
- `document_id: String` — 所属文档 ID
- `chunk: Chunk` — 原始 chunk

---

## Entity 15: VectorSearchResult (struct)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/vector_store.rs`

```rust
/// A search result from a vector store query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    /// Similarity score (higher = more similar).
    pub score: f32,
    /// Owning document identifier.
    pub document_id: String,
    /// The matched chunk.
    pub chunk: Chunk,
}
```

**Fields**:
- `score: f32` — 相似度分数（越大越相似）
- `document_id: String` — 所属文档 ID
- `chunk: Chunk` — 匹配到的 chunk

---

## Entity 16: DocumentSummary (struct)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/vector_store.rs`

```rust
/// Summary metadata for a document stored in a vector collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    /// Unique document identifier.
    pub document_id: String,
    /// Original source filename.
    pub source: String,
    /// Number of chunks in the store for this document.
    pub chunk_count: usize,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}
```

**Fields**:
- `document_id: String` — 文档唯一标识
- `source: String` — 原始源文件名
- `chunk_count: usize` — 该文档的 chunk 数量
- `metadata: HashMap<String, String>` — 额外元数据

---

## Entity 17: KnowledgeBase (struct)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/knowledge_base.rs`

```rust
/// Runtime knowledge base handle.
/// Wraps an embedding model + vector store + collection into a simple search/insert/delete/list API.
pub struct KnowledgeBase {
    /// Human-readable name for this KB (used as Tool name in agentic mode).
    pub name: String,
    /// Description for Agent context (e.g., "Company HR policies").
    pub description: String,
    /// Embedding model for query/document vectorization.
    embedding_model: Arc<dyn EmbeddingModel>,
    /// Vector store backend.
    vector_store: Arc<dyn VectorStore>,
    /// Collection name in the backing store.
    collection: String,
    /// Mandatory metadata filter — all operations are scoped to this filter.
    metadata_filter: Option<HashMap<String, String>>,
    /// Lazy init state.
    initialized: OnceCell<()>,
}
```

**Fields**:
- `name: String` — 知识库名称
- `description: String` — 面向 Agent 的描述
- `embedding_model: Arc<dyn EmbeddingModel>` — 嵌入模型
- `vector_store: Arc<dyn VectorStore>` — 向量存储
- `collection: String` — collection 名称
- `metadata_filter: Option<HashMap<String, String>>` — 安全边界过滤器
- `initialized: OnceCell<()>` — 懒初始化标记

**Methods**:
- `new(name, description, embedding_model, vector_store, collection, metadata_filter) -> Self`
- `search(queries: Vec<EmbeddingInput>, top_k: usize, score_threshold: Option<f32>) -> Result<Vec<VectorSearchResult>, KnowledgeBaseError>`
- `insert_document(chunks: Vec<Chunk>, document_id: Option<String>, document_metadata: Option<HashMap<String, String>>) -> Result<String, KnowledgeBaseError>`
- `delete_document(document_id: &str) -> Result<(), KnowledgeBaseError>`
- `list_documents() -> Result<Vec<DocumentSummary>, KnowledgeBaseError>`
- `async ensure_initialized() -> Result<(), KnowledgeBaseError>` (internal)

**State Transitions**:
```
[Uninitialized] --ensure_initialized()--> [Initialized] --search/insert/delete/list--> [Initialized]
       |                                          |
       v                                          v
  VectorStore::has_collection()           Operations use collection
  + VectorStore::create_collection()      (no init check on subsequent calls)
```

**Metadata Filter Override Logic** (FR-033):
```
final_metadata = merge(document_metadata, chunk.metadata)  // chunk wins
final_metadata = merge(final_metadata, metadata_filter)    // metadata_filter WINs
```

---

## Entity 18: RAGMiddleware (struct)

**Crate**: `agent_scope_rag`
**File**: `agent_scope_rag/src/rag_middleware.rs`

```rust
/// Middleware that integrates RAG knowledge retrieval into the Agent pipeline.
pub struct RAGMiddleware {
    /// Bound knowledge bases.
    knowledge_bases: Vec<Arc<KnowledgeBase>>,
    /// Operation mode.
    mode: RAGMode,
    /// Maximum search results to inject per KB.
    top_k: usize,
    /// Minimum similarity threshold.
    score_threshold: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RAGMode {
    /// Automatic context injection via pre_reply hook.
    Static,
    /// Tool-based search via post_acting hook.
    Agentic,
}
```

**Fields**:
- `knowledge_bases: Vec<Arc<KnowledgeBase>>` — 绑定知识库列表
- `mode: RAGMode` — 运行模式
- `top_k: usize` — 每个 KB 最多返回的结果数
- `score_threshold: Option<f32>` — 最低相似度阈值

**Methods**:
- `new(knowledge_bases, mode, top_k, score_threshold) -> Self`
- `impl Middleware for RAGMiddleware` — 实现 Feature 007 的 Middleware trait
  - `pre_reply(...)`: Static 模式下嵌入用户消息并搜索所有 KB，注入 HintBlock
  - `post_acting(...)`: Agentic 模式下注册 `search_knowledge` Tool

**Tool Registration (Agentic mode)**:
- 每个 KnowledgeBase 注册一个独立的 Tool：
  - Tool name: `search_{kb.name}`（sanitized）
  - Tool description: `kb.description`
  - Tool argument: `query: String`

---

## Entity Relationships (Summary)

```
EmbeddingModel (trait)
  ├── implemented by DashScopeEmbeddingModel (in agent_scope_dashscope)
  └── used by KnowledgeBase for query/document embedding

EmbeddingCache (trait)
  ├── implemented by FileEmbeddingCache
  └── optionally used by DashScopeEmbeddingModel for response caching

Parser (trait)
  ├── implemented by TextParser
  └── produces Vec<Section>

Chunker (trait)
  ├── implemented by ApproxTokenChunker
  ├── consumes Vec<Section>
  └── produces Vec<Chunk>

VectorStore (async trait)
  ├── no implementations in this crate
  ├── used by KnowledgeBase for storage/search
  └── implemented by downstream crates (Qdrant, Milvus, etc.)

KnowledgeBase (struct)
  ├── binds EmbeddingModel + VectorStore + collection
  ├── exposes search/insert/delete/list API
  └── used by RAGMiddleware

RAGMiddleware (struct)
  ├── implements Middleware (Feature 007)
  ├── binds Vec<KnowledgeBase>
  └── operates in Static or Agentic mode
```
