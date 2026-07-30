# Tasks: Session Management（会话管理）

**Input**: Design documents from `/specs/010-session-management/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Tests are REQUIRED — Constitution §6 mandates test-driven compatibility, and quickstart.md defines 6 validation scenarios.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- Rust workspace: `crates/<crate_name>/src/` for source, `crates/<crate_name>/tests/` for integration tests
- This feature extends 2 crates: `agent_scope_state` (primary), `agent_scope_event` (events)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add required dependencies and module declarations before any implementation

- [x] T001 Add `async-trait` and `tokio-util` dependencies to `crates/agent_scope_state/Cargo.toml`
- [x] T002 [P] Add `pub mod session;`, `pub mod session_store;`, `pub mod trim;` declarations to `crates/agent_scope_state/src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared types (SessionStatus, SessionError, SessionMeta) that all user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T003 Implement `SessionStatus` enum (Active, Closed) with `Serialize + Deserialize` in `crates/agent_scope_state/src/session.rs`
- [x] T004 Implement `SessionError` enum (6 variants: Closed, AlreadyExists, NotFound, SerializationError, StorageError, InvalidTrimConfig) with `Display + Error` impls in `crates/agent_scope_state/src/session.rs`
- [x] T005 Implement `SessionMeta` struct (session_id, status, message_count, created_at, last_active) with `Serialize + Deserialize` in `crates/agent_scope_state/src/session.rs`

**Checkpoint**: Foundation types ready — user story implementation can now begin

---

## Phase 3: User Story 1 — 创建和管理独立会话 (Priority: P1) 🎯 MVP

**Goal**: Session trait + SessionImpl enable creating, using, closing, and isolating conversation sessions. Each session wraps an AgentState and provides a CancellationToken for structured concurrency.

**Independent Test**: Create two sessions, append different messages to each, verify zero cross-contamination, close session A — session B remains active.

### Implementation for User Story 1

- [x] T006 [US1] Implement `Session` trait (id, status, state, state_mut, close, is_closed, created_at, last_active, touch) in `crates/agent_scope_state/src/session.rs`
- [x] T007 [US1] Implement `SessionImpl` struct wrapping `AgentState` with `new()`, `with_session_id()`, `cancel_token()` methods and `Session` trait impl in `crates/agent_scope_state/src/session.rs`
- [x] T008 [US1] Write session lifecycle tests (create, verify ID/status, append context, close, idempotent close, operations-rejected-after-close) in `crates/agent_scope_state/tests/session_tests.rs`
- [x] T009 [US1] Write session isolation tests (two sessions, append different messages, verify independent contexts, close one — other unaffected) in `crates/agent_scope_state/tests/session_tests.rs`

**Checkpoint**: US1 完成 — 会话可创建、使用、关闭、隔离，可独立验证

---

## Phase 4: User Story 2 — 会话状态持久化与恢复 (Priority: P2)

**Goal**: SessionStore trait + InMemorySessionStore enable save/load/delete/list of session state. Full AgentState round-trip including messages, reply_context, tool_context, middle_context.

**Independent Test**: Create session with 5 messages → save → load → verify 5 messages and all state identical → delete → verify NotFound.

### Implementation for User Story 2

- [x] T010 [US2] Implement `SessionStore` trait (save, load, delete, list_ids, list_meta) with `#[async_trait]` in `crates/agent_scope_state/src/session_store.rs`
- [x] T011 [US2] Implement `InMemorySessionStore` struct (HashMap<String, String> for JSON, HashMap<String, SessionMeta> for metadata, RwLock-protected) with `SessionStore` trait impl in `crates/agent_scope_state/src/session_store.rs`
- [x] T012 [US2] Write save/load/delete round-trip tests (create with messages → save → load to new SessionImpl → assert state identical → delete → assert NotFound) in `crates/agent_scope_state/tests/session_store_tests.rs`
- [x] T013 [US2] Write list_ids/list_meta tests (save 3 sessions with different message counts → list_meta → verify sorted by last_active desc) in `crates/agent_scope_state/tests/session_store_tests.rs`

