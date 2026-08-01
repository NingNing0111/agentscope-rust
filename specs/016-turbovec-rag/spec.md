# Feature Specification: Turbovec RAG 向量存储实现

**Feature Branch**: `016-turbovec-rag`

**Created**: 2026-07-31

**Status**: Draft

**Input**: User description: "参考项目 turbovec 这个项目，新建一个turbovec的rag实现"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 本地高性能向量存储 (Priority: P1)

开发者在 AgentScope Rust 中创建基于 turbovec 的本地向量存储，无需安装和配置外部数据库（如 Qdrant/Milvus），即可进行向量插入和语义搜索。turbovec 利用 Google TurboQuant 算法将向量压缩至 2-4 bit/维度，在 4GB 内存中可容纳 1000 万条 1536 维向量，搜索速度比 FAISS 快 19-31%（ARM 平台）。

**Why this priority**: 这是整个 feature 的核心——提供一个零依赖、高性能的本地向量存储实现，让开发者在不引入外部基础设施的情况下完成 RAG 检索。

**Independent Test**: 创建 `TurbovecVectorStore` 实例，插入 100 条向量，执行 top-10 搜索，验证返回结果按分数降序排列且分数在 [0,1] 区间。

**Acceptance Scenarios**:

1. **Given** 一个空的 `TurbovecVectorStore` 实例, **When** 调用 `insert("collection-1", records)` 插入 50 条 `VectorRecord`（含向量、document_id、chunk）, **Then** 后续 `search("collection-1", query_vector, top_k=10)` 返回这 50 条中与 query 最相似的 10 条结果
2. **Given** 同一 collection 中插入了 3 个不同 document 的 chunk（doc-A: 5 chunks, doc-B: 3 chunks, doc-C: 2 chunks）, **When** 调用 `delete("collection-1", "doc-B")`, **Then** 后续搜索不再返回 doc-B 的任何 chunk，`list_documents("collection-1")` 只返回 doc-A 和 doc-C
3. **Given** 向量维度为 1536、bit_width=4 的索引, **When** 插入 100 条向量, **Then** `has_collection("collection-1")` 返回 `true`

---

### User Story 2 - 索引持久化 (Priority: P2)

开发者将 turbovec 向量存储的状态保存到磁盘文件（`.tv` 或 `.tvim` 格式），之后重新加载恢复全部数据，无需重新索引。这使开发者可以在服务重启后快速恢复 RAG 知识库。

**Why this priority**: 持久化是生产环境必备能力，但依赖 US1 的基础插入和搜索功能，排为 P2。

**Independent Test**: 插入 100 条向量 → 保存到临时文件 → 从文件加载新实例 → 搜索相同 query → 验证结果与保存前一致。

**Acceptance Scenarios**:

1. **Given** 一个包含 3 个 collection 的 `TurbovecVectorStore`, **When** 调用 `save("/path/to/store")` 保存后重新 `load("/path/to/store")` 创建新实例, **Then** 所有 3 个 collection 可用，搜索返回与保存前相同的结果
2. **Given** 一个保存了 200 条向量的持久化文件, **When** 加载后继续插入 50 条新向量并再次保存, **Then** 重新加载后总共包含 250 条向量
3. **Given** 一个空 store（没有任何 collection）, **When** 调用 `save()` 和 `load()`, **Then** 加载后的 store 仍为空，不报错

---

### User Story 3 - KnowledgeBase 集成 (Priority: P3)

开发者在现有的 `KnowledgeBase` 中使用 `TurbovecVectorStore` 替代抽象的 `VectorStore` trait，完成端到端的 RAG 流程：解析文档 → 嵌入向量 → 存入 turbovec → 语义搜索 → 返回结果。`KnowledgeBase` 的行为（懒创建 collection、metadata_filter 强制覆盖等）保持不变。

**Why this priority**: KnowledgeBase 集成是最终用户价值闭环，但依赖 US1（存储）和 Feature 011 的 `VectorStore` trait / KnowledgeBase，排为 P3。

**Independent Test**: 使用 `DashScopeEmbeddingModel` + `TurbovecVectorStore` 创建 `KnowledgeBase`，插入一段 Markdown 文档的 Chunk，搜索后验证返回结果包含正确的 chunk 内容。

