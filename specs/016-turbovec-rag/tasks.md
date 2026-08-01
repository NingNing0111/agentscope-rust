# Tasks: Turbovec RAG 向量存储实现

**Input**: Design documents from `/specs/016-turbovec-rag/`

**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Tests**: Tests are included per spec — each US has independently testable acceptance scenarios. Tests follow the TDD pattern: write failing test first, then implement.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- Library crate: `crates/agent_scope_rag/src/`, `crates/agent_scope_rag/tests/`
- `turbovec` crate 路径（如果作为 workspace member）: `turbovec/turbovec/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 项目依赖和模块注册

- [X] T001 Add `turbovec` dependency (version 0.9.x, from crates.io) in `crates/agent_scope_rag/Cargo.toml`
- [X] T002 [P] Register `turbovec_store` module (`pub mod turbovec_store;`) in `crates/agent_scope_rag/src/lib.rs`

**Checkpoint**: `cargo check -p agent_scope_rag` 编译通过

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: `TurbovecVectorStore` 的类型定义、内部数据结构、辅助函数 —— 所有用户故事依赖的基础

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T003 Define `TurbovecVectorStore` struct (fields: `bit_width: usize`, `collections: RwLock<HashMap<String, Arc<CollectionInner>>>`) in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T004 Define `CollectionInner` struct (fields: `dim: usize`, `index: IdMapIndex`, `chunk_meta: HashMap<u64, ChunkMetaEntry>`, `doc_index: HashMap<String, Vec<u64>>`, `next_internal_id: u64`) in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T005 [P] Define `ChunkMetaEntry` struct (fields: `document_id`, `chunk_index`, `total_chunks`, `source`, `metadata: HashMap<String, String>`) with `Serialize`/`Deserialize` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T006 [P] Define `StoreManifest` struct (fields: `version: u32`, `bit_width: usize`, `collections: HashMap<String, CollectionManifestEntry>`) with `Serialize`/`Deserialize` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T007 [P] Define `CollectionManifestEntry` struct (fields: `dim: usize`, `n_vectors: usize`) with `Serialize`/`Deserialize` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T008 Implement `generate_internal_id(document_id: &str, chunk_index: usize) -> u64` using `std::collections::hash_map::DefaultHasher` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T009 Implement `l2_normalize(vec: &mut [f32])` helper (zero-norm vectors: keep as-is) in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T010 Implement `map_turbovec_error(err: impl Error) -> VectorStoreError` helper, mapping `AddError::DimMismatch` → `DimensionMismatch` and all others → `BackendError(msg)` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T011 [P] Implement `TurbovecVectorStore::new(bit_width: usize) -> Result<Self, VectorStoreError>` constructor with bit_width validation (2/3/4 only) in `crates/agent_scope_rag/src/turbovec_store.rs`

**Checkpoint**: Foundation ready — user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - 本地高性能向量存储 (Priority: P1) 🎯 MVP

**Goal**: 开发者可创建 `TurbovecVectorStore`，插入向量并执行语义搜索，无需外部数据库

**Independent Test**: 创建实例 → `create_collection` → `insert` 100 条 16 维向量 → `search` 返回 top-10 → 验证分数降序排列，`has_collection` 返回 true

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T012 [P] [US1] Write test `test_create_and_has_collection` — verify collection creation and existence check in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T013 [P] [US1] Write test `test_insert_and_search` — insert 100 vectors, search, verify top-k results sorted descending by score in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T014 [P] [US1] Write test `test_delete_then_search_empty` — insert a document, delete it, verify search returns empty in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T015 [P] [US1] Write test `test_delete_nonexistent_idempotent` — delete non-existent document, verify no error in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T016 [P] [US1] Write test `test_list_documents` — insert chunks for multiple docs, verify DocumentSummary list correct in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T017 [P] [US1] Write test `test_metadata_filter_search` — insert with metadata, search with filter, verify only matching results in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T018 [P] [US1] Write test `test_dimension_mismatch_error` — insert vector with wrong dim, verify `DimensionMismatch` error in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T019 [P] [US1] Write test `test_empty_search_on_empty_collection` — search empty collection, verify empty results (no error) in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T020 [P] [US1] Write test `test_bit_width_validation` — construct with invalid bit_width (0/1/5), verify error in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T021 [P] [US1] Write test `test_concurrent_search` — spawn multiple concurrent searches on same collection, verify all return valid results in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`

