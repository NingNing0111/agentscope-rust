# Tasks: RAG System（检索增强生成）

**Input**: Design documents from `specs/011-rag-system/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Included — quickstart.md defines 7 test scenarios. Test tasks are mapped to each user story.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create new crate scaffolding and configure workspace dependencies

- [x] T001 Create `crates/agent_scope_embedding/` directory structure with `Cargo.toml`, `src/lib.rs`, `tests/` directory per plan.md
- [x] T002 [P] Configure `agent_scope_embedding/Cargo.toml` dependencies: `agent_scope_types` (path), `serde`, `serde_json`, `async-trait`, `sha2`, `tokio`
- [x] T003 [P] Add `#![deny(unsafe_code)]` and `#![deny(clippy::unwrap_used)]` to `agent_scope_embedding/src/lib.rs` per Constitution §IX
- [x] T004 Create `crates/agent_scope_rag/` directory structure with `Cargo.toml`, `src/lib.rs`, `tests/` directory per plan.md
- [x] T005 [P] Configure `agent_scope_rag/Cargo.toml` dependencies: `agent_scope_types` (path), `agent_scope_embedding` (path), `agent_scope_model` (path), `agent_scope_agent` (path), `serde`, `serde_json`, `async-trait`, `futures`, `uuid`, `tokio`
- [x] T006 [P] Add `#![deny(unsafe_code)]` and `#![deny(clippy::unwrap_used)]` to `agent_scope_rag/src/lib.rs` per Constitution §IX
- [x] T007 [P] Register `agent_scope_embedding` and `agent_scope_rag` as workspace members in root `Cargo.toml`
- [x] T008 Verify workspace builds with `rtk cargo check --workspace` (new crates compile empty)

**Checkpoint**: Workspace structure ready — two new crates compile, no dependency errors

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core data types and error enums that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Embedding crate foundation

- [x] T009 [P] Define `EmbeddingError` enum variants (`ApiKeyMissing`, `HttpError`, `ApiError`, `MultimodalNotSupported`, `DeserializationError`, `DimensionMismatch`) in `crates/agent_scope_embedding/src/error.rs` per contracts/embedding-model.md
- [x] T010 [P] Implement `std::fmt::Display` and `std::error::Error` for `EmbeddingError` in `crates/agent_scope_embedding/src/error.rs` per Constitution §XIII
- [x] T011 Define `DataBlockData` re-export from `agent_scope_types` (or minimal stub) in `crates/agent_scope_embedding/src/lib.rs` — needed by `EmbeddingInput::DataBlock`

### RAG crate foundation

- [x] T012 [P] Define `ParserError` enum (`UnsupportedFormat`, `EncodingError`) in `crates/agent_scope_rag/src/error.rs` per contracts/parser-chunker.md
- [x] T013 [P] Define `ChunkerError` enum (`InvalidParameters`) in `crates/agent_scope_rag/src/error.rs` per contracts/parser-chunker.md
- [x] T014 [P] Define `VectorStoreError` enum (`CollectionNotFound`, `CollectionAlreadyExists`, `DimensionMismatch`, `BackendError`, `Timeout`) in `crates/agent_scope_rag/src/error.rs` per contracts/vector-store.md
- [x] T015 [P] Define `KnowledgeBaseError` enum (`EmbeddingError`, `VectorStoreError`, `CountMismatch`, `DimensionMismatch`) in `crates/agent_scope_rag/src/error.rs` per contracts/knowledge-base.md
- [x] T016 [P] Implement `std::fmt::Display` and `std::error::Error` for `ParserError`, `ChunkerError`, `VectorStoreError`, `KnowledgeBaseError` in `crates/agent_scope_rag/src/error.rs`
- [x] T017 Setup module declarations in `crates/agent_scope_rag/src/lib.rs`: `pub mod error; pub mod parser; pub mod chunker; pub mod vector_store; pub mod knowledge_base; pub mod rag_middleware;`

**Checkpoint**: All error types defined, module structure in place — user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - 文本嵌入 (Priority: P1) 🎯 MVP

**Goal**: 开发者可使用 `EmbeddingModel` trait 将文本嵌入为稠密向量，包括 DashScope provider 实现和文件缓存

