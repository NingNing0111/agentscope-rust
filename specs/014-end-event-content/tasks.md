# Tasks: End Event Content

**Input**: Design documents from `/specs/014-end-event-content/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/event-protocol.md, quickstart.md

**Tests**: Included — Feature spec explicitly requires tests (SC-001 through SC-006, FR-020, quickstart validation scenarios).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

```text
crates/agent_scope_event/src/         # Event struct definitions (protocol layer)
crates/agent_scope_event/tests/        # Event serialization tests
crates/agent_scope_agent/src/          # Event production (streaming + non-streaming)
crates/agent_scope_agent/tests/        # Agent event behavior tests
```

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify prerequisites and prepare for protocol changes.

- [X] T001 Verify existing tests pass before starting changes with `rtk cargo test -p agent_scope_event -p agent_scope_agent`
- [X] T002 [P] Review current EndEvent struct shapes in `crates/agent_scope_event/src/block_events.rs` and `crates/agent_scope_event/src/tool_events.rs` to confirm target fields

---

## Phase 2: Foundational — EndEvent Struct Extensions (Blocking)

**Purpose**: Add optional complete-content fields to all four EndEvent structs. This MUST complete before any agent-side content population can begin.

**⚠️ CRITICAL**: No user story implementation can begin until these struct changes are in place.

- [X] T003 [P] Add `text: Option<String>` field with `#[serde(default, skip_serializing_if = "Option::is_none")]` to `TextBlockEndEvent` in `crates/agent_scope_event/src/block_events.rs`
- [X] T004 [P] Add `thinking: Option<String>` field with `#[serde(default, skip_serializing_if = "Option::is_none")]` to `ThinkingBlockEndEvent` in `crates/agent_scope_event/src/block_events.rs`
- [X] T005 [P] Add `input: Option<String>` field with `#[serde(default, skip_serializing_if = "Option::is_none")]` to `ToolCallEndEvent` in `crates/agent_scope_event/src/tool_events.rs`
- [X] T006 [P] Add `output: Option<String>` field with `#[serde(default, skip_serializing_if = "Option::is_none")]` to `ToolResultEndEvent` in `crates/agent_scope_event/src/tool_events.rs`
- [X] T007 [P] Add serialization round-trip tests for new fields in `crates/agent_scope_event/tests/event_serde_tests.rs`: verify `Some("")`, `Some("hello")`, field-missing → `None`, empty-string ≠ missing
- [X] T008 Fix all compile errors from new mandatory struct fields across workspace (all existing EndEvent constructors need `text`/`thinking`/`input`/`output: None` for now)

**Checkpoint**: Struct extensions complete. All existing tests still pass with `None` defaults. Can now proceed to content population.

---

## Phase 3: User Story 1 — Consumer Reads Complete Content from EndEvent (Priority: P1) 🎯 MVP

**Goal**: Streaming model output paths populate EndEvent content fields from accumulated deltas. After streaming completes, each EndEvent carries its block's full content.

**Independent Test**: Construct streaming event sequences with text/thinking/tool-call/tool-result blocks, verify each EndEvent's content field equals concatenation of all that block's deltas.

### Tests for User Story 1

- [X] T009 [P] [US1] Add streaming TextBlockEndEvent content test in `crates/agent_scope_agent/tests/streaming_tests.rs`: multi-chunk text deltas → EndEvent.text equals concatenated result
- [X] T010 [P] [US1] Add streaming ThinkingBlockEndEvent content test in `crates/agent_scope_agent/tests/streaming_tests.rs`: multi-chunk thinking deltas → EndEvent.thinking equals concatenated result
- [X] T011 [P] [US1] Add streaming ToolCallEndEvent content test in `crates/agent_scope_agent/tests/streaming_tests.rs`: multi-chunk tool input deltas → EndEvent.input equals concatenated result
- [X] T012 [P] [US1] Add streaming ToolResultEndEvent content test in `crates/agent_scope_agent/tests/streaming_tests.rs`: multi-chunk tool result text deltas → EndEvent.output equals concatenated result

### Implementation for User Story 1

