# Tasks: End Event Content

**Input**: Design documents from `/specs/014-end-event-content/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/event-protocol.md, quickstart.md, constitution.md

**Tests**: Required by spec FR-020, SC-001 through SC-006, quickstart validation scenarios, and Constitution Article 6.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)
- Every task includes exact file paths for implementation or validation targets

## Path Conventions

- Rust workspace crates live under `crates/`
- Event protocol structs live in `crates/agent_scope_event/src/`
- Agent event production paths live in `crates/agent_scope_agent/src/`
- Feature documents live in `specs/014-end-event-content/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Locate affected event constructors, test fixtures, and current event production paths before changing behavior.

- [ ] T001 Inspect existing TextBlockEndEvent and ThinkingBlockEndEvent definitions and constructors in crates/agent_scope_event/src/block_events.rs
- [ ] T002 Inspect existing ToolCallEndEvent and ToolResultEndEvent definitions and constructors in crates/agent_scope_event/src/tool_events.rs
- [ ] T003 [P] Inspect current event serde coverage and constructor usage in crates/agent_scope_event/tests/event_serde_tests.rs
- [ ] T004 [P] Inspect existing append/cross-crate event tests for EndEvent construction patterns in crates/agent_scope_event/tests/append_event_tests.rs and crates/agent_scope_event/tests/cross_crate_tests.rs
- [ ] T005 [P] Inspect streaming block lifecycle helpers and BlockTracker state in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T006 [P] Inspect non-streaming event production path in crates/agent_scope_agent/src/react_loop.rs

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Extend the public event protocol in a backward-compatible way before any producer starts filling fields.

**⚠️ CRITICAL**: No user story implementation can rely on EndEvent content until this phase is complete.

- [ ] T007 Add optional `text: Option<String>` with serde default and skip-none semantics to TextBlockEndEvent in crates/agent_scope_event/src/block_events.rs
- [ ] T008 Add optional `thinking: Option<String>` with serde default and skip-none semantics to ThinkingBlockEndEvent in crates/agent_scope_event/src/block_events.rs
- [ ] T009 Add optional `input: Option<String>` with serde default and skip-none semantics to ToolCallEndEvent in crates/agent_scope_event/src/tool_events.rs
- [ ] T010 Add optional `output: Option<String>` with serde default and skip-none semantics to ToolResultEndEvent in crates/agent_scope_event/src/tool_events.rs
- [ ] T011 Update EndEvent constructor helpers or call sites to preserve existing construction with `None` defaults in crates/agent_scope_event/src/block_events.rs and crates/agent_scope_event/src/tool_events.rs
- [ ] T012 [P] Update event crate EndEvent construction compatibility tests for new optional fields in crates/agent_scope_event/tests/append_event_tests.rs
- [ ] T013 [P] Update cross-crate EndEvent construction compatibility tests for new optional fields in crates/agent_scope_event/tests/cross_crate_tests.rs
- [ ] T014 Run targeted protocol compile check with `rtk cargo test -p agent_scope_event --no-run`

**Checkpoint**: Event protocol compiles, old EndEvent construction remains source-compatible or has explicit None defaults, and user story work can begin.

---

## Phase 3: User Story 1 - 消费者在 EndEvent 读取完整内容 (Priority: P1) 🎯 MVP

**Goal**: Streaming EndEvents for text, thinking, tool calls, and tool results carry complete content equal to the concatenation of observed deltas.

**Independent Test**: Use scripted/mock streaming event sequences and verify each TextBlockEndEvent.text, ThinkingBlockEndEvent.thinking, ToolCallEndEvent.input, and ToolResultEndEvent.output equals the block-specific delta concatenation.

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation.**

- [ ] T015 [P] [US1] Add event serde tests for populated `text`, `thinking`, `input`, and `output` EndEvent fields in crates/agent_scope_event/tests/event_serde_tests.rs
- [ ] T016 [P] [US1] Add streaming text/thinking multi-chunk EndEvent content regression tests in crates/agent_scope_agent/tests/streaming_end_event_content_tests.rs
- [ ] T017 [P] [US1] Add streaming tool call input multi-chunk EndEvent content regression tests in crates/agent_scope_agent/tests/streaming_end_event_content_tests.rs
- [ ] T018 [P] [US1] Add streaming tool result output EndEvent content regression tests in crates/agent_scope_agent/tests/streaming_tool_result_end_event_content_tests.rs
- [ ] T019 [P] [US1] Add interleaved block accumulator isolation test with at least 10 block/tool ids in crates/agent_scope_agent/tests/interleaved_end_event_content_tests.rs