### Implementation for User Story 1

- [X] T022 [P] [US1] Implement `VectorStore::has_collection()` — check `collections` map for key in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T023 [P] [US1] Implement `VectorStore::create_collection()` — create new `IdMapIndex` + empty `chunk_meta`/`doc_index`, return error if already exists in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T024 [P] [US1] Implement auto-create logic: `ensure_collection(name, dim)` — used by `insert` when collection doesn't exist yet in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T025 [US1] Implement `VectorStore::insert()` — L2-normalize vectors, generate internal IDs, call `IdMapIndex::add_with_ids()`, store `ChunkMetaEntry`, update `doc_index`; handle auto-create for missing collection in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T026 [US1] Implement `VectorStore::search()` — L2-normalize query, optionally build `allowlist` from `metadata_filter`, call `index.search_with_mask()` via `spawn_blocking`, map results to `VectorSearchResult` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T027 [US1] Implement `metadata_filter` → allowlist conversion: iterate `chunk_meta` entries, collect internal IDs matching all filter conditions into a `Vec<bool>` mask in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T028 [US1] Implement `build_search_result()`: map `(score, internal_id)` from turbovec search to `VectorSearchResult` with `document_id` + rebuilt `Chunk` from `ChunkMetaEntry` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T029 [US1] Implement `VectorStore::delete()` — look up internal IDs from `doc_index`, call `IdMapIndex::remove()` in reverse-ID order, clean up `chunk_meta` and `doc_index` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T030 [US1] Implement `VectorStore::list_documents()` — iterate `chunk_meta`, apply `metadata_filter`, aggregate into `DocumentSummary` by `document_id` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T031 [US1] Add dimension validation in `insert()` and `search()` — verify vector length matches collection dim, return `DimensionMismatch` on mismatch in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T032 [US1] Add empty-records guard in `insert()` — return `Ok(())` immediately for `records.is_empty()` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T033 [US1] Run all US1 tests: `cargo test -p agent_scope_rag turbovec_store_tests --test turbovec_store_tests` — all tests pass

**Checkpoint**: US1 complete — CRUD 操作全部可用、测试通过、可独立验证 MVP

---

## Phase 4: User Story 2 - 索引持久化 (Priority: P2)

**Goal**: 开发者可将 store 保存到磁盘，重启后重新加载恢复全部数据

**Independent Test**: Insert 100 vectors → save → load new instance → search same query → results identical

### Tests for User Story 2

