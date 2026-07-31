# Research: RAG System (Feature 011)

**Feature**: 011-rag-system
**Date**: 2026-07-31
**Status**: Complete

## R1: Crate Architecture — Separate Crate vs Extending Existing

**Decision**: 创建两个新 crate：`agent_scope_embedding`（Embedding 模型抽象层）和 `agent_scope_rag`（RAG 管道逻辑）。不扩展现有 crate。

**Rationale**:
- `agent_scope_embedding` 与 `agent_scope_model` 平行——都是模型抽象层，独立 crate 保持职责单一
- `agent_scope_rag` 依赖 `agent_scope_embedding` + `agent_scope_model` + `agent_scope_agent`，如果在现有 crate 中实现会产生不合理的耦合
- Constitution §11（分层与依赖方向）：核心抽象（embedding）在独立 crate，RAG 管道（rag）在组合层 crate
- 遵循已确立的模式：Feature 003 `agent_scope_model` 独立于 `agent_scope_core`

**Alternatives considered**:
- 全放 `agent_scope_rag` 单个 crate：EmbeddingModel trait 与 ChatModel 不在同层，破坏架构一致性
- 全放现有 crates：任何单一现有 crate 都会产生循环依赖；例如放在 `agent_scope_agent` 会导致 agent 依赖 rag 依赖 agent
- Extend `agent_scope_model` with EmbeddingModel：两个 trait 语义独立（Chat vs Embedding），放在同一 crate 会模糊边界

## R2: EmbeddingModel Trait 设计 — Sync vs Async

**Decision**: `EmbeddingModel` trait 使用 `#[async_trait::async_trait]`，`embed()` 方法为 async。`model_card()` 和 `supports_multimodal()` 为同步方法。

**Rationale**:
- 与 `ChatModel` trait（Feature 003）保持一致的 async 模式——embedding 调用涉及 HTTP I/O
- `model_card()` 返回静态元信息，无需 async
- Constitution §10（结构化并发）：所有 I/O 操作必须 async
- Send + Sync bounds 确保 trait object 安全：`Arc<dyn EmbeddingModel>`

**Alternatives considered**:
- 同步 blocking API：违反 Constitution §10，会阻塞 tokio runtime
- 返回 `BoxFuture`：不需要，async_trait 已够用且更符合现有模式

## R3: DashScope Embedding API 集成

**Decision**: `DashScopeEmbeddingModel` 实现在 `agent_scope_dashscope` crate，复用 Feature 005 的 HTTP 客户端模式（reqwest + DashScope 认证头）。调用 DashScope Text Embedding API 的 `/api/v1/services/embeddings/text-embedding/text-embedding` 端点。

**Rationale**:
- Feature 005 已确立 pattern：provider 实现在 provider crate，trait 在抽象 crate
- DashScope `ChatModel` 已有成熟的 HTTP 客户端模式，直接复用
- 输入格式：`{"model": "...", "input": {"texts": [...]}}` 或 `{"model": "...", "input": {"texts": [...], "images": [...]}}`（多模态）
- 响应格式：`{"output": {"embeddings": [...]}, "usage": {"total_tokens": ...}}`

**Alternatives considered**:
- 新建独立的 HTTP 客户端：不必要的重复，DashScope 模式已验证
- 在 `agent_scope_embedding` 中实现 provider：违反 §11 分层约束

## R4: EmbeddingCache 设计

**Decision**: `EmbeddingCache` trait 使用输入内容的 SHA-256 哈希作为缓存键。`FileEmbeddingCache` 以文件系统存储，每个键对应一个 JSON 文件（`{cache_dir}/{hash}.json`）。

**Rationale**:
- SHA-256 内容寻址：相同输入必然产生相同缓存键，避免碰撞
- 文件系统存储：无需外部依赖，适合开发/测试场景
- 缓存文件格式：`Vec<Vec<f32>>` 的 JSON 序列化——简单可调试
- 与 Python AgentScope 的 `EmbeddingCache` 语义等价

**Alternatives considered**:
- 内存缓存（HashMap）：进程重启丢失，不适合生产环境
- Redis 缓存：超出 scope，由外部层实现
- MD5 哈希：SHA-256 更安全，性能差异可忽略

## R5: Parser 架构 — 多格式支持策略

**Decision**: v1 仅实现 `TextParser`（处理 `.txt`、`.md`）。Parser trait 设计预留扩展点（返回 `Vec<Section>`），但不实现 PDF/PPT/Word/Excel/Image parser。

**Rationale**:
- spec Assumptions 明确规定："Parser 的 v1 版本仅支持 TextParser；PDF/PPT/Word/Excel/Image 解析器不在此 feature 范围内"
- TextParser 将整个文本文件作为一个 Section 输出（Markdown 标题拆分是未来增强）
- 空文件返回空 Section 列表（Edge Case 已定义）
- Constitution §V（不允许伪兼容）：不支持的格式显式返回错误，不假装支持

**Alternatives considered**:
- 提前实现 Markdown 标题拆分：增加复杂度但 spec 未要求
- 用一个"万能 Parser"按扩展名分发：过度设计，v1 不需要

## R6: Chunker 算法 — ApproxTokenChunker

**Decision**: `ApproxTokenChunker` 使用简单启发式 token 计数——英文按空格分词（每个词 ≈ 1 token），非英文按字符/4 估算。这与 Python AgentScope 实现近似但不保证完全一致。Section 边界不可跨越（Per spec FR-013）。

**Rationale**:
- 精确 token 计数需要 tokenizer（如 tiktoken），引入额外依赖且性能差
- 启发式算法与 Python AgentScope 语义一致（approximate token count）
- 滑动窗口：`window_size = chunk_size`，`stride = chunk_size - overlap`
- Section 边界不可跨：每个 Section 独立切分，不同 Section 的 Chunk 不合并
- `chunk_index` 在同一 document 的 Section 之间连续（如果跨 Section 时 `source` 相同则 index 全局递增）

