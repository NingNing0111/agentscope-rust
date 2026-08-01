# Tasks: Integration API Tests (Examples)

**Input**: Design documents from `/specs/015-integration-api-tests/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Not applicable — these examples ARE the integration tests. Each example binary produces pass/fail output.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- **Examples**: `examples/` at repository root
- **Cargo config**: `Cargo.toml` at repository root

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Extend shared helpers and register new example binaries in Cargo.toml

- [X] T001 Extend `examples/common.rs` to add `create_memory_agent()` factory — creates FileMemory with tempdir, wraps in MemoryMiddleware, builds ReActAgent with memory config
- [X] T002 [P] Extend `examples/common.rs` to add `create_session_store()` and `SessionTestHarness` helpers — InMemorySessionStore creation, save/load/close wrappers with error handling
- [X] T003 [P] Extend `examples/common.rs` to add `create_rag_agent()` factory — creates DashScopeEmbeddingModel + in-memory VectorStore + KnowledgeBase + RAGMiddleware + ReActAgent
- [X] T004 [P] Add shared `TestResult` struct and `print_result()` / `print_summary()` utilities in `examples/common.rs` — reusable pass/fail output with duration, used by all examples
- [X] T005 Register 4 new examples in `Cargo.toml`: `[[example]]` entries for `memory_test`, `session_test`, `rag_test`, `streaming_tool_test` with correct paths

**Checkpoint**: Shared infrastructure ready — all user stories can begin in parallel

---

## Phase 2: User Story 1 - Memory Integration E2E (Priority: P1) 🎯 MVP

**Goal**: Verify Memory system (FileMemory + MemoryMiddleware) works with real DashScope API — write, search, and retrieval-augmented reasoning.

**Independent Test**: Run `cargo run --example memory_test -- --api-key sk-xxx` and observe 3 tests passing with Agent responses that reference stored memories.

### Implementation for User Story 1

- [X] T006 [US1] Create `examples/memory_test.rs` — CLI scaffold with clap (`--api-key`, `--model`, `--keep-dir`), header banner, tempdir setup
- [X] T007 [US1] Implement "Write Memory" test in `examples/memory_test.rs` — agent stores user preference, verify MemoryMiddleware includes it in system prompt via on_system_prompt hook
- [X] T008 [US1] Implement "Search Memory" test in `examples/memory_test.rs` — query stored memory by keyword, verify agent's response references the stored entry content
- [X] T009 [US1] Implement "Memory Reasoning" test in `examples/memory_test.rs` — multi-turn conversation where agent must use stored memory to answer a contextual question
- [X] T010 [US1] Add error handling and graceful degradation in `examples/memory_test.rs` — handle API key invalid, network timeout, model errors with descriptive messages

**Checkpoint**: Memory integration tests independently functional — write, search, reasoning all verified

---

## Phase 3: User Story 2 - Session Persistence E2E (Priority: P1)

**Goal**: Verify Session save/load round-trip with InMemorySessionStore preserves conversation history and AgentState.

**Independent Test**: Run `cargo run --example session_test -- --api-key sk-xxx` and observe 3 tests passing with preserved context after load.

### Implementation for User Story 2

- [X] T011 [US2] Create `examples/session_test.rs` — CLI scaffold with clap (`--api-key`, `--model`), header banner
- [X] T012 [US2] Implement "Save/Load Roundtrip" test in `examples/session_test.rs` — create session with 2-turn conversation, save to InMemorySessionStore, load, verify message count preserved
- [X] T013 [US2] Implement "Context Consistency" test in `examples/session_test.rs` — load saved session, ask agent about fact from prior conversation, verify answer references prior fact
- [X] T014 [US2] Implement "Close & Cleanup" test in `examples/session_test.rs` — close session, verify status is Closed, delete from store, verify NotFound on load
- [X] T015 [US2] Add error handling in `examples/session_test.rs` — handle Closed session operations, SerializationError, NotFound gracefully

**Checkpoint**: Session persistence tests independently functional — save, load, verify, cleanup all pass

---

## Phase 4: User Story 3 - RAG Pipeline E2E (Priority: P2)

**Goal**: Verify RAG pipeline (embedding → vector store → KnowledgeBase → RAGMiddleware → agent) with real DashScope embedding API.

**Independent Test**: Run `cargo run --example rag_test -- --api-key sk-xxx` and observe 3 tests passing with grounded answers.

### Implementation for User Story 3

- [X] T016 [US3] Implement in-memory `MockVectorStore` in `examples/rag_test.rs` (or shared module) — implement VectorStore trait with HashMap-backed storage for collections and cosine similarity search (reference `crates/agent_scope_rag/tests/vector_store_mock.rs`)
- [X] T017 [US3] Create `examples/rag_test.rs` — CLI scaffold with clap (`--api-key`, `--model`, `--embedding-model`, `--embedding-dims`), header banner
- [X] T018 [US3] Implement "Ingest Document" test in `examples/rag_test.rs` — create a synthetic document with known facts, index into KnowledgeBase via DashScopeEmbeddingModel, verify chunk count after ingest
- [X] T019 [US3] Implement "Grounded Query" test in `examples/rag_test.rs` — ask agent about facts from indexed document, verify agent's answer contains facts from the document (not hallucinated)
- [X] T020 [US3] Implement "Empty KB Query" test in `examples/rag_test.rs` — ask question with empty KnowledgeBase (no documents), verify agent responds normally without RAG errors or panics
- [X] T021 [US3] Add error handling in `examples/rag_test.rs` — handle embedding API failures, empty chunk sets, model API errors gracefully

**Checkpoint**: RAG pipeline tests independently functional — ingest, grounded query, empty KB all verified

---

## Phase 5: User Story 4 - Streaming Tool-Call Round-Trip (Priority: P2)

**Goal**: Verify complete streaming tool-call event lifecycle with real API — start → delta(s) → end for both tool calls and tool results, event pairing, answer correctness.

**Independent Test**: Run `cargo run --example streaming_tool_test -- --api-key sk-xxx` and observe event counts correct and answer correct.

### Implementation for User Story 4

- [X] T022 [US4] Create `examples/streaming_tool_test.rs` — CLI scaffold with clap (`--api-key`, `--model`), header banner, EventTrace struct
- [X] T023 [US4] Implement "Single Tool Call" test in `examples/streaming_tool_test.rs` — ask agent "Calculate 3.14 * 2.718" with calculator tool, count ToolCallStart/End and ToolResultStart/End (must be 1 each), verify answer ≈ 8.53452
- [X] T024 [US4] Implement "Multi-Tool Call" test in `examples/streaming_tool_test.rs` — ask agent a two-step math question, verify 2 complete tool-call cycles and correct final answer
- [X] T025 [US4] Implement EventTrace validation logic in `examples/streaming_tool_test.rs` — verify Start count == End count for both tool calls and tool results, verify replies have at least one TextBlock, verify ReplyStart precedes ReplyEnd
- [X] T026 [US4] Add error handling in `examples/streaming_tool_test.rs` — handle streaming errors (AlreadyStreaming, model errors, tool errors) gracefully

**Checkpoint**: Streaming tool-call tests independently functional — event lifecycle validated, both single and multi-tool verified

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final validation and cleanup across all examples

- [X] T027 [P] Verify `examples/memory_test.rs` compiles with `cargo build --example memory_test` and passes `cargo clippy` with no warnings
- [X] T028 [P] Verify `examples/session_test.rs` compiles with `cargo build --example session_test` and passes `cargo clippy` with no warnings
- [X] T029 [P] Verify `examples/rag_test.rs` compiles with `cargo build --example rag_test` and passes `cargo clippy` with no warnings
- [X] T030 [P] Verify `examples/streaming_tool_test.rs` compiles with `cargo build --example streaming_tool_test` and passes `cargo clippy` with no warnings
- [X] T031 Verify `cargo fmt` passes on all changed files (`examples/common.rs`, `examples/memory_test.rs`, `examples/session_test.rs`, `examples/rag_test.rs`, `examples/streaming_tool_test.rs`, `Cargo.toml`)
- [X] T032 Run quickstart.md validation — execute all 4 examples with a valid API key and confirm pass/fail output matches contracts

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **US1 Memory (Phase 2)**: Depends on T001 (create_memory_agent) and T004 (TestResult) from Setup
- **US2 Session (Phase 3)**: Depends on T002 (session helpers) and T004 (TestResult) from Setup
- **US3 RAG (Phase 4)**: Depends on T003 (create_rag_agent) and T004 (TestResult) from Setup
- **US4 Streaming (Phase 5)**: Depends on T004 (TestResult) from Setup only; reuses existing calculator tool from common.rs
- **Polish (Phase 6)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after T001+T004 — Independent, no dependencies on other stories
- **User Story 2 (P1)**: Can start after T002+T004 — Independent, no dependencies on other stories
- **User Story 3 (P2)**: Can start after T003+T004 — Independent, no dependencies on other stories
- **User Story 4 (P2)**: Can start after T004 — Independent, no dependencies on other stories

### Within Each User Story

- CLI scaffold first
- Test implementations in order (simpler → more complex)
- Error handling last within each story
- Story complete before moving to next priority (sequential) OR stories in parallel

### Parallel Opportunities

- Phase 1: T002, T003, T004 can all run in parallel (different functions in common.rs)
- Phase 1: T005 depends on T001-T004 completion (must know file paths)
- After Setup: All 4 user stories (Phase 2-5) can run in PARALLEL
- Phase 6: T027, T028, T029, T030 can all run in parallel

---

## Parallel Example: User Stories After Setup

```bash
# Once Phase 1 (Setup) is complete, launch all 4 user stories concurrently:
Task: "US1: Create examples/memory_test.rs with write/search/reasoning tests"
Task: "US2: Create examples/session_test.rs with save/load/cleanup tests"
Task: "US3: Create examples/rag_test.rs with ingest/grounded/empty-kb tests"
Task: "US4: Create examples/streaming_tool_test.rs with single/multi-tool tests"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only — Memory)