- [X] T013 [US1] Accumulate delta text in `process_text_block_chunk()` in `crates/agent_scope_agent/src/streaming_reactor.rs`: push `tb.text` into `tracker.text_blocks[block_id].1` after emitting delta, for both first-chunk and subsequent-chunk paths
- [X] T014 [US1] Accumulate delta thinking in `process_thinking_block_chunk()` in `crates/agent_scope_agent/src/streaming_reactor.rs`: push `thb.thinking` into `tracker.thinking_blocks[block_id].1` after emitting delta, for both first-chunk and subsequent-chunk paths
- [X] T015 [US1] Populate `TextBlockEndEvent.text` in `close_all_text_blocks()` in `crates/agent_scope_agent/src/streaming_reactor.rs`: concat all accumulated delta strings from `tracker.text_blocks[block_id].1` into `Some(concatenated)`
- [X] T016 [US1] Populate `ThinkingBlockEndEvent.thinking` in `close_all_thinking_blocks()` in `crates/agent_scope_agent/src/streaming_reactor.rs`: concat all accumulated delta strings from `tracker.thinking_blocks[block_id].1` into `Some(concatenated)`
- [X] T017 [US1] Populate `ToolCallEndEvent.input` in `close_active_tool_blocks()` in `crates/agent_scope_agent/src/streaming_reactor.rs`: pass `tracker.tool_blocks[id].input.clone()` as `Some(input)` when emitting EndEvent
- [X] T018 [US1] Populate `ToolResultEndEvent.output` in `emit_tool_result_and_collect()` in `crates/agent_scope_agent/src/streaming_reactor.rs`: pass `Some(collected_text)` for `Complete` and `Stream` success paths; pass `None` for `Stream` interrupted path and `Err` error path (not yet complete)

**Checkpoint**: Streaming EndEvent content fields populated. US1 tests pass — consumer can read complete content from streaming EndEvents.

---

## Phase 4: User Story 2 — Non-Streaming EndEvent Content (Priority: P1)

**Goal**: Non-streaming model response paths and one-shot tool execution paths populate EndEvent content fields, so consumers get consistent behavior regardless of streaming mode.

**Independent Test**: Use non-streaming model response and one-shot tool results, verify each EndEvent carries the same content as its corresponding Delta.

### Tests for User Story 2

- [X] T019 [P] [US2] Add non-streaming text EndEvent content test in `crates/agent_scope_agent/tests/react_agent_tests.rs`: complete text block → EndEvent.text equals delta content
- [X] T020 [P] [US2] Add non-streaming thinking EndEvent content test in `crates/agent_scope_agent/tests/react_agent_tests.rs`: complete thinking block → EndEvent.thinking equals delta content
- [X] T021 [P] [US2] Add non-streaming tool call EndEvent content test in `crates/agent_scope_agent/tests/react_agent_tests.rs`: one-shot tool call → EndEvent.input equals delta content
- [X] T022 [P] [US2] Add non-streaming tool result EndEvent content test in `crates/agent_scope_agent/tests/react_agent_tests.rs`: complete/success → EndEvent.output; error → output `None`

### Implementation for User Story 2

- [X] T023 [US2] Populate `TextBlockEndEvent.text` in non-streaming text path in `crates/agent_scope_agent/src/react_loop.rs`: pass `Some(tb.text.clone())` (around line 276)
- [X] T024 [US2] Populate `ThinkingBlockEndEvent.thinking` in non-streaming thinking path in `crates/agent_scope_agent/src/react_loop.rs`: pass `Some(thb.thinking.clone())` (around line 305)
- [X] T025 [US2] Populate `ToolCallEndEvent.input` in non-streaming tool call path in `crates/agent_scope_agent/src/react_loop.rs`: pass `Some(tc_mut.input.clone())` (around line 381)
- [X] T026 [US2] Populate `ToolResultEndEvent.output` in non-streaming tool result success path in `crates/agent_scope_agent/src/react_loop.rs`: pass `Some(output_text)` (around line 413)
- [X] T027 [US2] Set `ToolResultEndEvent.output = None` in non-streaming tool error path in `crates/agent_scope_agent/src/react_loop.rs` (around line 453)
- [X] T028 [P] [US2] Populate `TextBlockEndEvent.text` in `emit_events_from_response()` non-streaming text path in `crates/agent_scope_agent/src/streaming_reactor.rs`: pass `Some(tb.text.clone())` (around line 1160)
- [X] T029 [P] [US2] Populate `ThinkingBlockEndEvent.thinking` in `emit_events_from_response()` non-streaming thinking path in `crates/agent_scope_agent/src/streaming_reactor.rs`: pass `Some(thb.thinking.clone())` (around line 1188)
- [X] T030 [P] [US2] Populate `ToolCallEndEvent.input` in `process_response_and_continue()` complete tool call path in `crates/agent_scope_agent/src/streaming_reactor.rs`: pass `Some(tc.input.clone())` (around line 819)

