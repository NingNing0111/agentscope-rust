# Research: Turbovec RAG 向量存储实现

**Feature**: 016-turbovec-rag
**Date**: 2026-07-31

## 1. turbovec IdMapIndex 集成模式

**Decision**: 使用 `turbovec::IdMapIndex` 而非 `TurboQuantIndex` 作为每个 collection 的后端

**Rationale**:
- `IdMapIndex` 提供稳定的 `u64` external ID（支持 remove 操作后 ID 保持不变）
- `TurboQuantIndex` 使用槽位索引（swap_remove 后会变化），不适合 document-level 删除
- `IdMapIndex::search` 返回 `(scores: Vec<f32>, ids: Vec<u64>)` ，可直接映射回 document_id
- `IdMapIndex::add_with_ids(vectors, &[u64])` 允许控制 internal ID 分配

**Alternatives considered**:
- `TurboQuantIndex` + 自有 ID 映射层：增加复杂度，且 turbovec 已有 `IdMapIndex` 专门解决此问题
- 每次 search 后自己做 ID→document 映射：可行但多一层间接寻址

## 2. 异步桥接策略

**Decision**: 使用 `tokio::task::spawn_blocking` 将 turbovec 的同步操作桥接到 async 上下文

**Rationale**:
- `VectorStore` trait 所有方法都是 `async fn`（Feature 011 契约）
- turbovec 所有操作（`add_with_ids`、`search`、`write`、`load`）都是同步的
- `add_with_ids` 和 `search` 在百万级向量上可能是计算密集（毫秒级），不应阻塞 async runtime
- `insert` 包含 L2 归一化 + turbovec 编码，属于 CPU 密集型操作

**Pattern**:
```rust
// 伪代码示意
async fn search(...) -> Result<Vec<VectorSearchResult>, VectorStoreError> {
    let store = self.inner.read().await;
    let query = query_vector;
    let top_k = top_k;
    // CPU 密集操作放到 blocking pool
    tokio::task::spawn_blocking(move || {
        store.get(collection)?.index.search(&query, top_k)
    }).await?
}
```

**Alternatives considered**:
- `tokio::sync::Mutex` + 同步方法：可避免 spawn_blocking，但 trait 契约要求 async
- 用 `std::thread::spawn` 替代 spawn_blocking：不推荐，tokio 的 spawn_blocking 专为此场景设计

## 3. 内部并发模型

**Decision**: 使用 `tokio::sync::RwLock<HashMap<String, Collection>>` 保护内部状态

**Rationale**:
- `search` 是读密集型（turbovec 自身支持并发 search），应允许多个并发 read
- `insert`/`delete` 是写操作，需要独占访问
- tokio RwLock 适合 async 上下文（避免 std::sync::RwLock 在 spawn_blocking 中的死锁风险）
- 但 turbovec 调用发生在 spawn_blocking 内部，此时持有 tokio RwLock guard 跨 await 是安全的（guard 通过 `Send + Sync` 传递）

**Fine print**: `search` 虽然是 `&self`（turbovec 侧），但 `insert` 需要 `&mut self`（turbovec 侧）。在 `spawn_blocking` 内部我们需要对 `HashMap<String, Collection>` 做读写分离。方案：
- `search` → `RwLock::read()` → 在 blocking 线程中调用 `index.search()`
- `insert`/`delete` → `RwLock::write()` → 在 blocking 线程中调用 `index.add_with_ids()` / `index.remove()`

## 4. Internal ID 生成策略

**Decision**: 使用 `(document_id, chunk_index)` 的确定性哈希生成 u64 internal ID

**Rationale**:
- 需要从 document_id 快速找到对应的所有 u64 ID（delete 操作）
- `(document_id, chunk_index)` 天然唯一
- 确定性哈希允许重复插入同一 chunk 时生成相同 ID（幂等性）

**Hash function**: 使用 `std::collections::hash_map::DefaultHasher` 对 `(document_id.as_str(), chunk_index)` 做哈希。碰撞概率在 64 位空间内可忽略（生日攻击：10^6 条记录碰撞概率 ≈ 10^-7）。

