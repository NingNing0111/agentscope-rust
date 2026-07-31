# Feature Specification: RAG System（检索增强生成）

**Feature Branch**: `011-rag-system`

**Created**: 2026-07-31

**Status**: Draft

**Input**: User description: "为 AgentScope Rust 实现 RAG（检索增强生成）能力，包含 Embedding 模型层、文档解析管道、文件切分、向量存储抽象、KnowledgeBase 运行时代理、RAGMiddleware Agent 集成。VectorStore 只定义 trait 抽象，不做具体向量数据库实现。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 文本嵌入 (Priority: P1)

开发者使用 Embedding 模型将文本或多媒体内容转换为稠密向量，用于后续语义搜索。调用方只需调用 `embed()` 方法，传入文本/图像输入，即可获得向量列表。

**Why this priority**: Embedding 是整个 RAG 管道的基础——检索需要将查询转为向量、索引需要将文档转为向量。没有 Embedding 层就没有后续任何 RAG 能力。

**Independent Test**: 使用 DashScope embedding model 对一段文本调用 `embed()`，验证返回的向量维度与模型声称的 `dimensions` 一致，且向量值非空。

**Acceptance Scenarios**:

1. **Given** 已配置 DashScope API Key, **When** 调用 `embed(["hello world"])`, **Then** 返回 `EmbeddingResponse` 包含 1 个向量，长度等于模型 `dimensions`，且 `usage.total_tokens > 0`
2. **Given** embedding 结果为 `{"hello": vec_a, "world": vec_b}` 已缓存, **When** 再次请求嵌入 `["hello", "world"]`, **Then** 直接从缓存返回对应向量，不产生 API 调用
3. **Given** 模型 `supports_multimodal = false`, **When** 传入包含图像的 `EmbeddingInput`, **Then** 返回错误

---

### User Story 2 - 文档解析与切分 (Priority: P1)

开发者将原始文件（文本、Markdown 等）传入解析器，获得按自然边界划分的 `Section` 列表；再将 Section 传入切分器，获得可索引的 `Chunk` 列表。整个过程将非结构化字节流转为结构化的可索引片段。

**Why this priority**: 文档处理是 RAG 管道的入口——没有解析和切分，文档无法入库。与 US1 并列为 P1 的基础能力。

**Independent Test**: 上传一段 Markdown 文本，经过 TextParser → ApproxTokenChunker 处理后，验证每个 Chunk 携带正确的 source/chunk_index/total_chunks 元数据。

**Acceptance Scenarios**:

1. **Given** 一个 500 词的纯文本文件, **When** TextParser 解析后 ApproxTokenChunker 以 chunk_size=100、overlap=20 切分, **Then** 产生 5+ 个 Chunk，每个 Chunk 的 `source` 相同，`chunk_index` 从 0 递增，`total_chunks` 一致
2. **Given** 两个 Section 来自不同源文件（source 不同）, **When** Chunker 处理, **Then** 不跨 Section 边界合并内容
3. **Given** 一个空文件（0 字节）, **When** TextParser 解析, **Then** 返回空 Section 列表

---

### User Story 3 - 向量存储抽象 (Priority: P1)

定义 `VectorStore` trait，声明 collection 管理、向量搜索、插入、删除、文档列表等操作的标准接口。该 trait 是具体向量数据库实现的契约——具体的 Qdrant/Milvus/MongoDB 等实现放在独立 crate 中。

**Why this priority**: VectorStore 是 RAG 系统与外部向量数据库的唯一接口契约。先定义 trait 确保后续所有具体实现有一致的行为约定，且上层 KnowledgeBase 可以针对 trait 编写和测试。

**Independent Test**: 通过 mock 实现 `VectorStore` trait，验证 trait 中所有方法签名可调用、数据类型匹配。

**Acceptance Scenarios**:

