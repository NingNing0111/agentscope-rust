# Implementation Plan: RAG System（检索增强生成）

**Branch**: `011-rag-system` | **Date**: 2026-07-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/011-rag-system/spec.md`

## Summary

为 AgentScope Rust 实现 RAG（检索增强生成）能力。创建两个新 crate：`agent_scope_embedding`（Embedding 模型 trait + DashScope 实现）和 `agent_scope_rag`（Parser/Chunker/VectorStore/KnowledgeBase + RAGMiddleware）。VectorStore 仅定义 trait 抽象，不做具体向量数据库实现。RAGMiddleware 通过 Feature 007 的 Middleware trait 集成到 Agent 管道。

核心策略：EmbeddingModel trait 与 ChatModel trait 平行设计（参考 Feature 003）；Parser → Chunker 管道将非结构化文档转为 Chunk；KnowledgeBase 将 Embedding + VectorStore 封装为 search/insert/delete/list 四个操作；RAGMiddleware 支持 static（自动上下文注入）和 agentic（Tool 暴露）两种模式。

## Technical Context

**Language/Version**: Rust 1.75+ (workspace edition 2021)

**Primary Dependencies**: tokio (async runtime), serde/serde_json (serialization), async-trait, reqwest (DashScope HTTP), sha2 (cache key hashing), uuid (auto-generated document IDs)

**Storage**: FileEmbeddingCache (SHA-256 keyed, filesystem-based); VectorStore 仅 trait 抽象（无具体存储实现）

**Testing**: cargo test (unit + integration), per-crate tests/ layout, mock implementations for EmbeddingModel and VectorStore

**Target Platform**: Linux/macOS server (single-process), library crate

**Project Type**: library (embedded in agent applications)

**Performance Goals**: SC-004: 1MB 纯文本解析+切分 < 1s; SC-005: 100 查询缓存全命中零 API 调用

**Constraints**: Single-process; Constitution §10 结构化并发; Constitution §13 稳定错误模型; Constitution §12 稳定数据协议; VectorStore 仅 trait（无具体实现）

**Scale/Scope**: 2 new crates (`agent_scope_embedding`, `agent_scope_rag`); 1 extended crate (`agent_scope_dashscope`); DashScope 复用 Feature 005 HTTP 客户端模式

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | Principle | Status | Notes |
|---|-----------|--------|-------|
| I | 兼容性优先 | ✅ PASS | Python AgentScope `EmbeddingModelBase.__call__` 语义等价；`VectorStoreBase` 操作契约等价；RAGMiddleware 行为对齐 |
| II | 锁定上游版本 | ✅ PASS | 上游版本已在 Constitution 中锁定；无新的上游版本变更 |
| III | Python 是行为基准 | ✅ PASS | RAG 模块行为以 Python AgentScope 参考实现为基准；API 签名对齐 |
| IV | 先定义契约 | ✅ PASS | spec.md 已批准（5 US, 41 FRs, 7 SCs）；contracts/ 定义 4 个接口契约 |
| V | 不允许伪兼容 | ✅ PASS | 不支持的 Parser 格式（PDF/PPT）显式标记为 unsupported；VectorStore 无 mock 实现（仅 trait） |
| VI | 测试驱动兼容性 | ✅ PASS | Mock EmbeddingModel + Mock VectorStore 使 KnowledgeBase 测试确定性；FileEmbeddingCache 可重复测试 |
| VII | Trace 是核心验收产物 | ✅ PASS | RAGMiddleware 的 pre_reply/post_acting 钩子行为和 Tool 调用产生的全部副作用可追踪 |
| VIII | Rust 原生设计 | ✅ PASS | EmbeddingModel/Chunker/Parser/VectorStore 均为 trait；Arc<dyn Trait> 风格；enum 表达有限状态 |
| IX | 安全 Rust 优先 | ✅ PASS | 无 unsafe 代码；`#![deny(unsafe_code)]` 于新 crate |
| X | 结构化并发 | ✅ PASS | KnowledgeBase.search() 对多个查询词使用 `futures::join_all` 并发搜索，有边界控制 |
| XI | 分层与依赖方向 | ✅ PASS | `agent_scope_embedding` 仅依赖 core 层；`agent_scope_rag` 依赖 model + agent + types；无循环依赖 |
| XII | 稳定的数据协议 | ✅ PASS | Chunk/Section/VectorRecord 均 #[derive(Serialize, Deserialize)]；未知字段 #[serde(default)] |
| XIII | 稳定错误模型 | ✅ PASS | typed errors: EmbeddingError, ParserError, ChunkerError, VectorStoreError, KnowledgeBaseError |
| XIV | 可观测性 | ✅ PASS | RAGMiddleware 关键路径：pre_reply hook 耗时、Tool 调用名称/参数、搜索命中数；tracing span 覆盖 |
| XV | 性能不能牺牲正确性 | ✅ PASS | ApproxTokenChunker 启发式算法主动标注不保证精确性；metadata_filter 安全边界不可被绕过 |
| XVI | 小步交付 | ✅ PASS | Feature 011 是 §16 路线图的第 11 步，独立可测、独立可交付 |
| XVII | 完成的定义 | ✅ PASS | Will comply — 41 FRs 全部实现、tests 通过、clippy 0 warnings、compatibility matrix 更新 |
| XVIII | 兼容性分级 | ✅ PASS | Target: L2（核心行为兼容）— API 语义对齐 Python 参考实现，数据结构 Rust 原生设计 |
| XIX | 变更治理 | ✅ PASS | 无宪法违反；trait 设计遵循已确立的 ChatModel/Memory/Middleware 模式 |