**Independent Test**: 使用 mock EmbeddingModel 调用 `embed()`，验证返回向量维度与 `model_card().dimensions` 一致；使用 `FileEmbeddingCache` 验证缓存命中/未命中

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T018 [P] [US1] Write mock `EmbeddingModel` unit tests (embed returns fixed dims, model_card, supports_multimodal, DataBlock rejection) in `crates/agent_scope_embedding/tests/embedding_trait_tests.rs` per quickstart.md Scenario 1
- [x] T019 [P] [US1] Write `FileEmbeddingCache` unit tests (hit, miss, 100-entries, overwrite) in `crates/agent_scope_embedding/tests/cache_tests.rs` per quickstart.md Scenario 2

### Implementation for User Story 1 — Embedding Trait Layer

- [x] T020 [US1] Define `EmbeddingInput` enum (`Text(String)`, `DataBlock(DataBlockData)`) with `From<String>` and `From<&str>` impls in `crates/agent_scope_embedding/src/embedding.rs` per data-model.md Entity 1
- [x] T021 [P] [US1] Define `EmbeddingUsage` struct (`total_tokens: u32`) with `Default` derive in `crates/agent_scope_embedding/src/embedding.rs` per data-model.md Entity 3
- [x] T022 [P] [US1] Define `EmbeddingResponse` struct (`embeddings: Vec<Vec<f32>>`, `usage: EmbeddingUsage`) in `crates/agent_scope_embedding/src/embedding.rs` per data-model.md Entity 2
- [x] T023 [P] [US1] Define `EmbeddingModelCard` struct (`name: String`, `dimensions: u32`, `supports_multimodal: bool`) in `crates/agent_scope_embedding/src/embedding.rs` per data-model.md Entity 4
- [x] T024 [US1] Define `EmbeddingModel` async trait with `embed()`, `model_card()`, `supports_multimodal()` methods in `crates/agent_scope_embedding/src/embedding.rs` per data-model.md Entity 5 (depends on T020-T023)
- [x] T025 [US1] Add `pub mod embedding; pub mod cache; pub mod error;` and re-exports to `crates/agent_scope_embedding/src/lib.rs`

### Implementation for User Story 1 — Cache Layer

- [x] T026 [US1] Define `EmbeddingCache` trait (`lookup(key) -> Option<Vec<Vec<f32>>>`, `store(key, embeddings)`) in `crates/agent_scope_embedding/src/cache.rs` per data-model.md Entity 6
- [x] T027 [US1] Implement `FileEmbeddingCache` struct (`cache_dir: PathBuf`, `new()`, `lookup()`, `store()`) in `crates/agent_scope_embedding/src/cache.rs` per data-model.md Entity 7
- [x] T028 [US1] SHA-256 hash key generation for FileEmbeddingCache input in `crates/agent_scope_embedding/src/cache.rs` per research.md R4

### Implementation for User Story 1 — DashScope Provider

- [x] T029 [US1] Implement `DashScopeEmbeddingModel` struct with `new(api_key, model_card)` and `with_cache(cache)` in `crates/agent_scope_dashscope/src/embedding.rs` per contracts/embedding-model.md
- [x] T030 [US1] Implement `EmbeddingModel` trait for `DashScopeEmbeddingModel` — HTTP POST to `/api/v1/services/embeddings/text-embedding/text-embedding` in `crates/agent_scope_dashscope/src/embedding.rs` per research.md R3
- [x] T031 [US1] Implement DashScope response deserialization (map `output.embeddings[index].embedding` → `Vec<Vec<f32>>`) in `crates/agent_scope_dashscope/src/embedding.rs`
- [x] T032 [US1] Implement `EmbeddingError` mapping for DashScope HTTP errors (non-200, dimension mismatch, missing API key) in `crates/agent_scope_dashscope/src/embedding.rs`
- [x] T033 [US1] Integrate `EmbeddingCache` lookup in `DashScopeEmbeddingModel::embed()` — check cache before HTTP call, store result after in `crates/agent_scope_dashscope/src/embedding.rs`
- [x] T034 [US1] Add `pub mod embedding;` to `crates/agent_scope_dashscope/src/lib.rs`
- [x] T035 [US1] Write DashScope embedding integration tests (requires API key, `#[ignore]` default) in `crates/agent_scope_dashscope/tests/embedding_tests.rs` per quickstart.md Scenario 7