1. **Given** 定义的 `VectorStore` trait, **When** mock 实现 `insert()` 后调用 `search()`, **Then** `search()` 返回与插入时相同 `document_id` 和 `Chunk` 内容的 `VectorSearchResult`
2. **Given** `create_collection("kb", 1024)` 已调用, **When** 再次调用 `has_collection("kb")`, **Then** 返回 `true`
3. **Given** 3 个 document 已插入, **When** 调用 `list_documents()`, **Then** 返回 3 个 `DocumentSummary`，每个包含正确的 `document_id`/`source`/`chunk_count`

---

### User Story 4 - 知识库运行时代理 (Priority: P2)

开发者创建一个 `KnowledgeBase` 实例，绑定 embedding 模型、向量存储和 collection 名称。之后通过 `search()` 检索相关文档片段，通过 `insert_document()`/`delete_document()`/`list_documents()` 管理知识库内容。Collection 在首次操作时自动创建，无需显式初始化。

**Why this priority**: KnowledgeBase 是 RAG 模块对开发者的主要 API——它将 Embedding + VectorStore 管道的复杂性封装为四个简洁操作。但依赖 US1-US3 作为前置，所以排为 P2。

**Independent Test**: 使用 mock EmbeddingModel + mock VectorStore 创建 KnowledgeBase，验证 search/insert/delete/list 四个操作的正确性和 metadata_filter 隔离行为。

**Acceptance Scenarios**:

1. **Given** 一个 KnowledgeBase 实例, **When** 首次调用 `search(["query"])`, **Then** 自动创建 collection 并完成搜索（懒初始化）
2. **Given** `metadata_filter = {"tenant_id": "t1"}`, **When** `search()` 返回结果, **Then** 所有结果不包含 `metadata_filter` 范围外的记录
3. **Given** `metadata_filter = {"tenant_id": "t1"}`, **When** `insert_document(chunks)`, **Then** 每个 chunk 的 metadata 强制包含 `{"tenant_id": "t1"}`（即使调用方传入空 metadata）
4. **Given** 文档 `"doc-1"` 已插入 5 个 Chunk, **When** 调用 `delete_document("doc-1")`, **Then** 后续搜索不再返回该文档的任何片段
5. **Given** 3 个查询词分别匹配不同文档, **When** 调用 `search(queries, top_k=2)`, **Then** 按 (document_id, chunk_index) 去重后最多返回 2 个结果，按相似度降序排列

---

### User Story 5 - Agent 知识检索集成 (Priority: P3)

开发者在创建 Agent 时配置 `RAGMiddleware`，绑定一个或多个 `KnowledgeBase`。Agent 对话时可选择两种模式：
- **static**：每轮自动将用户问题嵌入并搜索知识库，命中片段作为上下文注入模型推理
- **agentic**：将知识库暴露为 Tool（`search_knowledge`），由模型自主决定搜索时机和内容

**Why this priority**: RAGMiddleware 是 RAG 能力的最终用户触点——它让 RAG 管道对 Agent 透明可用。但依赖 US4（KnowledgeBase）和 Feature 007（Agent middleware 管道），排为 P3。

**Independent Test**: 创建带 RAGMiddleware 的 Agent，分别测试 static 和 agentic 模式下的知识检索行为。

**Acceptance Scenarios**:

1. **Given** RAGMiddleware 配置为 static 模式, **When** Agent 收到用户消息 "公司远程办公政策是什么？", **Then** 模型上下文中包含知识库中匹配片段的引用
2. **Given** RAGMiddleware 配置为 agentic 模式, **When** Agent 判断需要查询知识库, **Then** Agent 调用 `search_knowledge` tool 并基于返回结果生成回答
3. **Given** static 模式下知识库无匹配结果, **When** Agent 生成回复, **Then** 不注入空上下文，Agent 按普通对话回复

---

### Edge Cases