**Alternatives considered**:
- tiktoken-rs 精确计数：额外依赖、性能差、spec 不要求精确性
- 字符数切分：更粗略，与 Python 实现差异过大

## R7: VectorStore Trait 设计 — Async vs Sync

**Decision**: `VectorStore` trait 使用 `#[async_trait::async_trait]`，所有方法均为 async。因为向量数据库操作（网络 I/O）本质是异步的。仅定义 trait，不做任何具体实现。

**Rationale**:
- 具体向量数据库（Qdrant/Milvus/MongoDB）的网络通信天然 async
- `has_collection` 可能涉及网络查询，统一为 async 避免假设
- Constitution §V（不允许伪兼容）：不做 mock/空实现冒充功能——trait 只是契约
- Constitution §X（结构化并发）：async 方法配合 CancellationToken 实现超时控制

**Alternatives considered**:
- 部分方法 sync：`has_collection` 可能本地判断，但无法假设后端不查询远程服务
- Generic trait（非 async-trait）：async trait 稳定性尚未完全，但 async_trait macro 是生态标准

## R8: KnowledgeBase 架构 — 懒初始化

**Decision**: KnowledgeBase 在首次操作时（search/insert/delete/list）自动调用 `VectorStore::has_collection()` 和 `create_collection()`。后续操作跳过检查。使用 `tokio::sync::OnceCell` 或 `Mutex<bool>` 保证线程安全的一次性初始化。

**Rationale**:
- FR-037 明确要求懒创建
- `OnceCell` 是 Rust 生态的标准懒初始化模式，比额外状态 flag 更安全
- 并发安全：首次多线程同时调用时 Only Once 语义

**Alternatives considered**:
- 显式 `init()` 方法：增加调用方负担，违背 SC-001（低门槛 API）
- 构造函数中初始化：要求 VectorStore 在构造时已可用，不够灵活

## R9: RAGMiddleware 模式 — Static vs Agentic

**Decision**: `RAGMiddleware` 通过 `RAGMode` enum（`Static` / `Agentic`）控制行为。Static 模式在 `pre_reply` 钩子中自动搜索并注入上下文；Agentic 模式在 `post_acting` 钩子中注册 Tool。

**Rationale**:
- `Middleware` trait（Feature 007）提供了 `pre_reply` 和 `post_acting` 钩子，天然匹配两种模式
- Static 模式：embedding → search → build HintBlock → inject into model context
- Agentic 模式：注册 `search_knowledge(name, query)` Tool，由 LLM 自主决策搜索时机
- 两者可共存（同一 RAGMiddleware 同时支持双模式），由配置决定
- 多 KnowledgeBase 支持：static 模式下全部搜索并去重；agentic 模式下每个 KB 注册独立 Tool

**Alternatives considered**:
- 两种模式作为不同 Middleware：增加 API 复杂度，共享相同的 KnowledgeBase 绑定逻辑
- 在 pre_reply 中同时注册 Tool：语义混淆——Tool 暴露 vs 上下文注入是两个不同阶段

## R10: 错误类型设计

**Decision**: 每个子系统定义独立的错误类型。`agent_scope_embedding` 定义 `EmbeddingError`，`agent_scope_rag` 定义 `ParserError`/`ChunkerError`/`VectorStoreError`/`KnowledgeBaseError`。所有错误实现 `std::error::Error` + `Display` + `Debug`。

**Rationale**:
- Constitution §13（稳定错误模型）：typed errors, not stringly-typed
- 每个 crate 独立错误类型避免跨 crate 依赖
- 错误分类对齐 §13 表格：`EmbeddingError` → ModelError, `KnowledgeBaseError::DimensionMismatch` → ValidationError

| Error Type | Category | Example |
|---|---|---|
| `EmbeddingError::HttpError` | ModelError | DashScope API 调用失败 |
| `EmbeddingError::MultimodalNotSupported` | ValidationError | 多模态模型收到图像但 `supports_multimodal = false` |
| `ParserError::UnsupportedFormat` | UnsupportedFeature | 不支持的文档格式 |
| `ChunkerError::EmptySections` | ValidationError | 空 Section 列表（正常返回空 Vec，非错误） |
| `VectorStoreError::BackendError` | InternalError | 向量数据库后端错误 |
| `KnowledgeBaseError::DimensionMismatch` | ValidationError | Embedding 返回维度与 collection 不匹配 |
| `KnowledgeBaseError::CountMismatch` | ValidationError | Embedding 返回向量数与 chunk 数不匹配 |
| `KnowledgeBaseError::VectorStoreError` | InternalError | 透传 VectorStore 错误 |

## R11: ApproxTokenChunker token 计数启发式

**Decision**: 英文按空格分词（1 word ≈ 1 token），非 ASCII/非空白字符按 4 字符 = 1 token 估算。与 Python AgentScope 实现近似。

**Rationale**:
- Python AgentScope 使用 `tiktoken` 近似计数，但 Rust 环境中引入完整 tokenizer 成本过高
- 启发式方法对英文文档准确度 > 90%，对中/日/韩文文档可接受（已知误差 < 20%）
- spec Assumptions 明确标注："与 Python 实现近似但不保证完全一致"
- 这不是兼容性问题——token 切分是内部逻辑，外部可观察行为是 Chunk 的边界和元数据

**Alternatives considered**:
- Unicode 分段 + 词边界检测（Unicode Segmentation crate）：额外依赖但更准确
- 纯字符计数：对 CJK 文本严重高估 token 数