**Acceptance Scenarios**:

1. **Given** 一个绑定 `TurbovecVectorStore` 的 `KnowledgeBase` 实例, **When** 首次调用 `search(["查询文本"])`, **Then** 自动创建 backing collection 并完成搜索
2. **Given** 插入 3 个文档的 chunk 到 KnowledgeBase, **When** 调用 `search(queries, top_k=2)`, **Then** 返回最多 2 个去重后的结果，按相似度降序
3. **Given** `metadata_filter = {"tenant_id": "t1"}`, **When** 通过 KnowledgeBase 插入 chunk, **Then** chunk 的 metadata 被 metadata_filter 强制覆盖

---

### Edge Cases

- 空向量存储搜索：`search()` 在空 collection 上返回空结果，不报错
- 维度不匹配：向已有 1536 维的 collection 插入不同维度的向量，返回明确错误
- 超大 k 值：`search()` 的 `top_k` 超过 collection 实际向量数时，返回所有可用结果
- bit_width 边界：bit_width 仅支持 2/3/4，传入其他值在构造时返回错误
- 重复 document_id 插入：与 Feature 011 `VectorStore` trait 一致，由实现定义行为（追加 chunk）
- 零向量（norm ≤ 1e-10）：turbovec 将其存储为 score=0，搜索时排在所有有效向量之后
- collection 名称冲突：同一 collection 只能创建一次，重复创建返回错误
- 并发搜索安全性：turbovec 的 `search` 接受 `&self`，支持多线程并发搜索
- 向量维度限制：turbovec 的 `dim` 必须为正且为 8 的倍数（最大 16384）

## Requirements *(mandatory)*

### Functional Requirements

**TurbovecVectorStore 核心**:

- **FR-001**: System MUST 实现 `TurbovecVectorStore` 结构体，实现 Feature 011 定义的 `VectorStore` trait 的所有方法
- **FR-002**: `TurbovecVectorStore` MUST 内部管理多个 collection，每个 collection 对应一个 `turbovec::IdMapIndex` 实例
- **FR-003**: `TurbovecVectorStore` MUST 支持可配置的 `bit_width`（2/3/4），控制向量压缩率
- **FR-004**: `TurbovecVectorStore::new(bit_width)` MUST 创建空的 store，后续通过 `create_collection` 或懒初始化添加 collection
- **FR-005**: `has_collection(name)` MUST 检查指定名称的 collection 是否已存在
- **FR-006**: `create_collection(name, dimensions)` MUST 创建新的 `IdMapIndex` 实例并绑定到指定名称，若已存在则返回 `false`
- **FR-007**: `insert(collection, records)` MUST 将 `VectorRecord` 列表的向量和元数据写入指定的 `IdMapIndex`，其中：
  - 向量部分通过 `add_with_ids()` 写入索引
  - chunk 元数据和 `document_id` 存入内部映射表
  - 若 collection 不存在则自动创建
- **FR-008**: `search(collection, query_vector, top_k, metadata_filter)` MUST：
  - 将 query_vector 传入 turbovec 的 `search()` 方法
  - 若提供 `metadata_filter`，通过 `allowlist`（id 掩码）过滤
  - 返回 `Vec<VectorSearchResult>`，包含 `score`、`document_id` 和 `Chunk`
- **FR-009**: `delete(collection, document_id)` MUST 删除指定 document 在 collection 中的所有 chunk 记录
- **FR-010**: `list_documents(collection, metadata_filter)` MUST 返回所有匹配 `metadata_filter` 的 `DocumentSummary` 列表
- **FR-011**: `TurbovecVectorStore` MUST 使用 `u64` 作为内部 ID，chunk 从 document_id 通过哈希映射到 u64 空间

**持久化**:

- **FR-012**: `TurbovecVectorStore` MUST 提供 `save(path)` 方法，将整个 store（所有 collection 的索引 + 元数据）持久化到磁盘
- **FR-013**: `TurbovecVectorStore` MUST 提供 `load(path)` 静态方法，从持久化文件恢复完整的 store 状态
- **FR-014**: 持久化格式 MUST 包含：每个 collection 的 turbovec 索引文件（`.tvim`）+ 元数据映射（chunk 元数据、document_id 映射等）
- **FR-015**: `save()` MUST 使用原子写入策略（先写临时文件，fsync 后 rename），防止文件损坏
- **FR-016**: `load()` MUST 验证文件格式版本和完整性，对于损坏文件返回明确错误而非静默加载错误数据