- 空查询列表（`queries=[]`）：`KnowledgeBase.search()` 返回空结果，不报错
- 文本 embedding 模型收到 `DataBlock`（图像）查询：静默丢弃，仅处理能消费的输入类型
- embedding 返回向量数与输入数不匹配：`insert_document()` 抛出明确错误
- 重复插入同一 document_id 的 chunk：由 VectorStore 后端定义行为（覆盖或追加），trait 不规定语义
- collection 已存在但维度不匹配：由 VectorStore 后端报错，KnowledgeBase 透传错误
- 超大文件解析（>100MB）：Parser 不负责文件大小限制，由调用方预先控制
- Chunker 收到空 Section 列表：返回空 Chunk 列表

## Requirements *(mandatory)*

### Functional Requirements

**Embedding 模型层**：

- **FR-001**: System MUST 定义 `EmbeddingModel` trait，暴露 `embed(inputs: Vec<EmbeddingInput>) -> EmbeddingResponse` 方法，语义与 Python AgentScope `EmbeddingModelBase.__call__` 等价
- **FR-002**: `EmbeddingInput` MUST 支持文本（`TextBlock`）和多媒体（`DataBlock`）两种输入类型
- **FR-003**: `EmbeddingResponse` MUST 包含 `embeddings: Vec<Vec<f32>>`（每个输入对应一个等长向量）和 `usage: EmbeddingUsage`（token 统计）
- **FR-004**: `EmbeddingUsage` MUST 包含 `total_tokens: u32`（本次 embedding 消耗的总 token 数）
- **FR-005**: `EmbeddingModelCard` MUST 包含 `name`（模型名）、`dimensions`（输出向量维度）、`supports_multimodal`（是否支持图像等多模态输入）
- **FR-006**: System MUST 实现 DashScope embedding provider（`DashScopeEmbeddingModel`），复用 Feature 005 的 DashScope HTTP 客户端模式
- **FR-007**: `DashScopeEmbeddingModel` MUST 能从 `EmbeddingModelCard` 获取 model name 和 dimensions
- **FR-008**: `EmbeddingModel` trait MUST 提供 `model_card() -> EmbeddingModelCard` 方法，返回静态模型元信息
- **FR-009**: System MUST 定义 `EmbeddingCache` trait，暴露 `lookup(key) -> Option<Vec<Vec<f32>>>` 和 `store(key, embeddings)` 方法
- **FR-010**: System MUST 实现 `FileEmbeddingCache`（基于文件系统的 embedding 缓存），以输入内容的哈希作为缓存键

**文档解析与切分**：

- **FR-011**: System MUST 定义 `Section` 数据结构，包含 `content`（TextBlock 或 DataBlock）、`source`（源文件名）、`metadata`（格式特定元数据，如 page/slide/sheet 编号）
- **FR-012**: System MUST 定义 `Parser` trait，暴露 `parse(file: Vec<u8>, filename: &str) -> Vec<Section>` 方法
- **FR-013**: `Parser` trait 约定：每个 `Section` 代表源文件的"一个自然边界"（如 Markdown 标题段、TXT 整体），Chunker 不能跨不同 Section 合并内容
- **FR-014**: System MUST 实现 `TextParser`，能处理 `.txt`、`.md` 等纯文本格式，将整个文件作为单个 Section 输出
- **FR-015**: System MUST 定义 `Chunk` 数据结构，包含 `content`、`source`、`chunk_index`、`total_chunks`、`metadata`
- **FR-016**: System MUST 定义 `Chunker` trait，暴露 `chunk(sections: Vec<Section>) -> Vec<Chunk>` 方法
- **FR-017**: System MUST 实现 `ApproxTokenChunker`，支持通过 `chunk_size`（目标 token 数）和 `overlap`（相邻 Chunk 重叠 token 数）进行滑动窗口切分
- **FR-018**: `ApproxTokenChunker` MUST 保证所有 Chunk 的 `chunk_index` 在同一 document 内从 0 连续递增，`total_chunks` 一致

