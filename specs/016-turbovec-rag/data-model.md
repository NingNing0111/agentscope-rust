# Data Model: Turbovec RAG 向量存储实现

**Feature**: 016-turbovec-rag
**Date**: 2026-07-31

## Entity Relationship

```
┌─────────────────────────────────────────────────────┐
│                   TurbovecVectorStore               │
│  bit_width: usize                                   │
│  collections: RwLock<HashMap<String, Collection>>   │
└───────────────┬─────────────────────────────────────┘
                │ 1:N
                ▼
┌─────────────────────────────────────────────────────┐
│                     Collection                      │
│  dim: usize                                        │
│  index: IdMapIndex          ← turbovec 向量索引      │
│  chunk_meta: HashMap<u64, ChunkMeta>  ← ID→元数据   │
│  doc_index: HashMap<String, Vec<u64>> ← doc→chunks  │
└───────────────┬─────────────────────────────────────┘
                │ 1:N (via chunk_meta)
                ▼
┌─────────────────────────────────────────────────────┐
│                     ChunkMeta                       │
│  document_id: String                               │
│  chunk_index: usize                                │
│  total_chunks: usize                               │
│  source: String                                    │
│  metadata: HashMap<String, String>                 │
└─────────────────────────────────────────────────────┘
```

## Entity Definitions

### TurbovecVectorStore

| Field | Type | Description |
|-------|------|-------------|
| `bit_width` | `usize` | 全局压缩位宽 (2/3/4)，构造时设定，不可变 |
| `collections` | `RwLock<HashMap<String, Collection>>` | collection 名称 → Collection 实例的映射 |

**Invariants**:
- `bit_width` ∈ {2, 3, 4}
- 同一 store 中所有 collection 共享相同 `bit_width`

**Lifecycle**:
1. `new(bit_width)` → 空 store
2. 首次 `insert` 或 `create_collection` → 创建 collection
3. 所有操作通过 `collections` map 路由
4. `save(path)` → 序列化到磁盘
5. `load(path)` → 从磁盘恢复

### Collection

| Field | Type | Description |
|-------|------|-------------|
| `dim` | `usize` | 向量维度（首条 insert 时确定，不可变） |
| `index` | `IdMapIndex` | turbovec 的压缩向量索引 |
| `chunk_meta` | `HashMap<u64, ChunkMeta>` | internal_id → chunk 元数据 |
| `doc_index` | `HashMap<String, Vec<u64>>` | document_id → 属于该 doc 的所有 internal_ids |
| `next_internal_id` | `u64` | 自增 ID 计数器（用于确定性生成已不足以区分时） |

**Invariants**:
- `dim` > 0 && `dim % 8 == 0` && `dim <= 16384`
- 所有 `doc_index` 中的 u64 ID 必须在 `chunk_meta` 中存在
- `index.len()` == `chunk_meta.len()`
- 所有 `Vec<u64>` in `doc_index` 非空

**Lifecycle**:
1. `create_collection(name, dim)` → 创建空索引
2. `insert(records)` → L2 normalize → internal_id 生成 → `add_with_ids` → 更新 meta
3. `search(query, k)` → L2 normalize query → 可选 allowlist → `index.search` → 映射回 Chunk
4. `delete(doc_id)` → 查 `doc_index` → 逐个 `index.remove(internal_id)` → 清理 meta

### ChunkMeta

| Field | Type | Description |
|-------|------|-------------|
| `document_id` | `String` | 所属文档 ID |
| `chunk_index` | `usize` | chunk 在文档内的序号（从 0 开始） |
| `total_chunks` | `usize` | 文档的总 chunk 数 |
| `source` | `String` | 源文件名 |
| `metadata` | `HashMap<String, String>` | 额外元数据（键值对） |

**Derivation**: 从 `VectorRecord.chunk` 提取（不含 `chunk.content` —— content 不存储在向量存储中，需要时从原始文档读取）
**Note**: `content` 字段不存储在 turbovec 索引中。搜索返回时，chunk content 必须从 `ChunkMeta` 重新构建为 `Chunk` 结构体返回。

### InternalId

**Type**: `u64`

**Generation algorithm**:
```rust
fn generate_internal_id(document_id: &str, chunk_index: usize) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    document_id.hash(&mut hasher);
    chunk_index.hash(&mut hasher);
    hasher.finish()
}
```

**Collision resolution**: 如果生成 ID 已存在（碰撞），使用自增 counter 替代：`next_internal_id += 1`。

### StoreManifest (持久化)

| Field | Type | Description |
|-------|------|-------------|
| `version` | `u32` | 格式版本号（v1 = 1） |
| `bit_width` | `usize` | store 的压缩位宽 |
| `collections` | `HashMap<String, CollectionManifestEntry>` | 所有 collection 的元信息 |

### CollectionManifestEntry

| Field | Type | Description |
|-------|------|-------------|
| `dim` | `usize` | 向量维度 |
| `n_vectors` | `usize` | 向量总数（用于验证加载一致性） |

## State Transitions

### Collection State

```
                    create_collection()
[Not Exist] ──────────────────────────► [Empty]
                                             │
                                    insert() │
                                             ▼
                                        [Populated]
                                             │
                          insert() / search() │
                                             ▼
                                        [Populated]  ← 正常运行状态
                                             │
                             delete(all docs) │
                                             ▼
                                        [Empty]
```

### Calibration State (turbovec 内部)

```
                        首次 add(rows < 1000)
[WarmingUp] ───────────────────────────────────► [WarmingUp]
                                                   │
                              add 达到 1000 总量    │
                                                   ▼
                                               [Fitted]  ← 标定完成，recall 最优
                                                   
                        从文件加载 warming-up 状态
[WarmingUp] ──────────────────────────────────► [Identity]  ← 永久放弃 TQ+
```

## Validation Rules

| Rule | Enforcement Point | Error |
|------|-------------------|-------|
| `bit_width ∈ {2,3,4}` | `TurbovecVectorStore::new()` | `VectorStoreError::BackendError` |
| `dim % 8 == 0` | `create_collection()` | `VectorStoreError::BackendError` |
| `1 ≤ dim ≤ 16384` | `create_collection()` | `VectorStoreError::BackendError` |
| `vector.len() == dim` | `insert()` before turbovec add | `VectorStoreError::DimensionMismatch` |
| `query_vector.len() == dim` | `search()` | `VectorStoreError::DimensionMismatch` |
| 输入向量非 NaN/Inf | L2 normalize pre-check | `VectorStoreError::BackendError` |
| 空 records → no-op | `insert()` guard | 返回 `Ok(())` |