**turbovec 依赖集成**:

- **FR-017**: System MUST 将 `turbovec` 作为外部依赖引入，版本锁定为 0.9.x，确保跨平台编码字节一致性
- **FR-018**: `TurbovecVectorStore` MUST 配置 turbovec 的 `IdMapIndex` 使用 vector 的 L2 归一化，使搜索结果分数等价于余弦相似度
- **FR-019**: System MUST 封装 turbovec 的 `CalibrationState`，在 KnowledgeBase 日志中暴露索引的标定状态（WarmingUp/Fitted/Identity）
- **FR-020**: System MUST 支持通过 `metadata_filter` 构建 turbovec 的 allowlist，在 SIMD 内核层面过滤，不损失召回率

**错误处理**:

- **FR-021**: 所有 turbovec 错误（`ConstructError`、`AddError`、`SearchError`）MUST 映射为 `VectorStore` trait 的对应错误类型
- **FR-022**: 维度不匹配、bit_width 非法等参数错误 MUST 在构造或首次操作时返回明确错误，而非运行时崩溃

### Key Entities

- **TurbovecVectorStore**: 基于 turbovec `IdMapIndex` 的 `VectorStore` trait 具体实现——管理多个命名 collection，每个 collection 是一个独立的压缩向量索引 + chunk 元数据映射
- **Collection**: store 内部的命名空间——包含一个 `IdMapIndex` 实例（向量存储与搜索）+ `HashMap<u64, ChunkMetadata>`（chunk 元数据）+ `HashMap<String, Vec<u64>>`（document_id → chunk internal IDs 的反向索引）
- **ChunkMetadata**: 存储在每个 collection 中的 chunk 相关信息——`document_id`、`chunk_index`、`total_chunks`、`source`、`metadata`（键值对）
- **StoreManifest**: 持久化的 store 元数据描述文件——记录所有 collection 名称、各自的 `dim` 和 `bit_width`、文件格式版本号
- **InternalId**: turbovec `IdMapIndex` 使用的 `u64` 稳定 ID——从 `(document_id, chunk_index)` 通过确定性哈希生成，支持按 document 的批量删除

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 开发者无需安装和配置任何外部数据库，通过一次构造调用即获得可用的向量存储
- **SC-002**: 插入 1000 条 1536 维向量的端到端耗时（含向量归一化与压缩）不超过 1 秒
- **SC-003**: 1000 条 1536 维向量的 top-10 搜索在单线程下耗时不超过 5ms
- **SC-004**: 保存 1000 条向量的 store 到磁盘 + 重新加载到可用状态总耗时不超过 0.5 秒
- **SC-005**: `TurbovecVectorStore` 的行为与 Feature 011 `VectorStore` trait 的所有 mock 测试中的约定兼容
- **SC-006**: 使用 `TurbovecVectorStore` 的 `KnowledgeBase` 端到端搜索流程与使用 mock `VectorStore` 的测试产生等价结果
- **SC-007**: store 持久化文件在进程崩溃后能被正确恢复，不丢失已持久化的数据（原子写入保证）

## Assumptions

- turbovec crate 通过 Git 依赖或 crates.io 引入（版本 0.9.0）
- `TurbovecVectorStore` 放在现有的 `agent_scope_rag` crate 中，作为 `VectorStore` trait 的第二个实现
- 每个 collection 使用独立的 `IdMapIndex` 实例——不支持跨 collection 联合查询
- `metadata_filter` 遵循与 Feature 011 相同的语义：键值对精确匹配，多个条件为 AND 关系
- L2 归一化在插入向量时执行（turbovec 内部已执行归一化，但调用方需保证输入向量已归一化以保证余弦相似度等价性）
- `TurbovecVectorStore` 是单机本地存储，不支持分布式或网络访问——如需分布式向量搜索，应使用独立的 Qdrant/Milvus 实现
- `document_id` 到 `u64` 的映射使用确定性哈希，可能在极低概率下产生碰撞（64 位空间内可忽略）
- 持久化目录必须已存在，`save()` 不负责创建目录结构