**向量存储抽象**：

- **FR-019**: System MUST 定义 `VectorStore` async trait，声明与 Python `VectorStoreBase` 等价的操作契约。该 trait 是纯抽象，不做任何具体向量数据库实现
- **FR-020**: `VectorStore` trait MUST 包含 `has_collection(name: &str) -> bool`
- **FR-021**: `VectorStore` trait MUST 包含 `create_collection(name: &str, dimensions: u32)`
- **FR-022**: `VectorStore` trait MUST 包含 `search(collection: &str, query_vector: Vec<f32>, top_k: usize, metadata_filter: Option<HashMap<String, String>>) -> Vec<VectorSearchResult>`
- **FR-023**: `VectorStore` trait MUST 包含 `insert(collection: &str, records: Vec<VectorRecord>)`
- **FR-024**: `VectorStore` trait MUST 包含 `delete(collection: &str, document_id: &str)`
- **FR-025**: `VectorStore` trait MUST 包含 `list_documents(collection: &str, metadata_filter: Option<HashMap<String, String>>) -> Vec<DocumentSummary>`
- **FR-026**: System MUST 定义 `VectorRecord` 数据结构：`vector`（嵌入向量）、`document_id`、`chunk`
- **FR-027**: System MUST 定义 `VectorSearchResult` 数据结构：`score`（相似度分数）、`document_id`、`chunk`
- **FR-028**: System MUST 定义 `DocumentSummary` 数据结构：`document_id`、`source`、`chunk_count`、`metadata`

**KnowledgeBase 运行时代理**：

- **FR-029**: `KnowledgeBase` MUST 绑定 `name`、`description`（面向 Agent 的描述）、`embedding_model: Arc<dyn EmbeddingModel>`、`vector_store: Arc<dyn VectorStore>`、`collection: String`、`metadata_filter: Option<HashMap<String, String>>`
- **FR-030**: `KnowledgeBase::search(queries, top_k, score_threshold)` MUST 执行以下流程：批量嵌入查询词 → 并发搜索各向量 → 按 `(document_id, chunk_index)` 去重（保留最高分） → 按 score 降序排序 → `top_k` 截断
- **FR-031**: `KnowledgeBase::search()` MUST 在 `EmbeddingModel` 不支持多模态时静默丢弃 `DataBlock` 类型查询
- **FR-032**: `KnowledgeBase::insert_document(chunks, document_id, document_metadata)` MUST 在未指定 `document_id` 时自动生成唯一 ID
- **FR-033**: `KnowledgeBase::insert_document()` MUST 以 `metadata_filter` 最高优先级合并 metadata——`metadata_filter` 的键值覆盖 chunk 自身和 `document_metadata` 的值（安全边界）
- **FR-034**: `KnowledgeBase::insert_document()` MUST 在 embedding 返回向量数不等于 chunk 数时返回错误
- **FR-035**: `KnowledgeBase::delete_document(document_id)` MUST 删除指定 document 的所有 chunk 记录
- **FR-036**: `KnowledgeBase::list_documents()` MUST 返回所有文档的 `DocumentSummary`，受 `metadata_filter` 约束
- **FR-037**: `KnowledgeBase` MUST 懒创建 backing collection——首次操作时自动调用 `VectorStore::has_collection()` 和 `create_collection()`，后续操作跳过检查

**Agent 集成**：

- **FR-038**: System MUST 实现 `RAGMiddleware`，实现现有 `Middleware` trait（Feature 007），可插入 Agent 的 middleware 管道
- **FR-039**: `RAGMiddleware` MUST 支持 `static` 模式：在 `pre_reply` 钩子中，自动嵌入用户最新消息、搜索绑定的所有 KnowledgeBase、将命中片段作为 `HintBlock` 注入模型上下文
- **FR-040**: `RAGMiddleware` MUST 支持 `agentic` 模式：在 `post_acting` 钩子中注册 `search_knowledge(name: String, query: String)` Tool，由模型自主决定是否调用
- **FR-041**: `RAGMiddleware` MUST 支持绑定多个 KnowledgeBase（`Vec<Arc<KnowledgeBase>>`），static 模式下全部搜索，agentic 模式下每个 KnowledgeBase 单独注册 Tool

