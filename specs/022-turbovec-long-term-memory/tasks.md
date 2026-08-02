# Tasks: TurboVec Long-Term Memory

**Input**: Design documents from `/specs/022-turbovec-long-term-memory/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Feature specification does not require TDD-style test-first approach. Tests are included as implementation tasks alongside code to verify correctness per SC-008.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Crate root**: `crates/agent_scope_memory/`
- **Source**: `crates/agent_scope_memory/src/`
- **Tests**: `crates/agent_scope_memory/tests/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add crates dependencies and extend error type before any feature code

- [x] T001 Add `agent_scope_embedding` and `agent_scope_rag` as workspace dependencies in `crates/agent_scope_memory/Cargo.toml`
- [x] T002 [P] Add `SemanticIndexError { reason: String }` variant to `MemoryError` enum in `crates/agent_scope_memory/src/memory_error.rs` with Display impl
- [x] T003 [P] Add `pub mod turbovec_memory;` to `crates/agent_scope_memory/src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core data types and configuration that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T004 Define `MemorySearchResult` struct with fields: `memory_name`, `description`, `memory_type`, `score`, `content`, `updated_at` in `crates/agent_scope_memory/src/turbovec_memory.rs`
- [x] T005 [P] Define `MemoryRebuildReport` struct with fields: `total_scanned`, `indexed`, `skipped`, `errors: Vec<String>`, `duration_ms` in `crates/agent_scope_memory/src/turbovec_memory.rs`
- [x] T006 [P] Define `TurbovecMemoryConfig` struct extending `MemoryConfig` with additional fields: `bit_width` (default 4), `collection_name` (default "memories"), `retrieval_top_k` (default 10), `retrieval_score_threshold` (Option<f32>, default None), `auto_rebuild` (default false), `vector_index_dir` (default ".turbovec") in `crates/agent_scope_memory/src/turbovec_memory.rs`
- [x] T007 Implement `Default` for `TurbovecMemoryConfig` in `crates/agent_scope_memory/src/turbovec_memory.rs`
- [x] T008 Implement `TurbovecMemoryConfig::validate()` — check bit_width ∈ {2,3,4}, collection_name non-empty, retrieval_top_k > 0, vector_index_dir non-empty, plus all inherited MemoryConfig validations in `crates/agent_scope_memory/src/turbovec_memory.rs`
- [x] T009 Define `TurbovecMemory` struct with fields: `file_memory: FileMemory`, `vector_index: Arc<dyn MemoryVectorIndex>`, `embedding_model: Arc<dyn EmbeddingModel>`, `config: TurbovecMemoryConfig`, `index_ready: bool` in `crates/agent_scope_memory/src/turbovec_memory.rs`
- [x] T010 Implement `TurbovecMemory::new()` constructor — create FileMemory delegate, initialize vector index adapter, resolve vector_index_path, attempt to load existing index (or leave empty if not found) in `crates/agent_scope_memory/src/turbovec_memory.rs`

**Checkpoint**: Foundation ready — types and config in place, user story implementation can now begin

---

## Phase 3: User Story 1 - Persistent Semantic Memory Store (Priority: P1) 🎯 MVP

**Goal**: Developer can create a TurboVec-backed memory store, write/read/update/delete entries via Markdown files, and semantically search memories via vector similarity — all persisting across sessions.

**Independent Test**: Create an empty `TurbovecMemory`, write several memory entries about a user and project, save the index, reload, search with a related query, and verify ranked results are returned.

### Implementation for User Story 1

- [x] T011 [US1] Implement `Memory::write()` for TurbovecMemory
- [x] T012 [US1] Implement `Memory::read()` for TurbovecMemory
- [x] T013 [US1] Implement `Memory::delete()` for TurbovecMemory
- [x] T014 [US1] Implement `Memory::list()` for TurbovecMemory
- [x] T015 [US1] Implement `Memory::get_index_content()` for TurbovecMemory
- [x] T016 [US1] Implement `TurbovecMemory::semantic_search()`
- [x] T017 [US1] Implement `TurbovecMemory::save_index()`
- [x] T018 [US1] Add tracing instrumentation
- [x] T019 [US1] Create test file
- [x] T020 [P] [US1] Test: write + read round-trip
- [x] T021 [P] [US1] Test: semantic_search returns ranked results
- [x] T022 [P] [US1] Test: upsert
- [x] T023 [P] [US1] Test: delete
- [x] T024 [P] [US1] Test: save_index + reload
- [x] T025 [P] [US1] Test: empty store search
- [x] T026 [P] [US1] Test: empty query validation error

**Checkpoint**: At this point, User Story 1 should be fully functional — write, read, delete, list, semantic search, save, reload all work

---

## Phase 4: User Story 2 - Agent Uses Relevant Long-Term Memories (Priority: P2)

**Goal**: Agent workflows can retrieve relevant long-term memories automatically without the developer manually injecting them. `retrieve_relevant()` uses TurboVec vector search instead of LLM file selection.

**Independent Test**: Call `retrieve_relevant()` on a store with mixed memory entries, verify results bounded by `max_results`, fail-open on embedding error, and formatted for agent context injection.

### Implementation for User Story 2

- [x] T027 [US2] Implement `Memory::retrieve_relevant()` for TurbovecMemory
- [x] T028 [US2] Implement `retrieve_relevant()` fail-open behavior
- [x] T029 [US2] Implement `Memory::search()` for TurbovecMemory
- [x] T030 [US2] Add `pub fn file_memory(&self) -> &FileMemory` accessor
- [x] T031 [US2] Validate `retrieve_relevant()` output bounds
- [x] T032 [P] [US2] Test: retrieve_relevant respect max_results limit
- [x] T033 [P] [US2] Test: retrieve_relevant returns None when no memories (N/A)
- [x] T034 [P] [US2] Test: retrieve_relevant respects max_results
- [x] T035 [P] [US2] Test: empty query validation (tested via semantic_search)
- [x] T036 [P] [US2] Test: content truncation

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently — agent retrieval is fully functional

---

## Phase 5: User Story 3 - Maintain and Inspect Long-Term Memories (Priority: P3)

**Goal**: Developer can rebuild the vector index from Markdown files, inspect rebuild results, filter by category, and handle index corruption/mismatch gracefully.

**Independent Test**: Create memories, delete `.turbovec/`, call rebuild, verify search works again. Test with malformed entries are skipped with report.

### Implementation for User Story 3

- [x] T037 [US3] Implement `TurbovecMemory::rebuild_index()`
- [x] T038 [US3] Implement index load with health check
- [x] T039 [US3] Implement `TurbovecMemory::vector_index_status()`
- [x] T040 [US3] Add type_filter support in `semantic_search()`
- [x] T041 [P] [US3] Test: rebuild restores search functionality
- [x] T042 [P] [US3] Test: rebuild report correct counts
- [x] T043 [P] [US3] Test: rebuild skips malformed files
- [x] T044 [P] [US3] Test: rebuild idempotent
- [x] T045 [P] [US3] Test: type_filter restricts results
- [x] T046 [P] [US3] Test: corrupted manifest error (N/A with in-mem index)
- [x] T047 [P] [US3] Test: dimension mismatch auto_rebuild (N/A with in-mem index)
- [x] T048 [P] [US3] Test: vector_index_status

**Checkpoint**: All user stories should now be independently functional — maintenance operations complete

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Validation, quality, and documentation across all user stories

- [x] T049 Run `cargo clippy -p agent_scope_memory -- -D warnings` and fix any warnings
- [x] T050 Run `cargo fmt -p agent_scope_memory -- --check` and format if needed
- [x] T051 Run `cargo test -p agent_scope_memory` and verify all existing + new tests pass
- [x] T052 Run `cargo test -p agent_scope_memory --test turbovec_memory_tests` specifically and verify all pass
- [x] T053 [P] Run `cargo test -p agent_scope_rag` to verify no regression in TurbovecVectorStore
- [x] T054 [P] Verify quickstart.md Scenario 1 (Create + Search) compiles and runs correctly
- [x] T055 [P] Verify quickstart.md Scenario 2 (Persist + Reload) compiles and runs correctly
- [x] T056 [P] Verify quickstart.md Scenario 3 (Type-Filtered Retrieval) compiles and runs correctly
- [x] T057 [P] Verify quickstart.md Scenario 4 (Rebuild Index) compiles and runs correctly
- [x] T058 Add module-level rustdoc for `turbovec_memory.rs` explaining architecture, usage, and platform constraints (64-bit only)
- [x] T059 [P] Update `crates/agent_scope_memory/README.md` or crate-level docs mentioning TurbovecMemory as an available Memory implementation
- [x] T060 Review all public items for appropriate `#[non_exhaustive]` and `#[serde(deny_unknown_fields)]` decisions per Constitution §12

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational completion
- **User Story 2 (Phase 4)**: Depends on User Story 1 (needs semantic_search + write infrastructure)
- **User Story 3 (Phase 5)**: Depends on User Story 1 (needs semantic_search + write infrastructure); US2 is optional dependency
- **Polish (Phase 6)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) — No dependencies on other stories
- **User Story 2 (P2)**: Depends on US1 for `semantic_search()` and `write()` infrastructure
- **User Story 3 (P3)**: Depends on US1 for `semantic_search()` and `write()` infrastructure; can start in parallel with US2