**Checkpoint**: US2 完成 — 会话可持久化和恢复，US1+US2 均独立可测

---

## Phase 5: User Story 3 — 会话上下文修剪 (Priority: P3)

**Goal**: TrimStrategy config + trim_context() function prunes context when message count or token count exceeds thresholds. Guarantees: tool call chains intact, system messages preserved, trimmed content accumulated in summary.

**Independent Test**: Create session with 50 messages, apply TrimStrategy { max_messages: 30, keep_recent: 20 }, verify result ≤ 30 messages, latest 20 retained, tool chains not broken.

### Implementation for User Story 3

- [x] T014 [US3] Implement `TrimStrategy` struct (max_messages, max_tokens, keep_recent, keep_system_messages) with `Default`, `Serialize + Deserialize` in `crates/agent_scope_state/src/trim.rs`
- [x] T015 [US3] Implement `trim_context()` function (threshold check, walk-backward retention, tool-call/tool-result pairing, system message preservation, summary accumulation) in `crates/agent_scope_state/src/trim.rs`
- [x] T016 [P] [US3] Write count-based and token-based trimming tests (trim when over max_messages, trim when over max_tokens, no-trim when under threshold, keep_recent honored) in `crates/agent_scope_state/tests/trim_tests.rs`
- [x] T017 [P] [US3] Write tool chain and system message tests (tool-call+tool-result never split, system messages preserved at context start, summary field updated) in `crates/agent_scope_state/tests/trim_tests.rs`

**Checkpoint**: US3 完成 — 上下文修剪可配置、可验证，US1+US2+US3 均独立可测

---

## Phase 6: User Story 4 — 中间件上下文集成 (Priority: P3)

**Goal**: Verify middle_context (already on AgentState) participates correctly in Session lifecycle — persisted/restored with save/load, isolated between sessions. No new structs needed; this phase validates existing behavior in the Session context.

**Independent Test**: Write to session A's middle_context via state_mut(), save/load, verify data restored. Verify session B cannot see session A's middle_context data.

### Implementation for User Story 4