**Gate result**: ALL 19 principles PASS. No violations to justify.

### Post-Design Re-evaluation (after Phase 1)

Re-evaluated after research.md, data-model.md, contracts/, and quickstart.md completion:

| # | Change from Initial | Status |
|---|---------------------|--------|
| IV | contracts/ confirms 4 interface contracts defined (embedding-model, parser-chunker, vector-store, knowledge-base) | ✅ STILL PASS |
| VIII | data-model confirms Rust-native patterns: 5 traits, Arc<dyn Trait>, enums for EmbeddingInput/SectionContent/RAGMode | ✅ STILL PASS |
| XI | Crate dependency graph confirmed: embedding → types, rag → embedding + model + agent + types. No cycles. | ✅ STILL PASS |
| XII | All 7 data structs have #[derive(Serialize, Deserialize)], metadata uses HashMap<String, String> per spec | ✅ STILL PASS |
| XIII | 5 error enums confirmed (EmbeddingError, ParserError, ChunkerError, VectorStoreError, KnowledgeBaseError) spanning all §13 categories | ✅ STILL PASS |

**Post-design result**: ALL 19 principles STILL PASS. Design artifacts do not introduce any constitution violations.

## Project Structure

### Documentation (this feature)

```text
specs/011-rag-system/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── embedding-model.md
│   ├── parser-chunker.md
│   ├── vector-store.md
│   └── knowledge-base.md
├── spec.md              # Feature specification (existing)
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/
├── agent_scope_embedding/           # NEW
│   ├── Cargo.toml                   # depends: agent_scope_types, serde, async-trait, sha2
│   ├── src/
│   │   ├── lib.rs                   # pub mod embedding; pub mod cache;
│   │   ├── embedding.rs             # EmbeddingModel trait, EmbeddingInput, EmbeddingResponse, EmbeddingUsage, EmbeddingModelCard
│   │   └── cache.rs                 # EmbeddingCache trait, FileEmbeddingCache
│   └── tests/
│       ├── embedding_trait_tests.rs  # Mock EmbeddingModel tests
│       └── cache_tests.rs           # FileEmbeddingCache tests
│
├── agent_scope_dashscope/           # EXTENDED
│   ├── src/
│   │   ├── lib.rs                   # +pub mod embedding;
│   │   └── embedding.rs             # NEW: DashScopeEmbeddingModel
│   └── tests/
│       └── embedding_tests.rs       # NEW: DashScope embedding integration tests
│
├── agent_scope_rag/                 # NEW
│   ├── Cargo.toml                   # depends: agent_scope_types, agent_scope_embedding, agent_scope_model, agent_scope_agent, serde, async-trait, futures, uuid
│   ├── src/
│   │   ├── lib.rs                   # pub mod parser; pub mod chunker; pub mod vector_store; pub mod knowledge_base; pub mod rag_middleware;
│   │   ├── parser.rs                # Section, Parser trait, TextParser
│   │   ├── chunker.rs               # Chunk, Chunker trait, ApproxTokenChunker
│   │   ├── vector_store.rs          # VectorStore trait, VectorRecord, VectorSearchResult, DocumentSummary
│   │   ├── knowledge_base.rs        # KnowledgeBase struct (implements search/insert/delete/list)
│   │   └── rag_middleware.rs         # RAGMiddleware (implements Middleware trait from Feature 007)
│   └── tests/
│       ├── parser_tests.rs          # TextParser tests
│       ├── chunker_tests.rs         # ApproxTokenChunker tests
│       ├── vector_store_mock.rs     # Mock VectorStore for testing
│       ├── knowledge_base_tests.rs  # KnowledgeBase unit tests (mock embedding + mock vs)
│       └── rag_middleware_tests.rs  # RAGMiddleware integration tests
```

**Structure Decision**: 创建两个新 crate 而非扩展现有 crate。`agent_scope_embedding` 作为与 `agent_scope_model`（Feature 003 ChatModel）平行的抽象层，职责单一。`agent_scope_rag` 集中所有 RAG 管道逻辑（Parser/Chunker/VectorStore/KnowledgeBase/RAGMiddleware），避免跨 crate 的碎片化依赖。DashScopeEmbeddingModel 实现在 `agent_scope_dashscope` 中，遵循 Feature 005 的 provider 架构（具体实现在 provider crate，trait 在抽象 crate）。

## Complexity Tracking

> No violations to justify. All 19 Constitution principles pass.