### Verification for User Story 1

- [x] T036 [US1] Run `rtk cargo test -p agent_scope_embedding` — verify all embedding trait and cache tests pass
- [x] T037 [US1] Run `rtk cargo test -p agent_scope_dashscope` — verify DashScope embedding tests pass (non-ignored)

**Checkpoint**: Embedding 模型层完成 — trait 可 mock, 缓存可用, DashScope 可调用

---

## Phase 4: User Story 2 - 文档解析与切分 (Priority: P1)

**Goal**: 开发者可将文本文件解析为 Section，再切分为可索引的 Chunk

**Independent Test**: 500 词文本经 TextParser → ApproxTokenChunker(chunk_size=100, overlap=20) 产生 5+ 个 Chunk，验证 source/chunk_index/total_chunks 元数据

### Tests for User Story 2

- [x] T038 [P] [US2] Write `TextParser` unit tests (basic .txt, .md, empty file, UTF-8 error) in `crates/agent_scope_rag/tests/parser_tests.rs` per quickstart.md Scenario 3
- [x] T039 [P] [US2] Write `ApproxTokenChunker` unit tests (500-word doc, cross-section boundary, empty sections, invalid params) in `crates/agent_scope_rag/tests/chunker_tests.rs` per quickstart.md Scenario 3

### Implementation for User Story 2

- [x] T040 [P] [US2] Define `SectionContent` enum (`Text(String)`, `DataBlock(DataBlockData)`) in `crates/agent_scope_rag/src/parser.rs` per data-model.md Entity 8
- [x] T041 [P] [US2] Define `Section` struct (`content: SectionContent`, `source: String`, `metadata: HashMap<String, String>`) in `crates/agent_scope_rag/src/parser.rs` per data-model.md Entity 8
- [x] T042 [US2] Define `Parser` trait with `parse(file: Vec<u8>, filename: &str) -> Result<Vec<Section>, ParserError>` in `crates/agent_scope_rag/src/parser.rs` per data-model.md Entity 9
- [x] T043 [US2] Implement `TextParser` struct — `parse()` converts bytes to UTF-8 string, wraps as single `Section`, empty file returns `Ok(vec![])` in `crates/agent_scope_rag/src/parser.rs` per contracts/parser-chunker.md
- [x] T044 [US2] Define `Chunk` struct (`content: String`, `source: String`, `chunk_index: usize`, `total_chunks: usize`, `metadata: HashMap<String, String>`) in `crates/agent_scope_rag/src/chunker.rs` per data-model.md Entity 10
- [x] T045 [US2] Define `Chunker` trait with `chunk(sections: Vec<Section>) -> Result<Vec<Chunk>, ChunkerError>` in `crates/agent_scope_rag/src/chunker.rs` per data-model.md Entity 11
- [x] T046 [US2] Implement `ApproxTokenChunker` struct (`chunk_size: usize`, `overlap: usize`) with parameter validation (`chunk_size > overlap`) in `crates/agent_scope_rag/src/chunker.rs` per data-model.md Entity 12
- [x] T047 [US2] Implement token counting heuristic (English: word-based, CJK: chars/4) in `crates/agent_scope_rag/src/chunker.rs` per research.md R11
- [x] T048 [US2] Implement sliding window chunk algorithm — Section boundary not crossed, chunk_index global increment for same source in `crates/agent_scope_rag/src/chunker.rs` per research.md R6
- [x] T049 [US2] Handle edge cases: empty sections → empty chunks, single-token sections, overlap ≥ content length in `crates/agent_scope_rag/src/chunker.rs`

### Verification for User Story 2

- [x] T050 [US2] Run `rtk cargo test -p agent_scope_rag -- parser` — verify all parser tests pass
- [x] T051 [US2] Run `rtk cargo test -p agent_scope_rag -- chunker` — verify all chunker tests pass

**Checkpoint**: 文档解析与切分管道完成 — 文本文件 → Section → Chunk 端到端可用

---

## Phase 5: User Story 3 - 向量存储抽象 (Priority: P1)

**Goal**: 定义 `VectorStore` async trait 及其相关数据类型，作为具体向量数据库的接口契约

**Independent Test**: 使用 mock VectorStore 实现验证 trait 方法签名、数据类型匹配、search/insert/delete/list 操作