### Implementation for User Story 1

- [ ] T020 [US1] Accumulate text deltas per block after TextBlockDeltaEvent publication in BlockTracker path in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T021 [US1] Populate TextBlockEndEvent.text from accumulated text when closing text blocks in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T022 [US1] Accumulate thinking deltas per block after ThinkingBlockDeltaEvent publication in BlockTracker path in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T023 [US1] Populate ThinkingBlockEndEvent.thinking from accumulated thinking when closing thinking blocks in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T024 [US1] Populate ToolCallEndEvent.input from accumulated ToolCallBlock.input when closing active tool blocks in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T025 [US1] Accumulate successful streaming ToolResultTextDeltaEvent output chunks per tool call in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T026 [US1] Populate ToolResultEndEvent.output for successful streaming tool results and omit it for interrupted tool streams in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T027 [US1] Run US1 targeted tests with `rtk cargo test -p agent_scope_event event_serde` and `rtk cargo test -p agent_scope_agent streaming_end_event_content`

**Checkpoint**: Streaming consumers can read complete block/tool content from EndEvent fields without losing DeltaEvent compatibility.

---

## Phase 4: User Story 2 - 非流式响应也发布完整 EndEvent 内容 (Priority: P1)

**Goal**: Non-streaming model responses and one-shot tool results populate the same EndEvent content fields as streaming paths.

**Independent Test**: Use non-streaming scripted/model responses and deterministic tool outputs to verify EndEvent fields contain complete text, thinking, tool input, and tool output.

### Tests for User Story 2 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation.**

- [ ] T028 [P] [US2] Add non-streaming text and thinking EndEvent content tests in crates/agent_scope_agent/tests/non_streaming_end_event_content_tests.rs
- [ ] T029 [P] [US2] Add non-streaming tool call input and tool result output EndEvent content tests in crates/agent_scope_agent/tests/non_streaming_tool_end_event_content_tests.rs
- [ ] T030 [P] [US2] Add non-streaming tool error path test that preserves error state without claiming successful output in crates/agent_scope_agent/tests/non_streaming_tool_end_event_content_tests.rs

### Implementation for User Story 2

- [ ] T031 [US2] Populate TextBlockEndEvent.text from complete TextBlock content in non-streaming react loop path in crates/agent_scope_agent/src/react_loop.rs
- [ ] T032 [US2] Populate ThinkingBlockEndEvent.thinking from complete ThinkingBlock content in non-streaming react loop path in crates/agent_scope_agent/src/react_loop.rs
- [ ] T033 [US2] Populate ToolCallEndEvent.input from complete tool call input in non-streaming react loop path in crates/agent_scope_agent/src/react_loop.rs
- [ ] T034 [US2] Populate ToolCallEndEvent.input in complete model response processing path in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T035 [US2] Populate ToolResultEndEvent.output for successful complete tool outputs while preserving error output semantics in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T036 [US2] Run US2 targeted tests with `rtk cargo test -p agent_scope_agent non_streaming_end_event_content` and `rtk cargo test -p agent_scope_agent non_streaming_tool_end_event_content`

**Checkpoint**: Streaming and non-streaming consumers can use the same EndEvent snapshot strategy.

---

## Phase 5: User Story 3 - 旧消费者可继续只依赖 EndEvent 生命周期语义 (Priority: P2)

**Goal**: Existing consumers that rely on EndEvent type, order, identifiers, and completion timing observe unchanged lifecycle semantics apart from optional content fields.

**Independent Test**: Compare event type order, EndEvent counts, block/tool identifiers, empty-content behavior, and cancellation/error behavior before and after content fields are introduced.

### Tests for User Story 3 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before any missing compatibility implementation.**

- [ ] T037 [P] [US3] Add backward-compatible deserialization tests for EndEvent JSON missing new fields in crates/agent_scope_event/tests/event_serde_tests.rs
- [ ] T038 [P] [US3] Add empty string versus missing field round-trip tests for all EndEvent content fields in crates/agent_scope_event/tests/event_serde_tests.rs
- [ ] T039 [P] [US3] Add event order and EndEvent count regression tests for streaming sequences in crates/agent_scope_agent/tests/streaming_end_event_content_tests.rs
- [ ] T040 [P] [US3] Add cancellation/error EndEvent content omission regression tests in crates/agent_scope_agent/tests/streaming_tool_result_end_event_content_tests.rs

### Implementation for User Story 3

