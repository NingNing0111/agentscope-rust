# Contract: VectorStore

**Feature**: 011-rag-system
**Crate**: `agent_scope_rag`
**Date**: 2026-07-31

## Data Types

```rust
/// A single record to store in the vector database.
pub struct VectorRecord {
    pub vector: Vec<f32>,
    pub document_id: String,
    pub chunk: Chunk,
}

/// A search hit.
pub struct VectorSearchResult {
    pub score: f32,
    pub document_id: String,
    pub chunk: Chunk,
}

/// Summary of a document stored in the collection.
pub struct DocumentSummary {
    pub document_id: String,
    pub source: String,
    pub chunk_count: usize,
    pub metadata: HashMap<String, String>,
}
```

## Trait Definition

```rust
#[async_trait::async_trait]
pub trait VectorStore: Send + Sync {
    async fn has_collection(&self, name: &str) -> Result<bool, VectorStoreError>;
    async fn create_collection(&self, name: &str, dimensions: u32) -> Result<(), VectorStoreError>;
    async fn search(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        top_k: usize,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<VectorSearchResult>, VectorStoreError>;
    async fn insert(
        &self,
        collection: &str,
        records: Vec<VectorRecord>,
    ) -> Result<(), VectorStoreError>;
    async fn delete(
        &self,
        collection: &str,
        document_id: &str,
    ) -> Result<(), VectorStoreError>;
    async fn list_documents(
        &self,
        collection: &str,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<DocumentSummary>, VectorStoreError>;
}
```

## Error Types

```rust
pub enum VectorStoreError {
    /// The collection does not exist
    CollectionNotFound(String),
    /// The collection already exists (on create_collection)
    CollectionAlreadyExists(String),
    /// Dimension mismatch: expected N, got M
    DimensionMismatch { expected: u32, got: usize },
    /// Backend-specific error
    BackendError(String),
    /// Operation timed out
    Timeout(String),
}
```

## Behavioral Contract

### has_collection
- 返回 `true` 如果 collection 存在
- 返回 `false` 如果 collection 不存在
- collection 名称区分大小写（case-sensitive）

### create_collection
- 如果 collection 已存在且维度匹配 → 成功（幂等）
- 如果 collection 已存在且维度不匹配 → `Err(CollectionAlreadyExists)` 或 `Err(DimensionMismatch)`
- 如果 collection 不存在 → 创建并返回 `Ok(())`

### search
- `query_vector` 长度必须等于 collection 的 `dimensions`
- `top_k=0` → 返回空列表
- `metadata_filter` 为 `None` → 不过滤
- `metadata_filter` 为 `Some(filter)` → 仅返回 metadata 包含所有 filter 键值对的记录
- 返回结果按 score 降序排列
- `document_id` 不可为空，chunk 必须携带完整的 content/source/chunk_index/total_chunks/metadata

### insert
- `records` 为空 → 空操作，返回 `Ok(())`
- `records[i].vector.len()` 必须等于 collection 维度
- 重复 `(collection, document_id, chunk_index)` 的行为由实现定义（overwrite 或 append），trait 无规定
- `chunk.metadata` 必须被存储以便后续 metadata_filter 查询

### delete
- 删除指定 `document_id` 的所有记录
- 如果 `document_id` 不存在 → 成功（幂等）
- 操作完成后，该 document 的分片不应再出现在 search/list_documents 结果中

### list_documents
- 返回 collection 中所有不重复的文档摘要
- `metadata_filter` 为 `Some(filter)` → 仅返回符合过滤条件的文档
- `chunk_count` 为每个文档的实际 chunk 数量
- `source` 从 chunk.source 提取

## Important Notes

1. **此 trait 是契约定义** — 不提供任何默认实现
2. **具体实现**（如 Qdrant, Milvus, MongoDB）由下游 crate 提供
3. **无生命周期方法** — 无 open/close/init/drop 方法。连接管理由实现者内部负责
4. **重复插入的语义** — trait 无规定，但实现者应文档化其选择（overwrite 或 append）
5. **metadata_filter 语义** — 精确匹配（所有 filter 键值对必须出现在记录的 metadata 中）。不做模糊匹配或前缀匹配