### Tests for User Story 3

- [x] T052 [P] [US3] Implement mock `VectorStore` (in-memory HashMap backend) in `crates/agent_scope_rag/tests/vector_store_mock.rs` per quickstart.md
- [x] T053 [P] [US3] Write `VectorStore` trait tests (has_collection/create_collection, insert→search roundtrip, delete→empty search, list_documents, metadata_filter) in `crates/agent_scope_rag/tests/vector_store_mock.rs` per quickstart.md Scenario 3 (extended: explicit vector store mock tests)

### Implementation for User Story 3

- [x] T054 [P] [US3] Define `VectorRecord` struct (`vector: Vec<f32>`, `document_id: String`, `chunk: Chunk`) in `crates/agent_scope_rag/src/vector_store.rs` per data-model.md Entity 14
- [x] T055 [P] [US3] Define `VectorSearchResult` struct (`score: f32`, `document_id: String`, `chunk: Chunk`) in `crates/agent_scope_rag/src/vector_store.rs` per data-model.md Entity 15
- [x] T056 [P] [US3] Define `DocumentSummary` struct (`document_id: String`, `source: String`, `chunk_count: usize`, `metadata: HashMap<String, String>`) in `crates/agent_scope_rag/src/vector_store.rs` per data-model.md Entity 16
- [x] T057 [US3] Define `VectorStore` async trait with 6 methods (`has_collection`, `create_collection`, `search`, `insert`, `delete`, `list_documents`) in `crates/agent_scope_rag/src/vector_store.rs` per data-model.md Entity 13 (depends on T054-T056)
- [x] T058 [US3] Add doc comments to `VectorStore` trait documenting behavioral contract (metadata_filter exact match, score descending, idempotent delete, etc.) per contracts/vector-store.md

### Verification for User Story 3

- [x] T059 [US3] Run `rtk cargo test -p agent_scope_rag -- vector_store_mock` — verify all mock VectorStore tests pass

**Checkpoint**: VectorStore trait 定义完成 — 具体向量数据库实现有清晰的接口契约

---

## Phase 6: User Story 4 - 知识库运行时代理 (Priority: P2)

**Goal**: KnowledgeBase 封装 Embedding + VectorStore，提供 search/insert/delete/list 四个操作

**Independent Test**: 使用 mock EmbeddingModel + mock VectorStore 创建 KnowledgeBase，验证 search 去重排序、insert 自动 ID、delete 幂等、list_documents、metadata_filter 覆盖

### Tests for User Story 4

- [x] T060 [P] [US4] Write KnowledgeBase unit tests — insert_and_search: insert chunks → search returns matching results in `crates/agent_scope_rag/tests/knowledge_base_tests.rs` per quickstart.md Scenario 4
- [x] T061 [P] [US4] Write KnowledgeBase unit tests — search_deduplication: same (doc_id, chunk_index) → keep highest score in `crates/agent_scope_rag/tests/knowledge_base_tests.rs`
- [x] T062 [P] [US4] Write KnowledgeBase unit tests — delete_document: insert then delete → subsequent search returns empty in `crates/agent_scope_rag/tests/knowledge_base_tests.rs`
- [x] T063 [P] [US4] Write KnowledgeBase unit tests — list_documents: returns correct DocumentSummary after inserts in `crates/agent_scope_rag/tests/knowledge_base_tests.rs`
- [x] T064 [P] [US4] Write KnowledgeBase unit tests — metadata_filter_override: filter wins over chunk metadata in `crates/agent_scope_rag/tests/knowledge_base_tests.rs`
- [x] T065 [P] [US4] Write KnowledgeBase unit tests — lazy_collection_init: first operation auto-creates collection in `crates/agent_scope_rag/tests/knowledge_base_tests.rs`
- [x] T066 [P] [US4] Write KnowledgeBase unit tests — count_mismatch: embedding returns wrong number of vectors → error in `crates/agent_scope_rag/tests/knowledge_base_tests.rs`

### Implementation for User Story 4

