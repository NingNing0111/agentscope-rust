# Tasks: Agent System

**Input**: Design documents from `/specs/007-agent-system/`

**Prerequisites**: plan.md (required), spec.md (required — 4 user stories), research.md, data-model.md, contracts/

**Tests**: Tests are included — the spec explicitly requires deterministic testing with Mock models, event trace verification, and contract tests (Constitution Articles 6, 7; SC-002).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- New crate: `crates/agent_scope_agent/`
- Source: `crates/agent_scope_agent/src/`
- Tests: `crates/agent_scope_agent/tests/`
- Existing crates at `crates/agent_scope_*/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the new `agent_scope_agent` crate and wire it into the workspace

- [x] T001 Create `crates/agent_scope_agent/` directory structure per plan.md: `src/` and `tests/` directories
- [x] T002 [P] Create `crates/agent_scope_agent/Cargo.toml` with workspace dependencies
- [x] T003 [P] Add `crates/agent_scope_agent` to workspace members in root `Cargo.toml` (auto-detected via `crates/*`)
- [x] T004 Verify workspace compiles: `cargo build` from repo root

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core types, traits, and infrastructure that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T005 Create `AgentError` enum with 10 variants in `crates/agent_scope_agent/src/agent_error.rs`
- [x] T006 [P] Create `AgentConfig` struct with builder pattern and validation in `crates/agent_scope_agent/src/config.rs`
- [x] T007 [P] Create `ReActConfig` struct with defaults and validation in `crates/agent_scope_agent/src/config.rs`
- [x] T008 [P] Create `ContextConfig` struct with defaults and validation in `crates/agent_scope_agent/src/config.rs`
- [x] T009 [P] Create `EventEmitter` in `crates/agent_scope_agent/src/event_emitter.rs`
- [x] T010 Define `Agent` trait with 5 methods in `crates/agent_scope_agent/src/agent_trait.rs`
- [x] T011 Create `crates/agent_scope_agent/src/lib.rs` with module declarations and re-exports
- [x] T012 [P] Create `MockModel` in `crates/agent_scope_agent/tests/mocks.rs`
- [x] T013 [P] Create `ScriptedModel` in `crates/agent_scope_agent/tests/mocks.rs`
- [x] T014 [P] Unit tests for `AgentConfig` validation in `config.rs` (inline)
- [x] T015 [P] Unit tests for `ReActConfig` validation in `config.rs` (inline)
- [x] T016 [P] Unit tests for `AgentError` Display and source chain in `agent_error.rs` (inline)

**Checkpoint**: Foundation ready — user story implementation can now begin. Crate compiles, all foundational types tested.

---

## Phase 3: User Story 1 - Create a Basic Text Agent (Priority: P1) 🎯 MVP

**Goal**: Implement `ReActAgent` with the basic reasoning loop (model call → text response). No tools, no middleware, no compression. A developer can create an agent, call `reply()`, and get a text response with correct event sequence.

**Independent Test**: Create agent with MockModel (returns fixed text). Call `agent.reply(user_msg("Hello"))`. Verify: (a) events emitted in order: ReplyStart → ModelCallStart → ModelCallEnd → TextBlockStart → TextBlockDelta → TextBlockEnd → ReplyEnd, (b) final Msg contains expected content, (c) AgentState records the reply context.

### Implementation for User Story 1

- [x] T017 [US1] Implement `ReActAgent` struct in `crates/agent_scope_agent/src/react_agent.rs`
- [x] T018 [US1] Implement `ReActAgent::new()` in `crates/agent_scope_agent/src/react_agent.rs`
- [x] T019 [US1] Implement `Agent::name()` and `Agent::state()` for ReActAgent in `crates/agent_scope_agent/src/react_agent.rs`
- [x] T020 [US1] Implement `Agent::observe()` for ReActAgent in `crates/agent_scope_agent/src/react_agent.rs`
- [x] T021 [US1] Implement basic reasoning loop in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T022 [US1] Implement `Agent::reply()` for ReActAgent in `crates/agent_scope_agent/src/react_agent.rs`
- [x] T023 [US1] Implement `Agent::reply_stream()` for ReActAgent in `crates/agent_scope_agent/src/react_agent.rs`
- [x] T024 [US1] Test: Basic text reply in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T025 [US1] Test: Event sequence for text reply in `crates/agent_scope_agent/tests/event_sequence_tests.rs`
- [x] T026 [US1] Test: `reply(None)` with empty context returns `Err(NoContentToReply)` in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T027 [US1] Test: `reply(None)` with existing context proceeds normally in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T028 [US1] Test: `observe()` appends messages to context in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T029 [US1] Test: `reply_stream()` yields all events in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T030 [US1] Test: Empty model response handled gracefully in `crates/agent_scope_agent/tests/react_agent_tests.rs`

**Checkpoint**: US1 complete — basic text agent works, event sequence verified, independently testable.

---

## Phase 4: User Story 2 - ReAct Agent with Tool Calls (Priority: P2)

**Goal**: Extend ReActAgent with the full reasoning-acting loop. The agent detects tool calls in model responses, executes tools via ToolKit, feeds results back, and iterates until a text response is produced.

**Independent Test**: Create ReActAgent with ScriptedModel (first response: tool_call for "calculator", second: text response). Register calculator tool. Verify: (a) tool call detected and executed, (b) tool result fed back to model, (c) full tool lifecycle events emitted (ToolCallStart → ToolCallEnd → ToolResultStart → ToolResultEnd), (d) final text response produced.

### Implementation for User Story 2

- [x] T031 [US2] Implement tool call detection: scan `ChatResponse::content` for `ContentBlock::ToolCall` blocks after model call in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T032 [US2] Implement tool execution: iterate tool calls → emit ToolCallStart → `toolkit.call_tool()` → emit ToolCallEnd → emit ToolResultStart/End → append ToolResult to state.context in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T033 [US2] Implement acting loop continuation: after tool execution, continue loop (go back to reasoning step with tool results in context) in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T034 [US2] Implement `max_iters` enforcement: increment `cur_iter` each loop, emit `ExceedMaxItersEvent` and break when exceeded in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T035 [US2] Implement structured output support: when `reply_context.structured_schema` is set, call `model.generate_structured_output()` instead of `model.call()`; retry up to `structured_output_grace_iters` on parse failure in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T036 [US2] Test: Tool call → execution → result → final text — ScriptedModel returns tool_call then text, verify full cycle in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T037 [US2] Test: Full tool lifecycle events — ToolCallStart → ToolCallEnd → ToolResultStart → ToolResultEnd in correct order in `crates/agent_scope_agent/tests/event_sequence_tests.rs`
- [x] T038 [US2] Test: `max_iters=1` with model always returning tool_call → ExceedMaxItersEvent emitted, last response returned in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T039 [US2] Test: ToolError from execution → ToolResultEnd with state=execution_error → error fed back to model in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T040 [US2] Test: 3+ iterations of reasoning-acting without state corruption in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T041 [US2] Test: Tool call with no tools registered → graceful handling (emit ToolResultEnd with error, feed to model) in `crates/agent_scope_agent/tests/react_agent_tests.rs`

**Checkpoint**: US2 complete — ReAct agent with tools works, tool lifecycle verified, independently testable.

---

## Phase 5: User Story 3 - Hook/Middleware Integration (Priority: P3)

**Goal**: Implement the `Middleware` trait and integrate hook dispatch into ReActAgent's lifecycle. Middleware can intercept at 8 hook points without modifying agent source code.

**Independent Test**: Create agent with middleware implementing `pre_reply` and `post_reply`. Trigger reply, verify both hooks fire. Test each remaining hook independently (pre_reasoning, post_reasoning, pre_acting, post_acting, pre_observe, pre_print).

### Implementation for User Story 3

- [x] T042 [US3] Define `Middleware` trait with 8 hook methods (pre_reply, post_reply, pre_reasoning, post_reasoning, pre_acting, post_acting, pre_observe, pre_print), each defaulting to no-op, in `crates/agent_scope_agent/src/middleware.rs`
- [x] T043 [US3] Implement hook dispatch helper: iterate `middlewares` in FIFO order per hook point, call each hook method, handle errors per hook point contract in `crates/agent_scope_agent/src/middleware.rs`
- [x] T044 [US3] Integrate `pre_reply` / `post_reply` hooks into `ReActAgent::reply()` flow in `crates/agent_scope_agent/src/react_agent.rs`
- [x] T045 [US3] Integrate `pre_reasoning` / `post_reasoning` hooks into reasoning step in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T046 [US3] Integrate `pre_acting` / `post_acting` hooks into tool execution step in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T047 [US3] Integrate `pre_observe` hook into `ReActAgent::observe()` in `crates/agent_scope_agent/src/react_agent.rs`
- [x] T048 [US3] Integrate `pre_print` hook (no-op for now, hook point available for future use) in `crates/agent_scope_agent/src/react_agent.rs`
- [x] T049 [US3] Implement middleware panic safety: catch panics via `std::panic::catch_unwind` with `AssertUnwindSafe` wrapper, convert to `AgentError` in `crates/agent_scope_agent/src/middleware.rs`
- [x] T050 [US3] Test: `pre_reply` and `post_reply` fire correctly around reply in `crates/agent_scope_agent/tests/middleware_tests.rs`
- [x] T051 [US3] Test: `pre_reasoning` can modify messages before model call in `crates/agent_scope_agent/tests/middleware_tests.rs`
- [x] T052 [US3] Test: `pre_acting` can modify or reject tool call in `crates/agent_scope_agent/tests/middleware_tests.rs`
- [x] T053 [US3] Test: `pre_observe` fires when observe() is called in `crates/agent_scope_agent/tests/middleware_tests.rs`
- [x] T054 [US3] Test: Middleware FIFO order — register [A, B, C], verify each hook fires A→B→C in `crates/agent_scope_agent/tests/middleware_tests.rs`
- [x] T055 [US3] Test: Middleware implementing only one hook — other hooks are no-ops (no false firing) in `crates/agent_scope_agent/tests/middleware_tests.rs`
- [x] T056 [US3] Test: `pre_reply` returning Err aborts reply, `post_reply` still fires with error in `crates/agent_scope_agent/tests/middleware_tests.rs`
- [x] T057 [US3] Test: Panicking middleware doesn't crash agent — error surfaced as AgentError in `crates/agent_scope_agent/tests/middleware_tests.rs`

**Checkpoint**: US3 complete — all 8 hook points work, middleware integration verified, independently testable.

---

## Phase 6: User Story 4 - Interruption and Cancellation (Priority: P4)

**Goal**: Implement graceful interruption. An external caller can interrupt a running reply, and the agent returns cleanly with an interruption message. The agent can resume normal operation afterward.

**Independent Test**: Start a long-running reply with a slow ScriptedModel. Interrupt with `agent.interrupt()`. Verify: (a) ReplyEnd with finished_reason=interrupted, (b) returned Msg has interruption_message, (c) agent can accept new reply() calls after interruption.

### Implementation for User Story 4

- [x] T058 [US4] Implement `ReActAgent::interrupt()` — set cancel_token, cancel any pending operations in `crates/agent_scope_agent/src/react_agent.rs`
- [x] T059 [US4] Add cancellation check at each loop iteration boundary (before model call, after tool execution) in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T060 [US4] Implement interruption handling: on cancel → emit UserInterruptEvent → emit ReplyEnd(finished_reason=Interrupted) → return interruption_message as Msg in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T061 [US4] Test: Interrupt during reasoning → loop exits cleanly, interruption message returned in `crates/agent_scope_agent/tests/interruption_tests.rs`
- [x] T062 [US4] Test: Interrupt during tool execution → pending tool calls marked interrupted → ToolResultEnd emitted → ReplyEnd(interrupted) in `crates/agent_scope_agent/tests/interruption_tests.rs`
- [x] T063 [US4] Test: Resume after interruption — new reply() call works normally in `crates/agent_scope_agent/tests/interruption_tests.rs`
- [x] T064 [US4] Test: Interrupt before reply starts → ReplyEnd emitted immediately, no model call in `crates/agent_scope_agent/tests/interruption_tests.rs`

**Checkpoint**: US4 complete — interruption works, agent can resume, independently testable.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Context compression, permission engine, edge case hardening, and final validation

- [x] T065 [P] Implement `PermissionEngine` with `PermissionRule` (tool_pattern, allow, require_confirm) and `check()` → `PermissionResult` (Allow/Deny/RequireConfirm) in `crates/agent_scope_agent/src/permission.rs`
- [x] T066 [P] Implement context compression: estimate token count → detect trigger threshold → call model for summarization → replace compressed messages in state.context in `crates/agent_scope_agent/src/context_compression.rs`
- [x] T067 [P] Implement context compression fallback: if compression model call fails, fall back to truncation (keep last N messages fitting reserve_ratio) in `crates/agent_scope_agent/src/context_compression.rs`
- [x] T068 [P] Implement token counting helper using `ChatModel::count_tokens()` for context size estimation before each model call in `crates/agent_scope_agent/src/token_counter.rs`
- [x] T069 [P] Replace `PermissionContext` placeholder in `crates/agent_scope_state/src/permission.rs` with real `PermissionEngine` or re-export from `agent_scope_agent`
- [x] T070 Integrate permission check before tool execution in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T071 Integrate context compression check before each model call in `crates/agent_scope_agent/src/react_loop.rs`
- [x] T072 Test: Permission denies tool → RequireUserConfirmEvent emitted in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T073 Test: Permission denies + stop_on_reject=true → loop stops, ReplyEnd emitted in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T074 Test: Context exceeds trigger_ratio → compression invoked → context size reduced in `crates/agent_scope_agent/tests/context_compression_tests.rs`
- [x] T075 Test: Context compression failure → falls back to truncation in `crates/agent_scope_agent/tests/context_compression_tests.rs`
- [x] T076 Test: `observe()` called while reply in progress — edge case handling (queue or error) in `crates/agent_scope_agent/tests/react_agent_tests.rs`
- [x] T077 Run `cargo clippy -p agent_scope_agent` and fix all warnings
- [x] T078 Run `cargo fmt --check` and ensure formatting clean
- [x] T079 Run `cargo test -p agent_scope_agent` — verify all tests pass
- [x] T080 Run quickstart.md validation: execute all 4 scenarios from quickstart.md and verify results

**Checkpoint**: All features complete, polished, and validated.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup (T001-T004) — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational (T005-T016) — MVP
- **User Story 2 (Phase 4)**: Depends on US1 (T017-T030) — extends react_loop with tools
- **User Story 3 (Phase 5)**: Depends on US1 (T017-T023) — extends with hooks; can be done in parallel with US2
- **User Story 4 (Phase 6)**: Depends on US1 (T017-T023) — extends with cancellation; can be done in parallel with US2/US3
- **Polish (Phase 7)**: Depends on US1-US4 completion

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Phase 2 — No dependencies on other stories
- **User Story 2 (P2)**: Can start after Phase 3 (US1) — extends ReActAgent with tool loop
- **User Story 3 (P3)**: Can start after Phase 3 (US1) — extends with middleware; CAN run in parallel with US2
- **User Story 4 (P4)**: Can start after Phase 3 (US1) — extends with cancellation; CAN run in parallel with US2/US3

### Within Each User Story

- Core implementation before tests
- Tests can be written in parallel (different test files)
- story complete before moving to next priority

### Parallel Opportunities

- **Phase 1**: T002, T003 can run in parallel
- **Phase 2**: T006, T007, T008, T009, T012, T013 are all independent (different files)
- **Phase 3**: Tests T024-T030 can run in parallel once implementation T017-T023 is done
- **Phase 4**: Tests T036-T041 can run in parallel
- **Phase 5**: Tests T050-T057 can run in parallel
- **Phase 6**: Tests T061-T064 can run in parallel
- **Phase 7**: T065, T066, T067, T068, T069 can run in parallel (different files)
- **Cross-phase**: US2, US3, US4 can be implemented in parallel after US1 core is complete

---

## Parallel Example: User Story 1

```bash
# After T017-T023 (implementation) complete, launch all US1 tests together:
Task: "T024 Test: Basic text reply in crates/agent_scope_agent/tests/react_agent_tests.rs"
Task: "T025 Test: Event sequence for text reply in crates/agent_scope_agent/tests/event_sequence_tests.rs"
Task: "T026 Test: reply(None) empty context in crates/agent_scope_agent/tests/react_agent_tests.rs"
Task: "T027 Test: reply(None) existing context in crates/agent_scope_agent/tests/react_agent_tests.rs"
Task: "T028 Test: observe() appends messages in crates/agent_scope_agent/tests/react_agent_tests.rs"
Task: "T029 Test: reply_stream() yields events in crates/agent_scope_agent/tests/react_agent_tests.rs"
Task: "T030 Test: Empty model response in crates/agent_scope_agent/tests/react_agent_tests.rs"
```

## Parallel Example: Foundational Phase

```bash
# All foundational type definitions are independent files:
Task: "T006 Create AgentConfig in crates/agent_scope_agent/src/config.rs"
Task: "T007 Create ReActConfig in crates/agent_scope_agent/src/config.rs"
Task: "T008 Create ContextConfig in crates/agent_scope_agent/src/config.rs"
Task: "T009 Create EventEmitter in crates/agent_scope_agent/src/event_emitter.rs"
Task: "T012 Create MockModel in crates/agent_scope_agent/tests/mocks.rs"
Task: "T013 Create ScriptedModel in crates/agent_scope_agent/tests/mocks.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T004)
2. Complete Phase 2: Foundational (T005-T016) — CRITICAL
3. Complete Phase 3: User Story 1 (T017-T030)
4. **STOP and VALIDATE**: Run `cargo test -p agent_scope_agent`, verify event sequence, verify all US1 scenarios
5. Demo: Basic text agent working

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US1 → Test independently → MVP: Basic agent works!
3. Add US2 → Test independently → User can use tools in agent loop
4. Add US3 → Test independently → Middleware extensibility works
5. Add US4 → Test independently → Graceful interruption works
6. Add Polish (Phase 7) → Context compression, permission engine, final hardening
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together (Phase 1-2)
2. Implement US1 core (T017-T023) together
3. Once US1 core is done:
   - Developer A: US1 tests (T024-T030) + start US2 implementation
   - Developer B: US3 implementation (T042-T048)
   - Developer C: US4 implementation (T058-T060)
4. Each completes tests for their story independently
5. Polish phase can be done in parallel (T065-T069 are independent)

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- All tests use MockModel/ScriptedModel — no live LLM API calls
- Event sequence tests verify exact ordering per AgentScope protocol (FR-010)
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- `cargo clippy` and `cargo fmt` run at the end (T077-T078)
- Run quickstart.md validation (T080) as final integration check