- [ ] T041 [US3] Ensure EndEvent serialization omits None fields while preserving Some empty strings in crates/agent_scope_event/src/block_events.rs and crates/agent_scope_event/src/tool_events.rs
- [ ] T042 [US3] Ensure close helpers clear per-id accumulator state when EndEvent is emitted without introducing late deltas in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T043 [US3] Ensure cancellation and error paths omit unknown complete-content fields instead of fabricating successful content in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T044 [US3] Run US3 targeted tests with `rtk cargo test -p agent_scope_event event_serde` and `rtk cargo test -p agent_scope_agent streaming_tool_result_end_event_content`

**Checkpoint**: Existing lifecycle consumers remain compatible, and old serialized data remains valid.

---

## Phase 6: User Story 4 - Trace 与调试工具可直接展示完整块内容 (Priority: P3)

**Goal**: Trace/debug consumers can reconstruct block-level final output from EndEvent content fields and compare it with DeltaEvent accumulation.

**Independent Test**: Capture or synthesize a trace with text, thinking, tool input, and tool output, then verify EndEvent snapshots reconstruct the same block-level output as all delta events.

### Tests for User Story 4 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation if trace capture does not include new fields.**

- [ ] T045 [P] [US4] Add trace reconstruction test from EndEvent content fields in crates/agent_scope_agent/tests/end_event_trace_content_tests.rs
- [ ] T046 [P] [US4] Add DeltaEvent versus EndEvent snapshot equivalence test in crates/agent_scope_agent/tests/end_event_trace_content_tests.rs

### Implementation for User Story 4

- [ ] T047 [US4] Ensure AgentEvent trace serialization includes populated EndEvent content fields without changing trace event ordering in crates/agent_scope_agent/src/streaming_reactor.rs
- [ ] T048 [US4] Ensure non-streaming trace serialization includes populated EndEvent content fields without changing ReplyEnd behavior in crates/agent_scope_agent/src/react_loop.rs
- [ ] T049 [US4] Run US4 targeted trace tests with `rtk cargo test -p agent_scope_agent end_event_trace_content`

**Checkpoint**: Trace tools can read EndEvent complete-content snapshots and reconstruct the same block-level output as delta accumulation.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, compatibility notes, formatting, linting, and full regression validation.

- [ ] T050 [P] Update event protocol compatibility notes for EndEvent snapshot fields in docs/ or specs/014-end-event-content/contracts/event-protocol.md
- [ ] T051 [P] Update examples or comments that construct EndEvent values after constructor/API changes in examples/ and crates/agent_scope_event/tests/
- [ ] T052 Run quickstart validation scenarios with `rtk cargo test -p agent_scope_event event_serde`, `rtk cargo test -p agent_scope_agent non_streaming_end_event_content`, `rtk cargo test -p agent_scope_agent non_streaming_tool_end_event_content`, `rtk cargo test -p agent_scope_agent streaming_end_event_content`, `rtk cargo test -p agent_scope_agent streaming_tool_result_end_event_content`, and `rtk cargo test -p agent_scope_agent interleaved_end_event_content`
- [ ] T053 Run full workspace regression with `rtk cargo test`
- [ ] T054 Run lint gate with `rtk cargo clippy --all-targets --all-features -- -D warnings`
- [ ] T055 Run formatting gate with `rtk cargo fmt --check`
- [ ] T056 Update specs/014-end-event-content/tasks.md checkboxes after implementation completion and validation evidence collection

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational; MVP for streaming EndEvent snapshot behavior
- **User Story 2 (Phase 4)**: Depends on Foundational; can proceed in parallel with US1 after protocol fields exist, but final validation should compare behavior with US1
- **User Story 3 (Phase 5)**: Depends on Foundational; compatibility checks can start early, but final error/cancellation assertions depend on US1/US2 producer changes
- **User Story 4 (Phase 6)**: Depends on US1 and US2 producing populated fields
- **Polish (Phase 7)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: Can start after Phase 2; no dependency on US2/US3/US4
- **US2 (P1)**: Can start after Phase 2; no dependency on US1 for implementation, but should align semantics with US1 before completion
- **US3 (P2)**: Can start after Phase 2 for serde compatibility; lifecycle and cancellation checks depend on US1/US2 production paths
- **US4 (P3)**: Depends on US1 and US2 because trace content exists only after producers populate EndEvent fields

### Within Each User Story

- Tests MUST be written and observed failing before implementation tasks are completed
- Protocol structs before producer code
- Producer content accumulation before trace/debug reconstruction
- Targeted tests before full workspace regression

### Parallel Opportunities

