# Contract: EmbeddingModel

**Feature**: 011-rag-system
**Crate**: `agent_scope_embedding`
**Date**: 2026-07-31

## Trait Definition

```rust
#[async_trait::async_trait]
pub trait EmbeddingModel: Send + Sync {
    async fn embed(
        &self,
        inputs: Vec<EmbeddingInput>,
    ) -> Result<EmbeddingResponse, EmbeddingError>;

    fn model_card(&self) -> &EmbeddingModelCard;
    fn supports_multimodal(&self) -> bool { self.model_card().supports_multimodal }
}
```

## Input Types

```rust
pub enum EmbeddingInput {
    Text(String),
    DataBlock(DataBlockData),
}
```

## Output Types

```rust
pub struct EmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,  // embeddings[i].len() == model_card().dimensions
    pub usage: EmbeddingUsage,
}

pub struct EmbeddingUsage {
    pub total_tokens: u32,
}
```

## Metadata

```rust
pub struct EmbeddingModelCard {
    pub name: String,
    pub dimensions: u32,
    pub supports_multimodal: bool,
}
```

## EmbeddingCache Trait

```rust
pub trait EmbeddingCache: Send + Sync {
    fn lookup(&self, key: &str) -> Option<Vec<Vec<f32>>>;
    fn store(&self, key: &str, embeddings: Vec<Vec<f32>>);
}
```

## Error Types

```rust
pub enum EmbeddingError {
    /// API key not configured
    ApiKeyMissing(String),
    /// HTTP request failed
    HttpError(String),
    /// API returned an error
    ApiError { code: String, message: String },
    /// Multi-modal input rejected (model doesn't support it)
    MultimodalNotSupported,
    /// Response deserialization failed
    DeserializationError(String),
    /// Dimension mismatch: model returned unexpected vector length
    DimensionMismatch { expected: u32, got: usize },
}
```

## Behavioral Contract

1. **Embedding**:
   - `embed(inputs)` 返回 `embeddings` 长度等于 `inputs.len()`
   - 每个 `embeddings[i]` 长度等于 `model_card().dimensions`
   - `usage.total_tokens` 有意义的非零值（实际 API 调用时）
   - 当 `supports_multimodal() == false` 且 inputs 包含 `DataBlock` → `MultimodalNotSupported`

2. **Caching**:
   - `lookup(key)` 返回 `None` 当缓存未命中
   - `lookup(key)` 返回 `Some(embeddings)` 当缓存命中
   - `store(key, embeddings)` 覆盖已存在的缓存项
   - 缓存键应为嵌入输入的确定哈希（SHA-256），键本身作为 contract 参数

3. **Model Card**:
   - `model_card()` 返回静态引用（不可变元数据）
   - `supports_multimodal()` 提供默认实现，委托给 `model_card().supports_multimodal`

## DashScope Provider Contract

```rust
// In agent_scope_dashscope crate
pub struct DashScopeEmbeddingModel {
    http_client: Client,        // reqwest::Client
    api_key: String,
    base_url: String,
    model_card: EmbeddingModelCard,
    cache: Option<Arc<dyn EmbeddingCache>>,
}

impl DashScopeEmbeddingModel {
    pub fn new(api_key: String, model_card: EmbeddingModelCard) -> Self;
    pub fn with_cache(self, cache: Arc<dyn EmbeddingCache>) -> Self;
}
```

**API Endpoint**: `POST {base_url}/api/v1/services/embeddings/text-embedding/text-embedding`

**Request Body**:
```json
{
    "model": "text-embedding-v3",
    "input": {"texts": ["hello", "world"]}
}
```

**Response Body** (expected):
```json
{
    "output": {
        "embeddings": [
            {"text_index": 0, "embedding": [0.1, 0.2, ...]},
            {"text_index": 1, "embedding": [0.3, 0.4, ...]}
        ]
    },
    "usage": {"total_tokens": 2}
}
```

**Error Handling**:
- HTTP non-200 → `EmbeddingError::HttpError(status + body)`
- 返回向量数与请求数不匹配 → `EmbeddingError::ApiError`
- 向量维度与 `model_card.dimensions` 不匹配 → `EmbeddingError::DimensionMismatch`
- 无 API Key → `EmbeddingError::ApiKeyMissing`
