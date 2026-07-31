# Contract: KnowledgeBase & RAGMiddleware

**Feature**: 011-rag-system
**Crate**: `agent_scope_rag`
**Date**: 2026-07-31

## KnowledgeBase

### Constructor

```rust
impl KnowledgeBase {
    pub fn new(
        name: String,
        description: String,
        embedding_model: Arc<dyn EmbeddingModel>,
        vector_store: Arc<dyn VectorStore>,
        collection: String,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Self;
}
```

### Search

```rust
impl KnowledgeBase {
    /// Search the knowledge base with one or more queries.
    /// Results are deduplicated by (document_id, chunk_index) and sorted by score descending.
    pub async fn search(
        &self,
        queries: Vec<EmbeddingInput>,
        top_k: usize,
        score_threshold: Option<f32>,
    ) -> Result<Vec<VectorSearchResult>, KnowledgeBaseError>;
}
```

**Behavior**:
1. 如果 `queries` 为空 → `Ok(vec![])`
2. 静默丢弃不支持的输入类型（DataBlock 被发送到非多模态 EmbeddingModel）
3. 批量嵌入所有有效查询词
4. 对每个嵌入向量并发调用 `VectorStore::search()`
5. 按 `(document_id, chunk_index)` 去重（保留最高分）
6. 按 `score` 降序排序
7. 如果设置了 `score_threshold`，过滤掉分数小于阈值的所有结果
8. `top_k` 截断

### Insert Document

```rust
impl KnowledgeBase {
    /// Insert chunks into the knowledge base.
    /// Returns the document ID (auto-generated if not provided).
    pub async fn insert_document(
        &self,
        chunks: Vec<Chunk>,
        document_id: Option<String>,
        document_metadata: Option<HashMap<String, String>>,
    ) -> Result<String, KnowledgeBaseError>;
}
```

**Behavior**:
1. 如果 `chunks` 为空 → 返回空字符串文档 ID
2. 如果 `document_id` 为空 → 自动生成 UUID v4
3. 对每个 chunk 应用 metadata 合并：
   ```
   merged = document_metadata.unwrap_or_default()
   for (k,v) in chunk.metadata: merged.insert(k,v)
   for (k,v) in metadata_filter: merged.insert(k,v)   // metadata_filter WINS
   ```
4. 对所有 chunk 调用 `EmbeddingModel::embed()`
5. 验证 `embeddings.len() == chunks.len()` → 否则 `CountMismatch`
6. 构造 `Vec<VectorRecord>` 并调用 `VectorStore::insert()`
7. 返回 ID 文档

### Delete Document

```rust
impl KnowledgeBase {
    /// Remove a document and all its chunks from the knowledge base.
    pub async fn delete_document(
        &self,
        document_id: &str,
    ) -> Result<(), KnowledgeBaseError>;
}
```

**Behavior**:
1. 调用 `VectorStore::delete(collection, document_id)`
2. 如果文档不存在 → 幂等（成功）

### List Documents

```rust
impl KnowledgeBase {
    /// List all documents in this knowledge base.
    pub async fn list_documents(
        &self,
    ) -> Result<Vec<DocumentSummary>, KnowledgeBaseError>;
}
```

**Behavior**:
1. 调用 `VectorStore::list_documents(collection, metadata_filter)`
2. 过滤受 KB 的 `metadata_filter` 约束

### Lazy Initialization (internal)

```rust
impl KnowledgeBase {
    async fn ensure_initialized(&self) -> Result<(), KnowledgeBaseError> {
        // Uses OnceCell — only runs on first call
        // Checks has_collection() → create_collection() if needed
    }
}
```

**Behavior**:
1. 第一次调用 `search`/`insert`/`delete`/`list` 时触发
2. 调用 `VectorStore::has_collection(collection)`
3. 如果不存在 → `VectorStore::create_collection(collection, dimensions)`
   - `dimensions` 从 `embedding_model.model_card().dimensions` 获取