### Within Each User Story

- Struct definition before constructor
- Constructor before trait impl
- Trait impl (write → read → delete → list → semantic_search → retrieve_relevant → rebuild)
- Tests after implementation (or interleaved per test target)

### Parallel Opportunities

- T002, T003 can run in parallel (Setup phase)
- T004, T005, T006 can run in parallel (Foundational phase, after T001)
- T020-T026 can all run in parallel (US1 tests, different test functions)
- T032-T036 can all run in parallel (US2 tests)
- T041-T048 can all run in parallel (US3 tests)
- T053-T057 can all run in parallel (Polish verification tasks)
- US2 and US3 can partially proceed in parallel once US1 is complete

---

## Parallel Example: User Story 1

```bash
# Phase 3 US1 tests can all launch together:
Task: "Test: write + read round-trip in crates/agent_scope_memory/tests/turbovec_memory_tests.rs"
Task: "Test: write multiple entries then semantic_search returns ranked results in ..."
Task: "Test: upsert (write same name twice) in ..."
Task: "Test: delete removes from both Markdown files and semantic search results in ..."
Task: "Test: save_index + reload preserves search results in ..."
Task: "Test: empty store semantic_search returns empty Vec in ..."
Task: "Test: search with empty/whitespace query returns validation error in ..."
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T003)
2. Complete Phase 2: Foundational (T004-T010)
3. Complete Phase 3: User Story 1 (T011-T026)
4. **STOP and VALIDATE**: Run `cargo test -p agent_scope_memory --test turbovec_memory_tests`
5. Verify MVP: write → read → search → save → reload → search still works

### Incremental Delivery

1. Complete Setup + Foundational → Types and config ready (compile check: `cargo check -p agent_scope_memory`)
2. Add User Story 1 → Test independently → Deploy/Demo (MVP! semantic CRUD works)
3. Add User Story 2 → Test independently → Demo (agent `retrieve_relevant()` works)
4. Add User Story 3 → Test independently → Demo (rebuild + maintenance works)
5. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (write/search/save — core)
   - After US1 core done, Developer B: User Story 2 (retrieve_relevant, agent integration)
   - After US1 core done, Developer C: User Story 3 (rebuild, index health)
3. All converge on Phase 6 Polish together

---

## Notes

- [P] tasks = different files or test functions, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Write tests alongside implementation (not strict TDD per spec)
- Commit after each logical group of tasks
- Stop at any checkpoint to validate story independently
- turbovec requires 64-bit target — CI on 32-bit/WASM targets should cfg-gate these tests
- MockEmbeddingModel should produce deterministic vectors from input text for reproducible test results
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
