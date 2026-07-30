# Tasks: Memory System

**Input**: Design documents from `/specs/009-memory-system/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md (design decisions), data-model.md (entities), contracts/ (interface contracts)

**Tests**: Included — unit tests for each module per Constitution §6 (测试驱动兼容性).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `- [X] [ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- `crates/agent_scope_memory/` — new crate for Memory trait and FileMemory
- `crates/agent_scope_agent/src/` — existing crate, modifications for middleware
- `crates/agent_scope_memory/tests/` — integration tests

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create new `agent_scope_memory` crate and register in workspace

- [X] T001 Create `crates/agent_scope_memory/` crate with Cargo.toml (name=`agent_scope_memory`, edition=2024, deps: `agent_scope_message`, `agent_scope_model`, `serde`, `serde_json`, `uuid`, `chrono`, `regex`, `tokio` with fs feature, `async-trait`, `tracing`) and `src/lib.rs` (with `#![deny(unsafe_code)]` and empty module declarations)
- [X] T002 [P] Add `agent_scope_memory` to workspace members in `/Cargo.toml` and add `agent_scope_memory` dependency to `agent_scope_agent/Cargo.toml`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core types that all user stories depend on — error enum, memory entry model, configuration

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T003 [P] Implement `MemoryError` enum in `crates/agent_scope_memory/src/memory_error.rs` — variants: `IoError { path, message }`, `ParseError { filename, message }`, `ValidationError { field, message }`, `NotFound { name }`, `IndexError { message }`, `RetrievalError { reason }`; impl `Display` and `std::error::Error`
- [X] T004 [P] Implement `MemoryType` enum with `User`, `Feedback`, `Project`, `Reference`, `Unknown(String)` (serde untagged) + `MemoryMetadata` struct (mem_type, created_at, updated_at, tags) + `MemoryEntry` struct (name, description, metadata, content) + `MemoryFileHeader` struct (filename, path, description, mem_type, mtime) in `crates/agent_scope_memory/src/memory_entry.rs`; include `MemoryEntry::new()` constructor that auto-sets timestamps
- [X] T005 Implement `MemoryConfig` struct in `crates/agent_scope_memory/src/memory_config.rs` — fields: `memory_dir` (default "Memory"), `max_index_tokens` (default 4000), `retrieval_async` (default true), `retrieval_max_files` (default 200), `retrieval_max_tokens_per_file` (default 2000), `retrieval_max_tokens_per_frontmatter` (default 256), `memory_instructions`, `retrieval_instructions`; `validate()` method checking `max_index_tokens > 0`, `retrieval_max_files > 0`, `retrieval_max_tokens_per_file > 0`; include defaults for instructions texts (matching Python `DEFAULT_MEMORY_INSTRUCTIONS` and `DEFAULT_RETRIEVAL_INSTRUCTIONS` per spec FR-023)
- [X] T006 Update `crates/agent_scope_memory/src/lib.rs` to declare modules: `memory_error`, `memory_entry`, `memory_config` and re-export public types; add `#![deny(unsafe_code)]` at crate level

**Checkpoint**: Foundation ready — all types defined, crate compiles with `cargo build -p agent_scope_memory`

---

## Phase 3: User Story 1 — Save and Retrieve Memories via Trait Interface (Priority: P1) 🎯 MVP

**Goal**: Developer can create a memory store, write entries, read/delete/search them through the `Memory` trait

**Independent Test**: Create a `FileMemory` instance, write a "user-role" entry, read it back, search by content, delete it, verify it's gone. See `spec.md` US1 acceptance scenarios.

### Implementation for User Story 1

- [X] T007 [P] [US1] Implement `frontmatter.rs` in `crates/agent_scope_memory/src/frontmatter.rs` — functions: `parse_frontmatter_fields(content: &str) -> HashMap<String, String>` (regex: match `---\n...\n---` block, extract `key: value` lines), `serialize_frontmatter(entry: &MemoryEntry) -> String` (generate YAML-like frontmatter string per spec FR-011); handle malformed frontmatter gracefully (return empty HashMap on parse failure)
- [X] T008 [P] [US1] Implement `Backend` trait in `crates/agent_scope_memory/src/backend.rs` — 9 async/sync methods per `contracts/backend-trait.md`: `read_file`, `write_file`, `delete_file`, `file_exists`, `list_dir`, `join_path`, `stat_mtime`, `normpath`, `isabs`; implement `LocalBackend` using `tokio::fs` (with `spawn_blocking` for `create_dir_all` in `write_file`)
- [X] T009 [US1] Implement `Memory` trait in `crates/agent_scope_memory/src/memory_trait.rs` — 7 async methods per `contracts/memory-trait.md`: `write()`, `read()`, `delete()`, `list()`, `search()`, `get_index_content()`, `retrieve_relevant()`; all `#[async_trait]` with `Send + Sync` bounds
- [X] T010 [US1] Implement `FileMemory` struct in `crates/agent_scope_memory/src/file_memory.rs` — fields: `backend: Arc<dyn Backend>`, `config: MemoryConfig`, `index_lock: tokio::sync::Mutex<()>`; implement `Memory` trait: `write()` (serialize entry → write .md file via backend), `read()` (read .md → parse frontmatter+content), `delete()` (remove .md + update index), `list()` (list_dir → parse headers per `retrieval_max_files` cap), `search()` (scan files → substring match on content + description with optional type filter); leave `get_index_content()` and `retrieve_relevant()` as `todo!()` stubs for US2/US3
- [X] T011 [US1] Wire `get_index_content()` in `FileMemory` — read `MEMORY.md` via backend, return as string (no truncation yet, stub for US2); wire index update in `write()` (append/update one line in `MEMORY.md`) and `delete()` (remove one line from `MEMORY.md`) with `index_lock` guarding concurrent writes
- [X] T012 [US1] Update `crates/agent_scope_memory/src/lib.rs` to add/export: `frontmatter`, `backend`, `memory_trait`, `file_memory` modules and their public types

### Tests for User Story 1

- [X] T013 [P] [US1] Write unit tests for `frontmatter.rs` in `crates/agent_scope_memory/src/frontmatter.rs` (#[cfg(test)] module) — test: valid frontmatter parsing, missing delimiters, empty fields, serialize roundtrip, multi-line content after frontmatter
- [X] T014 [P] [US1] Write unit tests for `backend.rs` in `crates/agent_scope_memory/src/backend.rs` (#[cfg(test)] module) — test: LocalBackend file write→read→exists→delete cycle, list_dir with subdirs, stat_mtime, join_path, normpath
- [X] T015 [P] [US1] Write integration test for `FileMemory` CRUD in `crates/agent_scope_memory/tests/file_memory_tests.rs` — test: write→read roundtrip (all 4 memory types), upsert (write same name twice = update), delete→read returns None, delete idempotent (no error), search by content substring, search with type filter, list returns headers (verify no content loaded), empty directory returns empty list
- [X] T016 [US1] Write integration test for `MemoryEntry` edge cases in `crates/agent_scope_memory/tests/file_memory_tests.rs` — test: write with empty content (allowed), write with empty name (rejected with ValidationError), write with empty description (rejected with ValidationError), name with special chars (slug validation), very long content (no truncation on write)

**Checkpoint**: US1 complete — Memory trait + FileMemory with CRUD operations fully functional and testable. `cargo test -p agent_scope_memory` passes all T013-T016 tests.

---

## Phase 4: User Story 2 — Memory Index Management (Priority: P2)

**Goal**: `MEMORY.md` index generates/updates automatically, respects `max_index_tokens` truncation

**Independent Test**: Write 5 entries, verify `MEMORY.md` has 5 one-line bullet points. Set small `max_index_tokens`, verify truncation notice appended. Delete entry, verify its index line removed. See `spec.md` US2 acceptance scenarios.

### Implementation for User Story 2

- [X] T017 [US2] Implement `index.rs` in `crates/agent_scope_memory/src/index.rs` — functions: `read_index(backend: &dyn Backend, path: &str) -> Result<String, MemoryError>` (read MEMORY.md file), `write_index_line(backend: &dyn Backend, path: &str, filename: &str, description: &str) -> Result<(), MemoryError>` (upsert a `- [filename](filename.md) — description` line), `remove_index_line(backend: &dyn Backend, path: &str, filename: &str) -> Result<(), MemoryError>` (remove matching line), `truncate_index(content: &str, max_tokens: usize, model: &dyn ChatModel) -> String` (scan lines, accumulate token count via `model.count_tokens`, truncate when budget exceeded, append `<<<TRUNCATED>>>` notice)
- [X] T018 [US2] Replace stub `get_index_content()` in `FileMemory` with full implementation using `index::read_index` + `index::truncate_index`; refactor `write()` and `delete()` to use `index::write_index_line` / `index::remove_index_line` instead of inline index manipulation
- [X] T019 [US2] Update `crates/agent_scope_memory/src/lib.rs` to add `index` module and re-export public items

### Tests for User Story 2

- [X] T020 [P] [US2] Write unit tests for `index.rs` in `crates/agent_scope_memory/src/index.rs` (#[cfg(test)] module) — test: write line to empty index, update existing line (upsert), delete existing line, delete non-existent line (no-op), truncation with small max_tokens, truncation notice format, empty index truncation
- [X] T021 [US2] Write index integration tests in `crates/agent_scope_memory/tests/index_tests.rs` — test: 5 entries → index has 5 lines, delete 1 entry → index has 4 lines + deleted entry absent, 100 entries → verify index generation < 500ms, truncation with `max_index_tokens=500` (small) → truncated index with notice, truncation with `max_index_tokens=100000` (large) → no truncation, manual edit of `MEMORY.md` → `get_index_content()` reflects disk state

**Checkpoint**: US2 complete — Index management fully functional. `cargo test -p agent_scope_memory` pasess all US1+US2 tests.

---

## Phase 5: User Story 3 — Relevance-Based Memory Retrieval (Priority: P3)

**Goal**: Agent auto-identifies relevant memories via LLM structured output call

**Independent Test**: Create 20 entries across 4 types. Query "auth bug" → returns only auth-related memories. Query "weather" → returns empty. See `spec.md` US3 acceptance scenarios.

### Implementation for User Story 3

- [X] T022 [US3] Implement `retrieval.rs` in `crates/agent_scope_memory/src/retrieval.rs` — struct `MemorySelection { selected_files: Vec<String> }` (Serialize + Deserialize for structured output); function `retrieve_relevant_files(memory: &FileMemory, query: &str, model: &Arc<dyn ChatModel>, max_results: usize) -> Result<Option<String>, MemoryError>` — steps: (1) call `list()` to get all headers, (2) format manifest string with filenames + descriptions + types, (3) call `model.generate_structured_output()` with selection prompt and `MemorySelection` schema, (4) filter hallucinated filenames (only keep those that exist), (5) read each selected file, truncate to `retrieval_max_tokens_per_file` using `model.count_tokens`, (6) format with age headers ("saved today/yesterday/N days ago"), (7) return combined string or None if nothing selected
- [X] T023 [US3] Replace stub `retrieve_relevant()` in `FileMemory` with call to `retrieval::retrieve_relevant_files()`; pass `config.retrieval_instructions` and `config.retrieval_max_tokens_per_file` through
- [X] T024 [US3] Update `crates/agent_scope_memory/src/lib.rs` to add `retrieval` module and `MemorySelection` re-export

### Tests for User Story 3

- [X] T025 [P] [US3] Write unit tests for retrieval logic in `crates/agent_scope_memory/tests/retrieval_tests.rs` — test: mock model returns valid filenames → result contains file content, mock model returns empty list → result is None, mock model returns hallucinated filename → filtered out (no crash, returned None or partial), mock model returns more than `max_results` → capped, mock model call fails (network error) → returns None (not propagated error per spec edge case), truncation: file content exceeding `retrieval_max_tokens_per_file` → truncated with token budget respected
- [X] T026 [US3] Write semantic retrieval validation test in `crates/agent_scope_memory/tests/retrieval_tests.rs` — test: 20 entries (mix of auth, deploy, user, feedback types), query "fix authentication bug" with mock model that selects only auth-related filenames → result contains auth entries, not deploy entries

**Checkpoint**: US3 complete — Relevance retrieval functional with mock model. `cargo test -p agent_scope_memory` passes all US1+US2+US3 tests.

---

## Phase 6: User Story 4 — Integration with Agent Memory Lifecycle (Priority: P4)

**Goal**: MemoryMiddleware injects memory instructions + index into system prompt, performs async retrieval during reply

**Independent Test**: Register MemoryMiddleware with ReActAgent. Send msg. Verify: system prompt has memory instructions + MEMORY.md, retrieval task runs concurrently, HintBlock injected with retrieved content. See `spec.md` US4 acceptance scenarios.

### Implementation for User Story 4

- [X] T027 [US4] Add `on_system_prompt` hook to `Middleware` trait in `crates/agent_scope_agent/src/middleware.rs` — signature: `async fn on_system_prompt(&self, _agent_name: &str, _current_prompt: &mut String) -> Result<(), AgentError> { Ok(()) }`; default no-op for backward compatibility
- [X] T028 [US4] Implement `MemoryMiddleware` struct in `crates/agent_scope_agent/src/memory_middleware.rs` — fields: `memory: Arc<dyn Memory>`, `config: MemoryConfig`, `retrieval_handle: Mutex<Option<tokio::task::JoinHandle<Result<Option<String>, MemoryError>>>>`, `cached_user_input: Mutex<Option<String>>`; constructor: `new(memory: Arc<dyn Memory>, config: MemoryConfig) -> Self`; implement `Middleware` trait: (1) `on_system_prompt` — read index via `memory.get_index_content()`, inject instructions + index into prompt; (2) `pre_reply` — if `retrieval_async=true`, extract user text from input msgs, spawn `tokio::spawn` with cloned memory+model, store handle; (3) `pre_reasoning` — poll handle, if done and has result, inject `ContentBlock::Hint(HintBlock::new(HintContent::Text(result)))` into messages
- [X] T029 [US4] Wire `on_system_prompt` hook into `ReActAgent::reply()` in `crates/agent_scope_agent/src/react_agent.rs` — call `mw.on_system_prompt(agent_name, &mut system_prompt).await?` AFTER `pre_reply` and BEFORE the first model call in the react loop; pass the agent's model for retrieval in `pre_reply`
- [X] T030 [US4] Add `pub mod memory_middleware;` to `crates/agent_scope_agent/src/lib.rs` and re-export `MemoryMiddleware`; add `agent_scope_memory` to `agent_scope_agent` dependencies

### Tests for User Story 4

- [X] T031 [P] [US4] Write unit tests for `MemoryMiddleware` in `crates/agent_scope_agent/src/memory_middleware.rs` (#[cfg(test)] module) — test: `on_system_prompt` appends memory instructions to empty prompt, `on_system_prompt` appends index content from a pre-populated memory store, `pre_reply` with `retrieval_async=false` is no-op, `pre_reply` with `retrieval_async=true` spawns task and stores handle, `pre_reasoning` polls finished task and injects HintBlock, `pre_reasoning` with unfinished task leaves messages unchanged, retrieval task failure → no HintBlock (silent skip)
- [X] T032 [US4] Write integration test for `ReActAgent` + `MemoryMiddleware` in `crates/agent_scope_agent/tests/memory_middleware_tests.rs` (create `tests/` dir if not exists) — test: create agent with MemoryMiddleware (FileMemory with temp dir), verify system prompt contains memory instructions after first reply, verify `on_system_prompt` is called in correct order (after pre_reply, before model call)

**Checkpoint**: US4 complete — MemoryMiddleware integrated with agent lifecycle. `cargo test -p agent_scope_agent` passes all existing + new tests.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final integration, documentation, performance validation

- [X] T033 [P] Add tracing spans to all `FileMemory` operations in `crates/agent_scope_memory/src/file_memory.rs` — `info` level for write/delete, `debug` level for read/list/search, `warn` level for retrieval failures per Constitution §14
- [X] T034 [P] Add tracing spans to `MemoryMiddleware` hook calls in `crates/agent_scope_agent/src/memory_middleware.rs` — `debug` level for hook entry/exit, `warn` for retrieval task failure
- [X] T035 Run `cargo clippy --all-targets -p agent_scope_memory` and `cargo clippy --all-targets -p agent_scope_agent` — fix all warnings
- [X] T036 Run `cargo fmt` on all changed files
- [X] T037 Run quickstart.md validation scenarios — verify Scenario 1 (CRUD), Scenario 2 (index), Scenario 3 (retrieval), Scenario 4 (agent integration) all produce expected outcomes
- [X] T038 [P] Verify `cargo build` passes for entire workspace (`cargo build --workspace`)
- [X] T039 [P] Verify `cargo test --workspace` passes all tests (no regressions in other crates)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 (crate exists) — BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Phase 2 — foundational types needed
- **US2 (Phase 4)**: Depends on US1 (needs FileMemory CRUD to be working first) — index management builds on existing write/read/delete
- **US3 (Phase 5)**: Depends on US1 (Memory trait + list()) and US2 (index/listing infrastructure) — needs manifest generation from list()
- **US4 (Phase 6)**: Depends on US1 (Memory trait as Arc<dyn Memory>), US3 (retrieve_relevant), and existing Middleware trait
- **Polish (Phase 7)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: Can start after Foundational — No dependencies on other stories
- **US2 (P2)**: Depends on US1 — uses FileMemory write/read/delete for index maintenance
- **US3 (P3)**: Depends on US1 (list()) and US2 (for index-based metadata) — can use list() which is in Memory trait from US1
- **US4 (P4)**: Depends on US1 (Memory trait), US3 (retrieve_relevant), and existing Middleware + ReActAgent

### Within Each Phase

- Foundational: T003, T004, T005 can run in parallel (different files)
- US1: T007, T008 can run in parallel; T009 after T004 (uses MemoryEntry); T010 after T007, T008, T009
- US2: T017 independent (new file); T018 after T017
- US3: T022 independent (new file); T023 after T022
- US4: T027, T028 can run in parallel; T029 after T027, T028
- Tests within each phase can run in parallel (different test files)

### Parallel Opportunities

- All Foundational tasks T003-T005 in parallel
- T007 (frontmatter) and T008 (backend) in parallel within US1
- All test tasks within a phase in parallel
- T033 and T034 in parallel (tracing in different crates)
- T038 and T039 in parallel

---

## Parallel Example: Phase 2 (Foundational)

```bash
# Launch all foundational type implementations together:
Task: "T003: Implement MemoryError enum in crates/agent_scope_memory/src/memory_error.rs"
Task: "T004: Implement MemoryType, MemoryMetadata, MemoryEntry in crates/agent_scope_memory/src/memory_entry.rs"
Task: "T005: Implement MemoryConfig in crates/agent_scope_memory/src/memory_config.rs"
```

## Parallel Example: US1 Tests

```bash
# Launch all US1 test files together:
Task: "T013: Unit tests for frontmatter.rs"
Task: "T014: Unit tests for backend.rs"
Task: "T015: Integration test for FileMemory CRUD"
Task: "T016: Integration test for MemoryEntry edge cases"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T002)
2. Complete Phase 2: Foundational (T003-T006)
3. Complete Phase 3: US1 (T007-T016)
4. **STOP and VALIDATE**: `cargo test -p agent_scope_memory` passes all US1 tests
5. Deploy/demo — developers can already use `Memory` trait + `FileMemory` for persistent memory storage

### Incremental Delivery

1. Setup + Foundational → types ready
2. Add US1 → CRUD operations + search → **MVP!** 🎯
3. Add US2 → index management → scalable memory stores
4. Add US3 → relevance retrieval → "agentic" memory
5. Add US4 → agent integration → end-to-end agent memory lifecycle
6. Polish → traces, lint, validation

### Parallel Team Strategy

With 2 developers:

1. Both complete Setup + Foundational together (T001-T006)
2. Once Foundational is done:
   - Developer A: US1 (T007-T016)
   - Developer B: (waits — US2-4 depend on US1)
3. Once US1 is done:
   - Developer A: US2 (T017-T021)
   - Developer B: US3 (T022-T026) — can start in parallel with US2 (both depend on US1)
4. Once US3 is done:
   - Developer A: US4 (T027-T032)
   - Developer B: Polish tasks (T033-T039)

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability (US1-US4)
- Each user story should be independently completable and testable
- US1 is the MVP — it alone delivers a working Memory trait + FileMemory
- US2 builds on US1's FileMemory; US3 builds on US2's list/index infrastructure
- US4 bridges `agent_scope_memory` and `agent_scope_agent` — requires both working
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- All `#[cfg(test)]` modules follow the existing pattern (tests in same file as implementation for unit tests, separate `tests/` directory for integration tests)
- `on_system_prompt` hook addition to `Middleware` trait is backward-compatible (default no-op)