- [X] T034 [P] [US2] Write test `test_save_load_roundtrip` — insert vectors, save, load new instance, verify search results identical and `has_collection`/`list_documents` match in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T035 [P] [US2] Write test `test_save_empty_store` — save empty store, load, verify no collections in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T036 [P] [US2] Write test `test_save_load_append_more` — save → load → insert more → save → load → verify total vector count in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T037 [P] [US2] Write test `test_load_corrupted_manifest_errors` — load from directory with malformed manifest.json, verify error (not panic) in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T038 [P] [US2] Write test `test_save_multiple_collections` — save store with 3 collections, load, verify all 3 available independently in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`

### Implementation for User Story 2

- [X] T039 [P] [US2] Implement `StoreManifest::write(path)` — serialize manifest.json with atomic write (temp → write → fsync → rename) in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T040 [P] [US2] Implement `StoreManifest::read(path)` — deserialize manifest.json, validate version (reject > 1) and field integrity in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T041 [US2] Implement `write_collection_meta(collection_name, chunk_meta)` — serialize `HashMap<u64, ChunkMetaEntry>` to JSON (keys as strings) in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T042 [US2] Implement `read_collection_meta(collection_name)` — deserialize JSON back to `HashMap<u64, ChunkMetaEntry>`, rebuild `doc_index` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T043 [US2] Implement `TurbovecVectorStore::save(path)` — for each collection: write `.tvim` (via `IdMapIndex::write`), write `.meta` (via `write_collection_meta`), write `manifest.json` (via `StoreManifest::write`); all via `spawn_blocking` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T044 [US2] Implement `TurbovecVectorStore::load(path)` — read `manifest.json`, for each collection: load `.tvim` (via `IdMapIndex::load`), load `.meta` (via `read_collection_meta`), verify `index.len() == n_vectors`; all via `spawn_blocking` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T045 [US2] Add integrity check on load: compare `IdMapIndex::len()` with `manifest.n_vectors`, mismatch → `BackendError("corrupted: vector count mismatch")` in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T046 [US2] Run all US2 tests: `cargo test -p agent_scope_rag turbovec_store_tests --test turbovec_store_tests` — all tests pass

**Checkpoint**: US2 complete — 持久化 round-trip 可用、原子写入、完整性验证、US1 测试仍然通过

---

## Phase 5: User Story 3 - KnowledgeBase 集成 (Priority: P3)

**Goal**: 现有 `KnowledgeBase` 使用 `TurbovecVectorStore` 完成端到端 RAG 流程，行为与 `MockVectorStore` 等价

**Independent Test**: `DashScopeEmbeddingModel` + `TurbovecVectorStore` → `KnowledgeBase` → insert chunks + search → 验证返回正确的 chunk 内容

### Tests for User Story 3

- [X] T047 [P] [US3] Write test `test_knowledge_base_with_turbovec_store` — create KB with TurbovecVectorStore, insert chunks via KB, search, verify results match expected document_ids and chunk content in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T048 [P] [US3] Write test `test_calibration_state_tracking` — insert <1000 vectors, verify `calibration_state()` returns `WarmingUp`; insert enough to cross 1000, verify `Fitted` in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`
- [X] T049 [P] [US3] Write test `test_metadata_filter_enforced_by_kb` — KB with `metadata_filter`, insert chunks, verify filter is applied (chunks from other filter values don't appear) in `crates/agent_scope_rag/tests/turbovec_store_tests.rs`

### Implementation for User Story 3

- [X] T050 [US3] Implement `TurbovecVectorStore::calibration_state(collection)` extension method — delegate to `IdMapIndex::calibration_state()`, map to `CalibrationState` enum in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T051 [P] [US3] Define public `CalibrationState` enum (variants: `WarmingUp`, `Fitted`, `Identity`) in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T052 [US3] Ensure `VectorStore::search()` returns `Chunk` with full `content` field populated from `ChunkMetaEntry` data (since content is not stored in turbovec index) in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T053 [US3] Run all US3 tests: `cargo test -p agent_scope_rag turbovec_store_tests --test turbovec_store_tests` — all tests pass
- [X] T054 [US3] Run existing KnowledgeBase tests with TurbovecVectorStore to verify backward compatibility: `cargo test -p agent_scope_rag` — existing tests still pass

**Checkpoint**: US3 complete — KnowledgeBase 集成可用、CalibrationState 可查询、US1/US2 测试仍然通过

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: 文档、代码质量、性能验证

- [X] T055 [P] Add rustdoc documentation to `TurbovecVectorStore` and all public methods in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T056 [P] Add module-level documentation (`//!`) explaining turbovec integration, bit_width tradeoffs, and usage examples in `crates/agent_scope_rag/src/turbovec_store.rs`
- [X] T057 Run `cargo clippy -p agent_scope_rag -- -D warnings` — zero warnings
- [X] T058 Run `cargo fmt -p agent_scope_rag -- --check` — format clean
- [X] T059 Run full test suite: `cargo test -p agent_scope_rag` — all tests pass
- [X] T060 Run `cargo build -p agent_scope_rag` — release build succeeds
- [X] T061 Validate quickstart.md scenarios: execute Scenario 1 (CRUD), Scenario 2 (Persistence), Scenario 3 (KB integration) per `specs/016-turbovec-rag/quickstart.md`
- [X] T062 Run `cargo test` for entire workspace to verify no regressions

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories
- **User Stories (Phase 3–5)**: All depend on Foundational phase completion
  - US1 (P1) → US2 (P2) → US3 (P3) 按优先级顺序执行
  - US2 可以与 US1 并行（不同关注点），但 tests 在 US1 code 之上
  - US3 依赖 US1 稳定（使用 US1 的 search/insert）
- **Polish (Phase 6)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) — No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) — Uses US1's Collection data structures but adds persistence layer independently. Tests can start after US1 implementation is complete
- **User Story 3 (P3)**: Depends on US1 being complete (uses insert/search/delete at runtime) — Cannot fully test without US1

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Structs/helpers before trait method implementations
- Core CRUD before extensions
- Story complete (all tests pass) before moving to next priority