- [x] T018 [US4] Write middle_context persistence test (write key-value to middle_context, save session, load, verify key-value restored) in `crates/agent_scope_state/tests/session_store_tests.rs`
- [x] T019 [US4] Write middle_context isolation test (write to session A's middle_context, verify session B's middle_context does NOT contain session A's data) in `crates/agent_scope_state/tests/session_tests.rs`

**Checkpoint**: US4 完成 — middle_context 在 Session 生命周期中正确参与持久化和隔离

---

## Phase 7: Session Events

**Purpose**: Emit AgentEvent variants for all session state changes — created, closed, saved, loaded, trimmed

- [x] T020 [P] Implement 5 session event structs (`SessionCreatedEvent`, `SessionClosedEvent`, `SessionSavedEvent`, `SessionLoadedEvent`, `SessionTrimmedEvent`) in `crates/agent_scope_event/src/session_events.rs`
- [x] T021 Add 5 `EventType` variants (SessionCreated, SessionClosed, SessionSaved, SessionLoaded, SessionTrimmed) to enum in `crates/agent_scope_event/src/event_type.rs`
- [x] T022 Add 5 `AgentEvent` variants + `pub mod session_events` + re-exports in `crates/agent_scope_event/src/lib.rs`
- [x] T023 Write event serialization/round-trip tests (serialize each event to JSON, verify type tag, deserialize back, assert equality) in `crates/agent_scope_event/tests/session_events_tests.rs`

**Checkpoint**: Session events 完整 — 5 种事件类型可序列化/反序列化，所有会话状态变化可追踪

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Structured concurrency wiring, public API re-exports, workspace validation

- [x] T024 Wire `CancellationToken` into `SessionImpl::close()` (cancel token on close) and `Drop` impl (cancel token as safety net if not already closed) in `crates/agent_scope_state/src/session.rs`
- [x] T025 Update public re-exports in `crates/agent_scope_state/src/lib.rs` — add `Session`, `SessionImpl`, `SessionStatus`, `SessionMeta`, `SessionError`, `SessionStore`, `InMemorySessionStore`, `TrimStrategy`, `trim_context`, `TrimResult`
- [x] T026 Run all quickstart.md validation scenarios and verify 6/6 pass:
  - Scenario 1: Session create & close → `cargo test -p agent_scope_state -- session_create_close`
  - Scenario 2: Session isolation → `cargo test -p agent_scope_state -- session_isolation`
  - Scenario 3: Save/load/delete → `cargo test -p agent_scope_state -- session_save_load_delete`
  - Scenario 4: Context trimming → `cargo test -p agent_scope_state -- context_trimming`
  - Scenario 5: Session events → `cargo test -p agent_scope_event -- session_events`
  - Scenario 6: Full lifecycle → `cargo test -p agent_scope_state -- session_full_lifecycle`
- [x] T027 Run workspace validation: `cargo test --workspace` (all tests pass), `cargo clippy --all-targets -- -D warnings` (0 warnings), `cargo fmt -- --check` (clean)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup (Phase 1) — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational (Phase 2)
- **User Story 2 (Phase 4)**: Depends on Foundational (Phase 2) — may reference US1 types but independently testable
- **User Story 3 (Phase 5)**: Depends on Foundational (Phase 2) — independently testable
- **User Story 4 (Phase 6)**: Depends on US1 + US2 (needs Session + SessionStore for full lifecycle tests)
- **Session Events (Phase 7)**: Depends on Foundational (Phase 2) — different crate, can run in parallel with US1-US4
- **Polish (Phase 8)**: Depends on ALL prior phases complete

### User Story Dependencies

```
Setup (Phase 1)
    │
    ▼
Foundational (Phase 2) ────────────────────────────────────┐
    │                                                        │
    ├──▶ US1 (Phase 3) ──┐                                   │
    │                     ├──▶ US4 (Phase 6)                 │
    ├──▶ US2 (Phase 4) ──┘                                   │
    │                                                        │
    ├──▶ US3 (Phase 5)                                       │
    │                                                        │
    └──▶ Events (Phase 7) ─────────────────────────────────▶ Polish (Phase 8)
```

- **US1 (P1)**: Can start after Foundational — No dependencies on other user stories
- **US2 (P2)**: Can start after Foundational — No dependencies on other user stories (independently testable)
- **US3 (P3)**: Can start after Foundational — No dependencies on other user stories (independently testable)
- **US4 (P3)**: Depends on US1 + US2 — needs Session + SessionStore for full lifecycle verification
- **Events (Phase 7)**: Can start after Foundational — different crate, independently buildable
- **Polish (Phase 8)**: Requires all phases complete

### Within Each User Story

- Struct/enum definitions before trait impl
- Trait before implementation struct
- Implementation before tests
- Tests written to FAIL first, then pass after implementation

### Parallel Opportunities

- **Phase 1**: T002 can run in parallel with T001 (different file: lib.rs vs Cargo.toml)
- **Phase 2**: T003–T005 are all in session.rs — sequential within file
- **Phase 3–6**: US1, US2, US3 can all start in parallel after Phase 2 (different source files)
- **Phase 7** (Events): Can run entirely in parallel with US1–US4 (different crate)
- **Phase 5**: T016 and T017 are both in trim_tests.rs — but they test different aspects, can run in parallel (different test functions)
- **Phase 8**: T024, T025 are in different files — can run in parallel

### Files by Phase (to avoid conflicts)

| File | Phases |
|------|--------|
| `agent_scope_state/Cargo.toml` | Phase 1 (T001) |
| `agent_scope_state/src/lib.rs` | Phase 1 (T002), Phase 8 (T025) |
| `agent_scope_state/src/session.rs` | Phase 2 (T003–T005), Phase 3 (T006–T007), Phase 8 (T024) |
| `agent_scope_state/src/session_store.rs` | Phase 4 (T010–T011) |
| `agent_scope_state/src/trim.rs` | Phase 5 (T014–T015) |
| `agent_scope_state/tests/session_tests.rs` | Phase 3 (T008–T009), Phase 6 (T019) |
| `agent_scope_state/tests/session_store_tests.rs` | Phase 4 (T012–T013), Phase 6 (T018) |
| `agent_scope_state/tests/trim_tests.rs` | Phase 5 (T016–T017) |
| `agent_scope_event/src/session_events.rs` | Phase 7 (T020) |
| `agent_scope_event/src/event_type.rs` | Phase 7 (T021) |
| `agent_scope_event/src/lib.rs` | Phase 7 (T022) |
| `agent_scope_event/tests/session_events_tests.rs` | Phase 7 (T023) |

---

## Parallel Examples

### Phase 3 + 4 + 5: US1, US2, US3 in parallel

```bash
# After Phase 2 completes, launch all three independently:

# US1 — Session lifecycle
Task: "T006 [US1] Implement Session trait in crates/agent_scope_state/src/session.rs"
Task: "T007 [US1] Implement SessionImpl in crates/agent_scope_state/src/session.rs"

# US2 — SessionStore persistence (different file: session_store.rs)
Task: "T010 [US2] Implement SessionStore trait in crates/agent_scope_state/src/session_store.rs"
Task: "T011 [US2] Implement InMemorySessionStore in crates/agent_scope_state/src/session_store.rs"

# US3 — Context trimming (different file: trim.rs)
Task: "T014 [US3] Implement TrimStrategy in crates/agent_scope_state/src/trim.rs"
Task: "T015 [US3] Implement trim_context() in crates/agent_scope_state/src/trim.rs"
```

### Phase 7: Session Events (parallel with user stories)

```bash
# Events in agent_scope_event — completely separate crate, no conflicts with agent_scope_state work

Task: "T020 [P] Implement session event structs in crates/agent_scope_event/src/session_events.rs"
Task: "T021 Add EventType variants in crates/agent_scope_event/src/event_type.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001–T002)
2. Complete Phase 2: Foundational (T003–T005)
3. Complete Phase 3: User Story 1 (T006–T009)
4. **STOP and VALIDATE**: `cargo test -p agent_scope_state -- session`
5. Demo: Sessions can be created, used, closed, and isolated

### Incremental Delivery

1. Setup + Foundational → Foundation types ready
2. Add US1 → Sessions can be created/closed/isolated → MVP!
3. Add US2 → Sessions can be persisted/restored → Usable for multi-turn apps
4. Add US3 → Context trimming prevents overflow → Long-conversation safe
5. Add US4 → Middleware context verified → Middleware integration confirmed
6. Add Events → All lifecycle changes observable → Production monitoring ready
7. Polish → Workspace clean, all checks pass → Release candidate

### Parallel Team Strategy

With multiple developers:

1. All complete Setup + Foundational together (Phase 1–2)
2. Once Foundational is done (all in session.rs, sequential):
   - Developer A: US1 (session.rs implementation + tests)
   - Developer B: US2 (session_store.rs implementation + tests)
   - Developer C: US3 (trim.rs implementation + tests)
   - Developer D: Events (agent_scope_event crate, no conflicts)
3. Developer A + B collaborate on US4 (cross-story integration)
4. All converge on Phase 8 (Polish)

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks in other files
- [Story] label maps task to specific user story for traceability (US1, US2, US3, US4)
- Each user story should be independently completable and testable
- Tests are REQUIRED per Constitution §6 — write tests alongside implementation, verify they pass
- Commit after each phase or logical task group
- Stop at any checkpoint to validate story independently
- `cargo test -p <crate>` for per-crate validation; `cargo test --workspace` for full validation
- Avoid: vague tasks, cross-crate circular dependencies, same-file conflicts in parallel phases