**Checkpoint**: Non-streaming EndEvent content fields populated. US2 tests pass — consumers get consistent content across streaming and non-streaming modes.

---

## Phase 5: User Story 3 — Backward Compatibility & Lifecycle Stability (Priority: P2)

**Goal**: Existing consumers that rely only on event type/order/block-id are unaffected. Error/cancellation/empty-content cases are handled correctly.

**Independent Test**: Compare event sequences (types, order, counts, block IDs) before and after changes — must be identical except for new optional fields.

### Tests for User Story 3

- [X] T031 [P] [US3] Add event sequence order regression test in `crates/agent_scope_agent/tests/event_sequence_tests.rs`: verify Start → Delta → End order unchanged across text/thinking/tool-call/tool-result for both streaming and non-streaming
- [X] T032 [P] [US3] Add empty-content EndEvent test in `crates/agent_scope_agent/tests/streaming_tests.rs`: block with no delta content → EndEvent still emitted with `text: None` (not absent EndEvent)
- [X] T033 [P] [US3] Add cancellation-preserves-semantics test in `crates/agent_scope_agent/tests/interruption_tests.rs`: cancelled/interrupted path → ToolResultEndEvent has `output: None` and correct `state`, no false success content

### Implementation for User Story 3

- [X] T034 [US3] Verify cancellation path in `crates/agent_scope_agent/src/streaming_reactor.rs`: `ToolResultEndEvent` with `state = Interrupted` has `output: None` at line 1272
- [X] T035 [US3] Verify error path in `crates/agent_scope_agent/src/streaming_reactor.rs`: `ToolResultEndEvent` with `state = Error` has `output: None` at line 1335 (already correct with `None` default)

**Checkpoint**: Backward compatibility verified. Tests confirm event order, empty-content handling, and error/cancellation semantics are preserved.

---

## Phase 6: User Story 4 — Trace & Debug Enhancement (Priority: P3)

**Goal**: Event trace captures EndEvent complete-content fields, enabling tools to display block final content without delta reconstruction.

**Independent Test**: Record a trace with multiple block types, verify EndEvent entries carry the new fields and can reconstruct block-level output.

### Tests for User Story 4

- [X] T036 [P] [US4] Add trace capture test in `crates/agent_scope_agent/tests/streaming_tests.rs` or new `crates/agent_scope_agent/tests/trace_tests.rs`: record agent trace, verify EndEvent JSON contains content fields for text/thinking/tool-call/tool-result blocks
- [X] T037 [P] [US4] Add interleaved block content isolation test in `crates/agent_scope_agent/tests/streaming_tests.rs`: ≥10 interleaved blocks, each EndEvent contains only its own block's content

### Implementation for User Story 4

- [X] T038 [US4] Ensure existing trace/Debug implementations capture new optional fields — verify `Debug` derive on all four EndEvent structs in `crates/agent_scope_event/src/block_events.rs` and `crates/agent_scope_event/src/tool_events.rs`

**Checkpoint**: Trace captures EndEvent content. Interleaved block isolation verified.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Full workspace validation, example updates, documentation.