### Parallel Opportunities

- Phase 1: T001 + T002 parallel (different files)
- Phase 2: T005, T006, T007, T011 all [P] (different struct definitions, independent)
- Phase 3 (US1): T012–T021 (10 test tasks) all [P] — write all tests in parallel; T022, T023, T024 [P] — independent implementations
- Phase 4 (US2): T034–T038 (5 test tasks) all [P]; T039, T040 [P]
- Phase 5 (US3): T047–T049 (3 test tasks) all [P]
- Phase 6: T055, T056 [P]

---

## Parallel Example: User Story 1

```bash
# Step 1: Write all US1 tests in parallel
Task: T012 "test_create_and_has_collection in turbovec_store_tests.rs"
Task: T013 "test_insert_and_search in turbovec_store_tests.rs"
Task: T014 "test_delete_then_search_empty in turbovec_store_tests.rs"
Task: T015 "test_delete_nonexistent_idempotent in turbovec_store_tests.rs"
Task: T016 "test_list_documents in turbovec_store_tests.rs"
Task: T017 "test_metadata_filter_search in turbovec_store_tests.rs"
Task: T018 "test_dimension_mismatch_error in turbovec_store_tests.rs"
Task: T019 "test_empty_search_on_empty_collection in turbovec_store_tests.rs"
Task: T020 "test_bit_width_validation in turbovec_store_tests.rs"
Task: T021 "test_concurrent_search in turbovec_store_tests.rs"

# Step 2: Implement independent components in parallel
Task: T022 "has_collection()"
Task: T023 "create_collection()"
Task: T024 "ensure_collection()"

# Step 3: Implement dependent components sequentially
Task: T025 "insert()"  (depends on T023, T024)
Task: T026 "search()"  (depends on T025 for test data)
Task: T029 "delete()"  (depends on T025 for test data)
Task: T030 "list_documents()" (depends on T025 for test data)
```

---

## Parallel Example: User Story 2

```bash
# Step 1: Write all US2 tests in parallel
Task: T034 "test_save_load_roundtrip"
Task: T035 "test_save_empty_store"
Task: T036 "test_save_load_append_more"
Task: T037 "test_load_corrupted_manifest_errors"
Task: T038 "test_save_multiple_collections"

# Step 2: Implement persistence components
Task: T039 "StoreManifest::write()"  (parallel with T040)
Task: T040 "StoreManifest::read()"   (parallel with T039)
Task: T041 "write_collection_meta()"
Task: T042 "read_collection_meta()"

# Step 3: Wire save/load (depends on T039-T042)
Task: T043 "save()"
Task: T044 "load()"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001–T002)
2. Complete Phase 2: Foundational (T003–T011)
3. Complete Phase 3: User Story 1 (T012–T033)
4. **STOP and VALIDATE**: `cargo test -p agent_scope_rag turbovec_store_tests` — all 10 US1 tests pass
5. 此时开发者已可使用 `TurbovecVectorStore` 完成基本的 CRUD 向量存储

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US1 → Test independently → **MVP!** 基本向量 CRUD 可用
3. Add US2 → Test independently → 持久化可用、进程重启后数据不丢失
4. Add US3 → Test independently → KnowledgeBase 端到端集成
5. Polish → Docs, clippy, fmt, full workspace regression check

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together (T001–T011)
2. Once Foundational is done:
   - Developer A: US1 (T012–T033) — core CRUD
   - Developer B: US2 (T034–T046) — persistence (can work on save/load independently, needs US1 Collection types)
   - Developer C: US3 (T047–T054) — KB integration (waits for US1 to stabilize)
3. All converge on Phase 6: Polish

---

## Notes

- [P] tasks = different files or independent sections within the same file, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Tests follow TDD: write test, verify failure, implement, verify pass
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
- turbovec 内部已执行归一化，但我们的 `l2_normalize()` 确保余弦相似度等价性
- `spawn_blocking` 用于所有 turbovec 同步调用（insert/add/search/write/load）