### Key Entities

- **EmbeddingInput**: 嵌入模型的输入——文本（TextBlock）或多媒体（DataBlock）内容的抽象表示
- **EmbeddingResponse**: 嵌入结果——每个输入的对应向量 + token 用量统计
- **Section**: 解析阶段的中间产物——源文件一个逻辑边界（页/幻灯片/标题段）的内容 + 元数据，不持久化
- **Chunk**: 切分后的最终可索引单元——内容 + 结构元数据（source/chunk_index/total_chunks），支持检索时上下文扩展
- **VectorRecord**: 向量存储的写入单元——embedding 向量 + document_id + Chunk 三者的绑定
- **VectorSearchResult**: 向量搜索的命中结果——Chunk + 相似度分数 + document_id
- **DocumentSummary**: 单篇源文档在向量存储中的摘要——document_id + source + chunk_count + metadata
- **KnowledgeBase**: 运行时知识库句柄——绑定 embedding 模型 + vector store + collection + metadata_filter，暴露 search/insert/delete/list 四个操作
- **RAGMiddleware**: Agent 中间件——绑定 KnowledgeBase 列表，按 static 或 agentic 模式将知识检索集成到 Agent 推理循环

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 开发者仅需创建 `KnowledgeBase` 实例并调用 `search()` 即可完成端到端语义搜索，无需了解 Embedding 模型或 VectorStore 内部实现
- **SC-002**: `EmbeddingModel` trait 可在不依赖真实 HTTP 请求的情况下，通过 mock 实现完成所有 KnowledgeBase 单元测试
- **SC-003**: `VectorStore` trait 可在不依赖具体数据库实现的情况下，通过 mock 验证 KnowledgeBase 的 search/insert/delete/list 业务逻辑正确性
- **SC-004**: 文本解析+切分管道处理 1MB 纯文本文件的时间不超过 1 秒
- **SC-005**: 100 个查询词的 batch embedding 通过 FileEmbeddingCache 在缓存全命中时零 API 调用完成
- **SC-006**: `metadata_filter` 强制覆盖机制使调用方无法通过 insert_document 将数据写入过滤器范围外的 collection
- **SC-007**: static 模式的 RAGMiddleware 使 Agent 在不显式调用 tool 的情况下自动获得知识库上下文增强

## Assumptions

- Embedding 模型的实际 HTTP 调用依赖 Feature 005 的 DashScope provider 架构（`agent_scope_dashscope` crate）
- `EmbeddingModel` trait 设计为与 `ChatModel` trait（Feature 003）平行的抽象层，放在独立的 `agent_scope_embedding` crate
- `Parser`/`Chunker`/`VectorStore`/`KnowledgeBase` trait 和实现放在 `agent_scope_rag` crate
- Parser 的 v1 版本仅支持 TextParser；PDF/PPT/Word/Excel/Image 解析器不在此 feature 范围内
- VectorStore 不实现任何具体向量数据库——Qdrant/Milvus/MongoDB 等实现各自放在独立 crate
- RAGMiddleware 依赖 Feature 007 的 `Middleware` trait 和 `Tool` trait（Feature 006）
- RAG 服务端层（`app/rag/` 的 KnowledgeBaseManager、index_worker、blob_store）不在此 feature 范围内
- EmbeddingCache 的缓存键基于输入内容的 SHA-256 哈希
- ApproxTokenChunker 的 token 计数采用简单启发式（英文按空格分词，非英文按字符/4 估算），与 Python 实现近似但不保证完全一致