- [X] T039 Update `examples/chat.rs` if it directly constructs EndEvent structs — add `None` for new fields (compile fix, no behavior change)
- [X] T040 [P] Update `crates/agent_scope_event/tests/event_type_tests.rs` if any test validates EndEvent field counts or shapes
- [X] T041 Run `rtk cargo fmt` and `rtk cargo clippy --all-targets --all-features -- -D warnings`
- [X] T042 Run full workspace test suite: `rtk cargo test`
- [X] T043 [P] Verify all quickstart validation scenarios from `specs/014-end-event-content/quickstart.md`: serialization (scenario 1), non-streaming text/thinking (scenario 2), non-streaming tool (scenario 3), streaming accumulation (scenario 4), streaming tool result (scenario 5), interleaved blocks (scenario 6)
- [X] T044 [P] Document the EndEvent content extension in compatibility notes — note that new fields are optional and follow serde default/skip_none pattern

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Streaming content population — depends on Phase 2 struct extensions
- **User Story 2 (Phase 4)**: Non-streaming content population — depends on Phase 2 struct extensions; can run in parallel with US1
- **User Story 3 (Phase 5)**: Backward compatibility tests — depends on US1+US2 completion (to test the final state)
- **User Story 4 (Phase 6)**: Trace enhancement — depends on US1+US2 completion
- **Polish (Phase 7)**: Depends on all user stories being complete

### User Story Dependencies

- **US1 (P1)**: Can start after Phase 2 — No dependencies on other stories
- **US2 (P1)**: Can start after Phase 2 — No dependencies on US1. Parallelizable with US1.
- **US3 (P2)**: Depends on US1 + US2 — validates the combined result
- **US4 (P3)**: Depends on US1 + US2 — validates trace on populated EndEvents

### Within Each User Story

- Tests MUST be written and FAIL before implementation (T009-T012 before T013-T018; T019-T022 before T023-T030)
- Struct field additions (Phase 2) before agent-side population
- Implementation before validation (US3 tests after US1+US2)

### Parallel Opportunities

- All 4 struct field additions (T003-T006) can run in parallel
- US1 and US2 can run in parallel after Phase 2
- All tests within US1 (T009-T012) can run in parallel
- All tests within US2 (T019-T022) can run in parallel
- Non-streaming path tasks within US2 (T023-T027 for react_loop.rs vs T028-T030 for streaming_reactor.rs non-streaming paths) can run in parallel
- Polish tasks (T040, T041, T043, T044) can run in parallel

---

## Parallel Example: Phase 2 (Struct Extensions)

```text
Launch all four field additions in parallel:
  Task: "Add text field to TextBlockEndEvent in block_events.rs"
  Task: "Add thinking field to ThinkingBlockEndEvent in block_events.rs"
  Task: "Add input field to ToolCallEndEvent in tool_events.rs"
  Task: "Add output field to ToolResultEndEvent in tool_events.rs"
```

## Parallel Example: User Story 1 Tests

```text
Launch all four streaming tests together:
  Task: "Test streaming TextBlockEndEvent content in streaming_tests.rs"
  Task: "Test streaming ThinkingBlockEndEvent content in streaming_tests.rs"
  Task: "Test streaming ToolCallEndEvent content in streaming_tests.rs"
  Task: "Test streaming ToolResultEndEvent content in streaming_tests.rs"
```

---

## Implementation Strategy

### MVP First (Phase 2 + US1 Only)

1. Complete Phase 1: Setup (verify baseline)
2. Complete Phase 2: Add struct fields + serde tests (T003-T008)
3. Complete Phase 3: US1 streaming content population (T009-T018)
4. **STOP and VALIDATE**: Run streaming tests, verify EndEvent content in trace
5. This gives immediate value — consumers can read complete content from streaming EndEvents

### Incremental Delivery

1. Phase 1 + 2 → Protocol foundation ready
2. Add US1 → Streaming EndEvent content works → **MVP!**
3. Add US2 → Non-streaming parity → Full coverage
4. Add US3 → Backward compatibility verified
5. Add US4 → Trace enhancement
6. Phase 7 → All green: tests, clippy, fmt

### Recommended Execution

Given that US1 and US2 are both P1 and independently implementable:

1. Phase 2 (Foundational) — single dev, sequential
2. After Phase 2 checkpoint: US1 and US2 in parallel (two devs)
3. After US1+US2: US3 and US4 in parallel
4. Phase 7: Final sweep

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Tests MUST be written first and fail before implementing content population
- Commit after each phase or logical group
- Stop at any checkpoint to validate story independently
- All EndEvent structs already derive `Debug + Clone + Serialize + Deserialize` — no new derives needed
- `serde_json::Value` is already in scope in tool_events.rs; no new imports needed for the optional String fields
- The `examples/chat.rs` file may need updating if it constructs EndEvent structs directly; check after Phase 2