4. 后续操作跳过检查

## Error Types

```rust
pub enum KnowledgeBaseError {
    /// Embedding model returned error
    EmbeddingError(String),
    /// Vector store returned error
    VectorStoreError(String),
    /// Number of embeddings doesn't match number of chunks
    CountMismatch { expected: usize, got: usize },
    /// Dimension mismatch (embedding vs collection)
    DimensionMismatch { expected: u32, got: u32 },
}
```

## RAGMiddleware

### Constructor

```rust
impl RAGMiddleware {
    pub fn new(
        knowledge_bases: Vec<Arc<KnowledgeBase>>,
        mode: RAGMode,
        top_k: usize,
        score_threshold: Option<f32>,
    ) -> Self;
}
```

### Static Mode — pre_reply Hook

```rust
// Inside impl Middleware for RAGMiddleware
async fn pre_reply(
    &self,
    agent_state: &mut AgentState,
) -> Result<Option<ModelRequest>, MiddlewareError> {
    // 1. Extract the latest user message from agent_state
    // 2. For each KB: embed user message → search(top_k, score_threshold)
    // 3. Collect all results (deduplicate across KBs)
    // 4. Build HintBlock containing matched chunks with source citations
    // 5. Append HintBlock to agent_state.context
    // 6. Return None (let normal flow continue)
}
```

**Behavior**:
1. 从 `agent_state` 中提取最新用户消息
2. 对每个绑定的 KB 调用 `kb.search([user_message], top_k, score_threshold)`
3. 汇总跨 KB 的所有结果
4. 将匹配片段格式化为 `HintBlock`，包含 chunk 内容和 source 引用
5. 注入模型上下文
6. 如果零结果 → 不注入上下文，正常执行

### Agentic Mode — post_acting Hook

```rust
// Inside impl Middleware for RAGMiddleware
async fn post_acting(
    &self,
    agent_state: &mut AgentState,
    tools: &mut Vec<Arc<dyn Tool>>,
) -> Result<(), MiddlewareError> {
    // 1. For each KB: create a Tool with:
    //    - name: "search_{kb.name}" (sanitized: lowercase, underscore)
    //    - description: kb.description
    //    - argument: {"query": String}
    // 2. Register all tools
    // 3. Tool execution: embed query → search KB → format results as text
}
```

**Behavior**:
1. 每个 KB 注册一个独立的 Tool
2. Tool 名称：`"search_{sanitized_kb_name}"`
3. Tool 描述：来自 `kb.description`
4. Tool 参数：`query: String`
5. Tool 执行：`kb.search([query])` → 格式化结果供 LLM 消费
6. 每个钩子调用只注册一次工具（幂等——后续调用不重复注册）

### RAGMode Enum

```rust
pub enum RAGMode {
    /// Automatic context injection on every turn.
    Static,
    /// Tool-based: LLM decides when/if to search.
    Agentic,
}
```

## Middleware Integration Contract

RAGMiddleware 实现 Feature 007 的 `Middleware` trait：

```rust
// Pre-existing trait (Feature 007), re-stated for clarity:
pub trait Middleware: Send + Sync {
    fn name(&self) -> &str;
    async fn pre_reply(
        &self,
        agent_state: &mut AgentState,
    ) -> Result<Option<ModelRequest>, MiddlewareError> { Ok(None) }
    async fn post_reply(
        &self,
        agent_state: &mut AgentState,
        response: &ChatResponse,
    ) -> Result<(), MiddlewareError> { Ok(()) }
    async fn post_acting(
        &self,
        agent_state: &mut AgentState,
        tools: &mut Vec<Arc<dyn Tool>>,
    ) -> Result<(), MiddlewareError> { Ok(()) }
}
```

- Static mode → 覆盖 `pre_reply`
- Agentic mode → 覆盖 `post_acting`
- `name()` → 返回 `"RAGMiddleware"`
- 不覆盖 `post_reply` → 使用默认空实现