- T003-T006 can run in parallel during setup
- T012-T013 can run in parallel after T007-T011
- T015-T019 can run in parallel because they target separate test scenarios/files
- T028-T030 can run in parallel because they target separate non-streaming scenarios
- T037-T040 can run in parallel because they target independent compatibility dimensions
- T045-T046 can run in parallel because they cover separate trace assertions
- T050-T051 can run in parallel during polish

---

## Parallel Example: User Story 1

```bash
# Write US1 streaming protocol tests in parallel:
Task: "Add streaming text/thinking multi-chunk EndEvent content regression tests in crates/agent_scope_agent/tests/streaming_end_event_content_tests.rs"
Task: "Add streaming tool result output EndEvent content regression tests in crates/agent_scope_agent/tests/streaming_tool_result_end_event_content_tests.rs"
Task: "Add interleaved block accumulator isolation test with at least 10 block/tool ids in crates/agent_scope_agent/tests/interleaved_end_event_content_tests.rs"

# Then implement producer paths in order because they touch the same file:
Task: "Accumulate text deltas per block in crates/agent_scope_agent/src/streaming_reactor.rs"
Task: "Accumulate thinking deltas per block in crates/agent_scope_agent/src/streaming_reactor.rs"
Task: "Populate tool call and tool result EndEvent fields in crates/agent_scope_agent/src/streaming_reactor.rs"
```

## Parallel Example: User Story 2

```bash
# Write US2 non-streaming tests in parallel:
Task: "Add non-streaming text and thinking EndEvent content tests in crates/agent_scope_agent/tests/non_streaming_end_event_content_tests.rs"
Task: "Add non-streaming tool call input and tool result output EndEvent content tests in crates/agent_scope_agent/tests/non_streaming_tool_end_event_content_tests.rs"

# Then implement non-streaming producers in order because they share react_loop.rs and streaming_reactor.rs paths:
Task: "Populate non-streaming TextBlockEndEvent and ThinkingBlockEndEvent fields in crates/agent_scope_agent/src/react_loop.rs"
Task: "Populate complete tool call and tool result EndEvent fields in crates/agent_scope_agent/src/streaming_reactor.rs"
```

## Parallel Example: User Story 3

```bash
# Compatibility tests can be written in parallel:
Task: "Add backward-compatible deserialization tests in crates/agent_scope_event/tests/event_serde_tests.rs"
Task: "Add event order and EndEvent count regression tests in crates/agent_scope_agent/tests/streaming_end_event_content_tests.rs"
Task: "Add cancellation/error content omission regression tests in crates/agent_scope_agent/tests/streaming_tool_result_end_event_content_tests.rs"
```

---

## Implementation Strategy

### MVP First (US1 + protocol foundation)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational protocol field additions
3. Complete Phase 3: US1 streaming EndEvent complete-content snapshots
4. **STOP and VALIDATE**: Run US1 targeted quickstart commands and verify Start → Delta → End ordering is unchanged

### P1 Completion

1. Complete MVP steps above
2. Complete Phase 4: US2 non-streaming EndEvent complete-content snapshots
3. Validate streaming and non-streaming semantic equivalence for text, thinking, tool input, and tool output

### Incremental Delivery

1. Protocol fields + serde compatibility → consumers can accept new fields
2. US1 streaming snapshots → streaming consumers get complete EndEvent content
3. US2 non-streaming snapshots → unified consumer logic across modes
4. US3 compatibility hardening → lifecycle semantics, old JSON, empty content, cancellation/error behavior
5. US4 trace reconstruction → debugging and observability improvement
6. Polish → docs, examples, full tests, clippy, fmt

### Parallel Team Strategy

With multiple developers:

1. One developer completes Phase 2 protocol structs and constructor updates
2. After Phase 2:
   - Developer A: US1 streaming tests and streaming_reactor.rs accumulation
   - Developer B: US2 non-streaming tests and react_loop.rs complete-content fill
   - Developer C: US3 serde/order/error compatibility tests
3. After US1 + US2 are complete:
   - Developer D: US4 trace reconstruction tests and documentation polish
4. Final owner runs T052-T055 validation gates

---

## Notes

- [P] tasks touch different files or independent test scenarios and can be done concurrently
- Non-[P] tasks often touch `crates/agent_scope_agent/src/streaming_reactor.rs` or `crates/agent_scope_agent/src/react_loop.rs` and should be serialized to avoid conflicts
- New EndEvent fields must be `Option<String>` and preserve `None` versus `Some("")` semantics
- EndEvent content is a convenience snapshot; DeltaEvent remains the streaming source of truth and must not be removed
- Do not fabricate complete output for error, cancellation, or interrupted tool result paths
- Use `rtk` prefix for all shell validation commands per project instructions
