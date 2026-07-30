# Tasks: Streaming Infrastructure

**Input**: Design documents from `specs/008-streaming-infrastructure/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Included — each user story has corresponding streaming integration tests per spec acceptance scenarios.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- **crate root**: `crates/agent_scope_agent/`
- **src/**: Production code
- **tests/**: Integration tests (inline with crate)

---

## Phase 1: Setup

**Purpose**: Verify preconditions and baseline — all existing tests must pass before any changes

- [x] T001 Verify all existing tests pass with `cargo test -p agent_scope_agent`
- [x] T002 Verify workspace-wide tests pass with `cargo test` and record test count baseline

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure changes that ALL user stories depend on — EventEmitter rewrite, AgentError extension, AgentConfig extension, StreamHandle, is_streaming guard

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T003 [P] Add `AlreadyStreaming` variant to `AgentError` enum in `crates/agent_scope_agent/src/agent_error.rs` with Display impl
- [X] T004 [P] Add `stream_channel_capacity: Option<usize>` field to `AgentConfig` in `crates/agent_scope_agent/src/config.rs` with builder method `with_stream_channel_capacity(mut self, cap: Option<usize>) -> Self`
- [X] T005 Rewrite `EventEmitter` from broadcast to mpsc in `crates/agent_scope_agent/src/event_emitter.rs`: change `new(capacity: Option<usize>)` for bounded/unbounded, make `emit()` async fn using `tx.send(event).await`, remove `subscribe()`, add `clone_sender()` returning `mpsc::Sender<AgentEvent>`
- [X] T006 Create `StreamHandle` struct in `crates/agent_scope_agent/src/stream_handle.rs`: `cancel_rx: oneshot::Receiver<()>`, `is_streaming: Arc<AtomicBool>`, `new()` factory, `is_cancelled()` check, `Drop` impl that clears is_streaming flag
- [X] T007 Add `is_streaming: AtomicBool` field to `AgentInner` in `crates/agent_scope_agent/src/react_agent.rs`, initialized to `false` in `ReActAgent::new()`
- [X] T008 [P] Add unit tests for `StreamHandle` in `crates/agent_scope_agent/src/stream_handle.rs` (test is_cancelled returns true after sender dropped, test is_streaming cleared on Drop)
- [X] T009 [P] Add unit test for `AlreadyStreaming` error format in `crates/agent_scope_agent/src/agent_error.rs`

**Checkpoint**: Foundation infrastructure ready — EventEmitter (mpsc), StreamHandle, AlreadyStreaming error, is_streaming guard all exist. All existing agent tests still pass (T001 baseline unchanged since no behavior change yet).

---

## Phase 3: User Story 1 - Real-Time Event Streaming to Callers (Priority: P1) 🎯 MVP

**Goal**: `reply_stream()` yields events progressively as model produces chunks, not after accumulation. The underlying `do_reply` spawns a streaming reactor that forwards model chunks in real-time. `EventStream` wraps `mpsc::Receiver` with Drop-triggered cancellation. The existing `reply()` method is updated to use the same streaming pipeline internally (accumulating events → final Msg).

**Independent Test**: Create agent with MockModel streaming 3 chunks. Call `reply_stream()`. Assert first event (ReplyStart) arrives before model completes, and TextBlockDelta events arrive progressively (not all at once).

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T010 [P] [US1] Test progressive event delivery: mock model streams 3 chunks, verify events arrive across multiple poll points in `crates/agent_scope_agent/tests/streaming_tests.rs`
- [X] T011 [P] [US1] Test ReplyStart event arrives within first poll (before model completion) in `crates/agent_scope_agent/tests/streaming_tests.rs`
- [X] T012 [P] [US1] Test non-streaming model produces same event sequence (single burst) in `crates/agent_scope_agent/tests/streaming_tests.rs`

### Implementation for User Story 1

- [X] T013 [US1] Create `EventStream` struct in `crates/agent_scope_agent/src/react_agent.rs`: wraps `mpsc::Receiver<AgentEvent>` + `Option<oneshot::Sender<()>>`, impl `Stream` (poll_next dequeues from rx, None after ReplyEnd), impl `Drop` (fires oneshot sender, clears is_streaming flag)
- [X] T014 [US1] Update `ReActAgent::reply_stream()` in `crates/agent_scope_agent/src/react_agent.rs`: create mpsc channel pair, set is_streaming via compare_exchange (return AlreadyStreaming if active), create StreamHandle + EventStream, spawn `run_streaming_loop()` in tokio task, return EventStream as `Pin<Box<dyn Stream>>`
- [X] T015 [US1] Update `ReActAgent::reply()` in `crates/agent_scope_agent/src/react_agent.rs`: use same streaming pipeline but collect all events from mpsc receiver into `Vec<AgentEvent>`, extract final `Msg` from last text content
- [X] T016 [US1] Create `run_streaming_loop()` entry point in `crates/agent_scope_agent/src/streaming_reactor.rs`: accept `ReactLoopContext`, `StreamHandle`, `mpsc::Sender<AgentEvent>`; implement progressive model stream processing (for each chunk: emit TextBlockStart/Delta/End events in real-time via event_tx.send().await, check stream_handle.is_cancelled())
- [X] T017 [US1] Implement model stream consumption in `run_streaming_loop()`: match `ModelCallResult::Stream` → while-let loop over chunks, emit ModelCallStart (first chunk only), emit per-block events progressively, handle `is_last` for ModelCallEnd, preserve ChatUsage from final chunk
- [X] T018 [US1] Handle `ModelCallResult::Complete` in `run_streaming_loop()`: emit all events in single burst (same sequence as stream but without interleaving)
- [X] T019 [US1] Handle model stream error in `run_streaming_loop()`: emit ReplyEnd with error state, terminate stream, return error
- [X] T020 [US1] Export new public types in `crates/agent_scope_agent/src/lib.rs`: `StreamHandle` (if pub), `AlreadyStreaming` error, `AgentConfig` builder method docs
- [X] T021 [US1] Update `interrupt()` method in `crates/agent_scope_agent/src/react_agent.rs` to work with new streaming model (interrupt sets AtomicBool, streaming loop checks it same as before)

**Checkpoint**: At this point, `reply_stream()` yields events progressively in real-time. `reply()` continues to work identically. All existing 47 agent tests still pass. T010-T012 new streaming tests pass.

---

## Phase 4: User Story 2 - Streaming Tool Call Detection (Priority: P2)

**Goal**: When model streams tool calls in fragments, the agent emits `ToolCallStart` → `ToolCallDelta` → `ToolCallEnd` events in real-time. When a tool call is detected as complete (arguments fully received, indicated by block-type transition or stream end), execution begins immediately — without waiting for subsequent model chunks.

**Independent Test**: Mock model streams: ToolCall("calc") frag 1 + ToolCall("calc") frag 2 + Text("Now computing..."). Verify ToolCallStart at frag1, ToolCallDelta at frag2, ToolCallEnd before TextDelta, tool execution interleaved.

### Tests for User Story 2

- [X] T022 [P] [US2] Test tool call completion detection: tool call spanning 3 chunks, verify executor invoked before text chunk arrives in `crates/agent_scope_agent/tests/streaming_tests.rs`
- [X] T023 [P] [US2] Test multiple tool calls interleaved with text: verify each tool call executes as soon as its arguments complete in `crates/agent_scope_agent/tests/streaming_tests.rs`
- [X] T024 [P] [US2] Test malformed JSON tool arguments: verify ToolCallEnd emitted + execution error (not silent ignore) in `crates/agent_scope_agent/tests/streaming_tests.rs`

### Implementation for User Story 2

- [X] T025 [US2] Extend `MockModel` with `with_streaming_tool_calls(tool_name, args_chunks, text_after)` method in `crates/agent_scope_agent/tests/mocks.rs` for test scenarios
- [X] T026 [US2] Add per-block tool call accumulation in `run_streaming_loop()` in `crates/agent_scope_agent/src/streaming_reactor.rs`: use `StreamAccumulator` per block_id for ToolCallBlock chunks, emit `ToolCallStart` on first chunk, `ToolCallDelta` on subsequent chunks for same block_id
- [X] T027 [US2] Implement tool call completion detection heuristic in `crates/agent_scope_agent/src/streaming_reactor.rs`: detect block-type transition (ToolCallBlock → TextBlock/ThinkingBlock/DataBlock) or stream end → mark tool call complete → emit `ToolCallEnd` → execute tool via `toolkit.call_tool()`
- [X] T028 [US2] Feed tool execution results back to model in `run_streaming_loop()`: append ToolResultBlock to context, continue ReAct loop (emit new ModelCallStart/End events as part of same continuous stream per FR-003)
- [X] T029 [US2] Handle tool execution error in `run_streaming_loop()`: emit `ToolResultEnd` with error state, feed error text to model context, continue loop if max_iters allows

**Checkpoint**: Tool calls detected progressively during streaming. Tool execution begins mid-stream. Multi-iteration ReAct loops produce single continuous event stream.

---

## Phase 5: User Story 3 - Streaming Tool Execution (Priority: P3)

**Goal**: Tools returning `ToolExecOutput::Stream` produce progressive `ToolResultTextDelta` events forwarded in real-time to the caller.

**Independent Test**: Register tool returning `ToolExecOutput::Stream` with 3 chunks. Execute via `reply_stream()`. Verify `ToolResultTextDelta` × 3 → `ToolResultEnd` arrive progressively.

### Tests for User Story 3

- [X] T030 [P] [US3] Test streaming tool output: tool yields 3 chunks, verify progressive ToolResultTextDelta events in `crates/agent_scope_agent/tests/streaming_tests.rs`
- [X] T031 [P] [US3] Test streaming tool failure mid-execution: tool yields 2 chunks then errors, verify ToolResultEnd with error state in `crates/agent_scope_agent/tests/streaming_tests.rs`
- [X] T032 [P] [US3] Test streaming tool interrupted via UserInterruptEvent: verify ToolResultEnd with interrupted state in `crates/agent_scope_agent/tests/streaming_tests.rs`

### Implementation for User Story 3

- [X] T033 [US3] Add streaming tool consumption in `run_streaming_loop()` in `crates/agent_scope_agent/src/streaming_reactor.rs`: when tool returns `ToolExecOutput::Stream`, spawn consumption loop that emits `ToolResultStart` → while-let chunk (emit `ToolResultTextDelta`, check StreamHandle) → on error (emit `ToolResultEnd` with error state)
- [X] T034 [US3] Handle `ToolExecOutput::Complete` path in streaming reactor: emit `ToolResultStart` → single `ToolResultTextDelta` → `ToolResultEnd` (backward compatible with existing tools per FR-015)
- [X] T035 [US3] Handle interruption during streaming tool execution: check `stream_handle.is_cancelled()` and `interrupted` AtomicBool in tool consumption loop, emit `ToolResultEnd(state=Interrupted)` on cancel
- [X] T036 [P] [US3] Create a mock streaming tool in `crates/agent_scope_agent/tests/mocks.rs` for testing: implements `Tool` trait, returns `ToolExecOutput::Stream` yielding configurable chunks

**Checkpoint**: Streaming tools produce progressive output. Batch tools continue to work unchanged. Interruption during tool execution handled correctly.

---

## Phase 6: User Story 4 - Backpressure and Flow Control (Priority: P4)

**Goal**: Bounded channel prevents unbounded memory growth. When channel is full, event emission blocks (backpressure) until consumer catches up. Bounded mode is opt-in; unbounded (default) preserves existing behavior.

**Independent Test**: Slow consumer with 100ms delays. Bounded channel of capacity 16. Fast model. Verify events never dropped, delivery order preserved, emission blocked when full.

### Tests for User Story 4

- [X] T037 [P] [US4] Test bounded channel backpressure: capacity 4, slow consumer (100ms delay), fast model (many chunks), verify all events delivered and no events lost in `crates/agent_scope_agent/tests/streaming_tests.rs`
- [X] T038 [P] [US4] Test unbounded channel (default) preserves all events with fast model and slow consumer in `crates/agent_scope_agent/tests/streaming_tests.rs`

### Implementation for User Story 4

- [X] T039 [US4] Wire `AgentConfig::stream_channel_capacity` through `ReActAgent::new()` into `EventEmitter::new()` in `crates/agent_scope_agent/src/react_agent.rs` (already created in T005, just pass the config value)
- [X] T040 [US4] Verify `EventEmitter::emit()` async behavior correctly propagates backpressure: `.send().await` blocks when bounded channel full, unblocks when consumer polls

**Checkpoint**: Bounded channel mode works. Default unbounded mode unchanged. Backpressure propagates correctly.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Backward compatibility verification, cancellation tests, concurrent call protection, lint/format cleanup, quickstart validation

- [X] T041 [P] Verify all pre-existing agent tests pass (backward compat): `cargo test -p agent_scope_agent` — all 47 tests from Feature 007 pass without modification
- [X] T042 [P] Test stream drop cancellation: create stream, drop after first event, verify is_streaming cleared and new reply succeeds in `crates/agent_scope_agent/tests/streaming_tests.rs`
- [X] T043 [P] Test AlreadyStreaming guard: call `reply_stream()` then call `reply()` before consuming stream, verify `Err(AgentError::AlreadyStreaming)` in `crates/agent_scope_agent/tests/streaming_tests.rs`
- [X] T044 [P] Test interrupted agent recovery: interrupt during streaming, verify ReplyEnd(interrupted), then new reply succeeds in `crates/agent_scope_agent/tests/streaming_tests.rs`
- [X] T045 Run `cargo clippy --all-targets -- -D warnings` and fix all warnings
- [X] T046 Run `cargo fmt --all -- --check` and fix any format issues
- [X] T047 Run full workspace tests: `cargo test` — all crates pass
- [X] T048 Run quickstart.md validation scenarios

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — verify baseline immediately
- **Foundational (Phase 2)**: Depends on Setup (baseline verified) — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational — No dependencies on other stories
- **User Story 2 (Phase 4)**: Depends on Phase 3 (extends streaming_reactor with tool call detection)
- **User Story 3 (Phase 5)**: Depends on Phase 4 (tool execution already working)
- **User Story 4 (Phase 6)**: Depends on Phase 3 (channel infrastructure from US1)
- **Polish (Phase 7)**: Depends on all desired user stories

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational — core streaming pipeline
- **User Story 2 (P2)**: Depends on US1 (extends streaming_reactor) — US2 builds on US1's progressive model stream consumption
- **User Story 3 (P3)**: Depends on US2 (tool execution infrastructure in streaming_reactor) — extends tool execution with streaming output
- **User Story 4 (P4)**: Depends on US1 (mpsc channel infrastructure) — independent of US2/US3 logic
- **US4 IS actually parallel with US2+US3** — channel capacity wiring is separate from tool handling logic

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Implementation follows: structs/types → reactor logic → agent wiring → integration
- Story complete (all tests pass) before moving to next priority

### Parallel Opportunities

- **Phase 2**: T003, T004, T008, T009 can all run in parallel (different files)
- **US1 Tests**: T010, T011, T012 can run in parallel
- **US2 Tests**: T022, T023, T024 can run in parallel
- **US3 Tests**: T030, T031, T032 can run in parallel (T036 can run in parallel with implementation)
- **US4 Tests**: T037, T038 can run in parallel
- **Phase 7**: T041, T042, T043, T044 can all run in parallel (different test functions in same files — write concurrently)
- **Cross-phase**: US4 (Phase 6) can be implemented in parallel with US2 (Phase 4) and US3 (Phase 5) after US1 is done

---

## Parallel Example: Phase 2 Foundational

```bash
# All independent foundational work:
Task: "T003 Add AlreadyStreaming variant in agent_error.rs"
Task: "T004 Add stream_channel_capacity to AgentConfig in config.rs"
Task: "T008 Unit tests for StreamHandle in stream_handle.rs"
Task: "T009 Unit test for AlreadyStreaming error in agent_error.rs"
```

## Parallel Example: User Story 1 Tests

```bash
# All US1 tests launch together:
Task: "T010 Test progressive event delivery in streaming_tests.rs"
Task: "T011 Test ReplyStart arrives within first poll in streaming_tests.rs"
Task: "T012 Test non-streaming model event sequence in streaming_tests.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (verify baseline)
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: User Story 1 (T010-T021)
4. **STOP and VALIDATE**: Run `cargo test -p agent_scope_agent`, verify progressive event delivery
5. Demo: `reply_stream()` yields events in real-time!

MVP delivers: Real-time text streaming at ~12 tasks (T001-T021)

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US1 → Test independently → MVP: progressive text streaming!
3. Add US2 → Test independently → Tool calls detected mid-stream!
4. Add US3 → Test independently → Streaming tool output!
5. Add US4 → Test independently → Bounded channel backpressure!
6. Polish (Phase 7) → Lint, format, backward compat verification, quickstart
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. US1 implementation (T013-T021) — single developer (sequential dependencies)
3. After US1 complete:
   - Developer A: US2 (T022-T029)
   - Developer B: US4 (T037-T040) — parallel with US2
   - They both extend different parts of the codebase
4. Developer A continues: US3 (T030-T036) — depends on US2
5. Developer B joins US3 work after US4 done
6. Both: Phase 7 Polish can be parallelized

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- All tests use MockModel/ScriptedModel — no live LLM API calls
- Event sequence tests verify exact ordering per AgentScope protocol (FR-003)
- SC-006 (47 existing tests pass) is the backward compatibility gate — checked at T041
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- `cargo clippy` and `cargo fmt` run at the end (T045-T046)
- Run quickstart.md validation (T048) as final integration check
- The key behavior change: `emit()` becomes `async fn` — this is the fundamental refactor that enables backpressure