**Alternatives considered**:
- UUID v4 → u64 截断：截断 128→64 位碰撞概率不可接受
- 自增 counter：简单但无法保证确定性（重启后 counter 重置）
- `document_id` 的 SHA-256 前 8 字节：计算成本高于简单的 DefaultHasher

## 5. L2 归一化处理

**Decision**: 插入前对输入向量执行 L2 归一化，保证余弦相似度等价性

**Rationale**:
- turbovec 内部执行归一化（单位方向编码），但使用欧几里得范数
- 如果输入向量未归一化，turbovec 的内积分数 = 余弦相似度 × ||v||
- 为与 mock VectorStore 的余弦相似度行为一致，必须在插入前归一化
- search 的 query_vector 也需要归一化

**Implementation**: 对每个向量计算 `norm = sqrt(sum(x_i^2))`，如果 `norm > 1e-10` 则除以 norm；否则保留原向量（turbovec 将其编码为 score=0）

## 6. 持久化格式设计

**Decision**: 使用"manifest.json + 多个 .tvim 文件"的目录结构

**Rationale**:
- turbovec 的 `.tvim` 格式已经支持原子写入（write → fsync → rename）
- 元数据（chunk metadata, document_id 映射）不适合嵌入 turbovec 文件，更适合独立 JSON
- manifest 记录 collection 列表和版本号，支持前向兼容性
- 每个 collection 独立文件允许部分损坏时其他 collection 可恢复

**Directory layout**:
```text
{store_path}/
├── manifest.json          # { version: 1, collections: { "kb": { dim: 1536, bit_width: 4 } } }
├── {collection_name}.tvim # turbovec IdMapIndex file
└── {collection_name}.meta # JSON: { id_map: { u64_id: ChunkMetadata }, doc_index: { doc_id: [u64_ids] } }
```

## 7. metadata_filter 实现策略

**Decision**: 使用 turbovec 的 allowlist/slot-mask 进行 post-filter 而非 pre-filter

**Rationale**:
- turbovec 的 `search` 不支持内置 metadata 过滤（它只存储向量）
- 最佳路径：先执行 turbovec search（利用 SIMD 加速），然后在结果上应用 metadata_filter
- 为了不损失 recall：search 时请求 `top_k * overscan_factor`（如 2x）个候选，然后过滤
- turbovec 的 allowlist 用于按 id 过滤，需先根据 metadata_filter 构建允许的 internal ID 集合

**实际上 metadata_filter 走两层**：
1. 如果 metadata_filter 存在，先扫描 chunk metadata 找出匹配的 internal IDs → 构建 allowlist
2. 将 allowlist 传给 turbovec search（SIMD 内核层面跳过不匹配的 block，零 recall 损失）
3. 返回的 top_k 结果已经在 filtered set 中

## 8. 错误映射

**Decision**: 将 turbovec 错误类型映射为 `VectorStoreError` 的现有变体

| turbovec 错误 | VectorStoreError |
|---------------|------------------|
| `ConstructError::BitWidthOutOfRange` | `BackendError(msg)` — 构造时捕获，不会在运行时出现 |
| `AddError::DimMismatch` | `DimensionMismatch { expected, got }` |
| `AddError::InvalidInputValue` | `BackendError(msg)` — NaN/Inf 在更早层被拦截 |
| `SearchError::*` | `BackendError(msg)` |
| `io::Error` (load/save) | `BackendError(msg)` 或自定义 IO 错误 |

## 9. CalibrationState 暴露

**Decision**: 在 `TurbovecVectorStore` 上暴露 `calibration_state(collection: &str)` 方法

**Rationale**:
- turbovec 的 TQ+ 标定影响 recall（WarmingUp → Fitted 后 recall 提升）
- 用户应在 KnowledgeBase 日志中看到标定状态，以便在达到 1000 向量阈值前了解 recall 可能较低
- 这不是 `VectorStore` trait 的方法，而是 `TurbovecVectorStore` 特有的扩展 API