- [x] T067 [US4] Implement `KnowledgeBase` struct with all fields (`name`, `description`, `embedding_model`, `vector_store`, `collection`, `metadata_filter`, `initialized: OnceCell<()>`) in `crates/agent_scope_rag/src/knowledge_base.rs` per data-model.md Entity 17
- [x] T068 [US4] Implement `KnowledgeBase::new()` constructor in `crates/agent_scope_rag/src/knowledge_base.rs` per contracts/knowledge-base.md
- [x] T069 [US4] Implement `KnowledgeBase::ensure_initialized()` — lazy check `has_collection()` + `create_collection()` using `OnceCell` in `crates/agent_scope_rag/src/knowledge_base.rs` per research.md R8
- [x] T070 [US4] Implement `KnowledgeBase::search()` — embed queries → concurrent `VectorStore::search()` per query → deduplicate by `(document_id, chunk_index)` → sort by score desc → `score_threshold` filter → `top_k` truncation in `crates/agent_scope_rag/src/knowledge_base.rs` per FR-030/FR-031
- [x] T071 [US4] Implement `KnowledgeBase::insert_document()` — auto-generate UUID v4 if no document_id → metadata merge (document_metadata < chunk.metadata < metadata_filter) → embed chunks → verify count match → `VectorStore::insert()` in `crates/agent_scope_rag/src/knowledge_base.rs` per FR-032/FR-033/FR-034
- [x] T072 [US4] Implement `KnowledgeBase::delete_document()` — delegate to `VectorStore::delete()` in `crates/agent_scope_rag/src/knowledge_base.rs` per FR-035
- [x] T073 [US4] Implement `KnowledgeBase::list_documents()` — delegate to `VectorStore::list_documents()` with `metadata_filter` in `crates/agent_scope_rag/src/knowledge_base.rs` per FR-036
- [x] T074 [US4] Handle edge cases: empty queries → empty results, empty chunks insert → return empty string ID, DataBlock query to non-multimodal model → silently dropped in `crates/agent_scope_rag/src/knowledge_base.rs`

### Verification for User Story 4

- [x] T075 [US4] Run `rtk cargo test -p agent_scope_rag -- knowledge_base` — verify all KnowledgeBase tests pass

**Checkpoint**: KnowledgeBase 完成 — search/insert/delete/list 四操作可用, metadata_filter 安全边界生效

---

## Phase 7: User Story 5 - Agent 知识检索集成 (Priority: P3)

**Goal**: RAGMiddleware 集成到 Agent pipeline，支持 static（自动上下文注入）和 agentic（Tool 暴露）两种模式

**Independent Test**: 创建 RAGMiddleware，分别测试 static 模式下 HintBlock 注入、agentic 模式下 Tool 注册与执行

### Tests for User Story 5

- [x] T076 [P] [US5] Write RAGMiddleware static mode tests — injects context on pre_reply with matching chunks in `crates/agent_scope_rag/tests/rag_middleware_tests.rs` per quickstart.md Scenario 5
- [x] T077 [P] [US5] Write RAGMiddleware static mode tests — empty results → no context injection in `crates/agent_scope_rag/tests/rag_middleware_tests.rs`
- [x] T078 [P] [US5] Write RAGMiddleware static mode tests — multiple KBs → aggregated results in `crates/agent_scope_rag/tests/rag_middleware_tests.rs`
- [x] T079 [P] [US5] Write RAGMiddleware agentic mode tests — registers `search_{kb_name}` Tool on post_acting in `crates/agent_scope_rag/tests/rag_middleware_tests.rs` per quickstart.md Scenario 6
- [x] T080 [P] [US5] Write RAGMiddleware agentic mode tests — Tool execution calls kb.search() and returns formatted results in `crates/agent_scope_rag/tests/rag_middleware_tests.rs`
- [x] T081 [P] [US5] Write RAGMiddleware agentic mode tests — multiple KBs → each registers independent Tool in `crates/agent_scope_rag/tests/rag_middleware_tests.rs`
- [x] T082 [P] [US5] Write RAGMiddleware agentic mode tests — duplicate post_acting calls → no duplicate Tool registration in `crates/agent_scope_rag/tests/rag_middleware_tests.rs`

### Implementation for User Story 5