1. Complete Phase 1: Setup (T001-T005)
2. Complete Phase 2: User Story 1 (T006-T010)
3. **STOP**: Run `cargo run --example memory_test -- --api-key sk-xxx`
4. Verify: 3/3 tests pass, clippy clean
5. Deliver memory_test.rs as MVP

### Incremental Delivery

1. Complete Setup + US1 → Memory tests done (MVP!)
2. Add US2 → Session tests done → 2 examples working
3. Add US3 → RAG tests done → 3 examples working
4. Add US4 → Streaming tests done → 4 examples working
5. Polish → All clean, fmt, quickstart validated

### Parallel Team Strategy

With multiple developers:
1. One dev completes Setup (Phase 1) — the shared common.rs extensions
2. Once Setup is done:
   - Developer A: US1 Memory (examples/memory_test.rs)
   - Developer B: US2 Session (examples/session_test.rs)
   - Developer C: US3 RAG (examples/rag_test.rs)
   - Developer D: US4 Streaming (examples/streaming_tool_test.rs)
3. Each story is a single new file, zero merge conflicts
4. Final validation together in Phase 6

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story produces ONE example binary — independently compilable and runnable
- No test tasks needed — these examples ARE the tests
- TestResult struct in common.rs is the only shared code between examples
- RAG example MockVectorStore is the only complex inline type; it can be ~100 lines based on vector_store_mock.rs
- All examples follow the same output format from contracts/cli-contracts.md
- Do NOT use `unwrap()` or `expect()` in production paths — use proper Result handling per Constitution Article 9
- Maximum 60s per test scenario from Success Criteria SC-002