- [x] T083 [US5] Define `RAGMode` enum (`Static`, `Agentic`) in `crates/agent_scope_rag/src/rag_middleware.rs` per data-model.md Entity 18
- [x] T084 [US5] Implement `RAGMiddleware` struct (`knowledge_bases: Vec<Arc<KnowledgeBase>>`, `mode: RAGMode`, `top_k: usize`, `score_threshold: Option<f32>`) with `new()` constructor in `crates/agent_scope_rag/src/rag_middleware.rs`
- [x] T085 [US5] Implement static mode: override `pre_reply()` — extract latest user message → search all KBs → build `HintBlock` with source citations → inject into `agent_state.context` in `crates/agent_scope_rag/src/rag_middleware.rs` per FR-039
- [x] T086 [US5] Implement agentic mode: override `post_acting()` — for each KB, create Tool with name `search_{sanitized_kb_name}`, description from `kb.description`, parameter `query: String` → register to `tools` vec in `crates/agent_scope_rag/src/rag_middleware.rs` per FR-040
- [x] T087 [US5] Implement Tool execution logic for agentic mode — embed query → `kb.search()` → format results as text string for LLM consumption in `crates/agent_scope_rag/src/rag_middleware.rs`
- [x] T088 [US5] Implement deduplication of registered tools across repeated `post_acting` calls (check existing tool names before adding) in `crates/agent_scope_rag/src/rag_middleware.rs`
- [x] T089 [US5] Implement `Middleware` trait for `RAGMiddleware` (`name()` returns `"RAGMiddleware"`, `pre_reply` per static mode, `post_acting` per agentic mode, `post_reply` uses default) in `crates/agent_scope_rag/src/rag_middleware.rs` per contracts/knowledge-base.md
- [x] T090 [US5] Handle edge cases: User message with DataBlock → text-based embedding only, KB name sanitization (lowercase + underscore), zero KBs bound → no-op

### Verification for User Story 5

- [x] T091 [US5] Run `rtk cargo test -p agent_scope_rag -- rag_middleware` — verify all RAGMiddleware tests pass

**Checkpoint**: RAGMiddleware 完成 — static 模式自动注入知识, agentic 模式暴露搜索 Tool

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Quality gates, documentation, and final validation

- [x] T092 [P] Run `rtk cargo clippy --workspace` — fix all warnings across all crates
- [x] T093 [P] Run `rtk cargo fmt --all -- --check` — verify formatting compliance
- [x] T094 Run `rtk cargo test --workspace` — verify ALL tests pass (embedding + dashscope + rag)
- [x] T095 [P] Add rustdoc comments to all public types and traits in `crates/agent_scope_embedding/src/`
- [x] T096 [P] Add rustdoc comments to all public types and traits in `crates/agent_scope_rag/src/`
- [x] T097 Validate quickstart.md scenarios 1-6 pass end-to-end per quickstart.md Test Summary
- [x] T098 Update compatibility matrix per Constitution §XVIII (L2 target for RAG system)
- [x] T099 [P] Run `rtk cargo check --workspace` — final verification, zero warnings

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion (T001-T008) — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational (Phase 2) — No dependencies on US2/US3
- **User Story 2 (Phase 4)**: Depends on Foundational (Phase 2) — No dependencies on US1/US3
- **User Story 3 (Phase 5)**: Depends on Foundational (Phase 2) — No dependencies on US1/US2
- **User Story 4 (Phase 6)**: Depends on US1 (EmbeddingModel) + US3 (VectorStore) + Foundational
- **User Story 5 (Phase 7)**: Depends on US4 (KnowledgeBase) + Feature 007 (Middleware trait)
- **Polish (Phase 8)**: Depends on all desired user stories being complete

### User Story Dependencies

```
Phase 1: Setup
    ↓
Phase 2: Foundational
    ↓
┌───────┬───────┬───────┐
│  US1  │  US2  │  US3  │  ← All P1, can run in PARALLEL
│ (P1)  │ (P1)  │ (P1)  │
└───┬───┴───────┴───┬───┘
    └───────┬────────┘
            ↓
        ┌───────┐
        │  US4  │  ← P2, depends on US1 + US3
        │ (P2)  │
        └───┬───┘
            ↓
        ┌───────┐
        │  US5  │  ← P3, depends on US4 + Feature 007
        │ (P3)  │
        └───┬───┘
            ↓
        Phase 8: Polish
```

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Types/Structs/Enums before Traits (traits reference types)
- Traits before Implementations
- Provider implementations last (depend on trait)
- Story complete before moving to dependent stories (US4, US5)

### Parallel Opportunities

- **Phase 1**: T002, T003, T005, T006, T007 — all parallel
- **Phase 2**: T009-T010 (EmbeddingError) parallel to T012-T016 (RAG errors). T011 last.
- **Phase 3**: T018-T019 (tests) parallel to T020-T023 (types). T024 (trait) after types.
- **Phase 4**: T038-T039 (tests) parallel to T040-T041 (Section). T040-T041 parallel.
- **Phase 5**: T054-T056 (structs) all parallel. T052-T053 parallel to T057.
- **Phase 6**: T060-T066 (all tests) parallel to each other.
- **Phase 7**: T076-T082 (all tests) parallel to each other.
- **Phase 8**: T092, T093, T095, T096, T099 all parallel.
- **Cross-phase**: US1, US2, US3 can all be worked on in parallel after Phase 2.

---

## Parallel Example: User Story 1

```bash
# Launch all type definitions together:
Task: "Define EmbeddingInput enum in crates/agent_scope_embedding/src/embedding.rs"
Task: "Define EmbeddingUsage struct in crates/agent_scope_embedding/src/embedding.rs"
Task: "Define EmbeddingResponse struct in crates/agent_scope_embedding/src/embedding.rs"
Task: "Define EmbeddingModelCard struct in crates/agent_scope_embedding/src/embedding.rs"

# After types are done, launch tests + trait definition:
Task: "Define EmbeddingModel trait in crates/agent_scope_embedding/src/embedding.rs"
Task: "Write mock EmbeddingModel tests in crates/agent_scope_embedding/tests/embedding_trait_tests.rs"
```

## Parallel Example: Phase 2 + US1/US2/US3 Kickoff

```bash
# Phase 2: Define all error types in parallel
Task: "Define EmbeddingError, ParserError, ChunkerError, VectorStoreError, KnowledgeBaseError"

# After Phase 2 completes, launch all three P1 stories simultaneously:
# Developer A: US1 — EmbeddingModel trait + cache + DashScope
# Developer B: US2 — Parser + Chunker
# Developer C: US3 — VectorStore trait + data types
```

---

## Implementation Strategy

### MVP First (US1 + US2 + US3 — All P1)

1. Complete Phase 1: Setup (T001-T008)
2. Complete Phase 2: Foundational (T009-T017) — **CRITICAL BLOCKER**
3. Complete Phase 3: User Story 1 — Embedding (T018-T037)
4. Complete Phase 4: User Story 2 — Parser & Chunker (T038-T051)
5. Complete Phase 5: User Story 3 — VectorStore (T052-T059)
6. **STOP and VALIDATE**: US1, US2, US3 each independently testable
7. All P1 stories form the RAG foundation

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US1 → Embedding 可用 → Verify (mock + DashScope)
3. Add US2 → 文档管道可用 → Verify (Parser + Chunker)
4. Add US3 → VectorStore 契约就绪 → Verify (mock implementation)
5. Add US4 → KnowledgeBase 端到端可用 → Verify (mock backends) 🎯 **主要价值点**
6. Add US5 → Agent 集成完成 → Verify (static + agentic modes)
7. Polish → Quality gates passed → Ready for release

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together (T001-T017)
2. Once Foundational is done:
   - Developer A: US1 — Embedding (T018-T037)
   - Developer B: US2 — Parser & Chunker (T038-T051)
   - Developer C: US3 — VectorStore (T052-T059)
3. After US1 + US3 complete:
   - Developer A: US4 — KnowledgeBase (T060-T075)
4. After US4 completes:
   - Developer A: US5 — RAGMiddleware (T076-T091)
5. All: Polish phase (T092-T099)

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Tests are based on quickstart.md scenarios — write tests first, ensure they FAIL before implementation
- `Arc<dyn EmbeddingModel>` and `Arc<dyn VectorStore>` are the primary integration points
- DashScopeEmbeddingModel reuses Feature 005's HTTP client pattern — consult `crates/agent_scope_dashscope/src/lib.rs` for existing patterns
- RAGMiddleware implements the Middleware trait from Feature 007 — consult `crates/agent_scope_agent/src/middleware.rs` for trait definition
- Constitution §IX: all new crates must use `#![deny(unsafe_code)]`
- Constitution §XIII: all errors must be typed enums implementing `std::error::Error`
- Commit after each phase completion or logical task group
