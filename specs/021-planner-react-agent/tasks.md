# Tasks: Planner + ReActAgent Compatibility

**Input**: Design documents from `/specs/021-planner-react-agent/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Test tasks are included because the specification and quickstart explicitly require deterministic compatibility, event ordering, cancellation, error, regression, and Python-vs-Rust trace validation.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare module boundaries and compatibility targets without changing runtime behavior.

- [x] T001 Add planner module declarations and public re-export placeholders in `crates/agent_scope_agent/src/lib.rs`
- [x] T002 [P] Create empty planner module file `crates/agent_scope_agent/src/planner.rs`
- [x] T003 [P] Create empty plan data module file `crates/agent_scope_agent/src/plan.rs`
- [x] T004 [P] Create empty planning trace module file `crates/agent_scope_agent/src/planning_trace.rs`
- [x] T005 [P] Create empty planner error module file `crates/agent_scope_agent/src/planner_error.rs`
- [x] T006 [P] Create empty planner stream helper module file `crates/agent_scope_agent/src/planner_stream.rs`
- [x] T007 [P] Create planner test helper scaffold in `crates/agent_scope_agent/tests/planner_mocks.rs`
- [x] T008 Add Feature 021 supported/deferred capability placeholders in `specs/001-compatibility-baseline/capability-matrix.json`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core data, error, trace, event, and helper infrastructure that MUST be complete before any user story implementation.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T009 Implement `PlanStatus`, `PlanStepStatus`, `PlanRevisionTrigger`, and `PlannerOutcome` enums in `crates/agent_scope_agent/src/plan.rs`
- [x] T010 Implement `PlannedTask`, `Plan`, `PlanStep`, and `PlanRevision` structs with serde support in `crates/agent_scope_agent/src/plan.rs`
- [x] T011 Implement validation methods for goal, plan, step uniqueness, terminal states, and revision consistency in `crates/agent_scope_agent/src/plan.rs`
- [x] T012 Implement `PlannerErrorCategory` and `PlannerError` with stable categories in `crates/agent_scope_agent/src/planner_error.rs`
- [x] T013 Implement conversions between planner errors and existing `AgentError` categories in `crates/agent_scope_agent/src/planner_error.rs`
- [x] T014 Implement `PlanningEventType`, `PlanningEvent`, and `PlanningTrace` structs with serde support in `crates/agent_scope_agent/src/planning_trace.rs`
- [x] T015 Implement monotonic sequence append and boundary validation helpers in `crates/agent_scope_agent/src/planning_trace.rs`
- [x] T016 Implement trace redaction helpers for summaries, tool arguments, and metadata in `crates/agent_scope_agent/src/planning_trace.rs`
- [x] T017 Implement `PlannerConfig` with max steps, max replans, per-step iteration, timeout, and redaction policy fields in `crates/agent_scope_agent/src/planner.rs`
- [x] T018 Implement `PlannerConfig` validation and defaults in `crates/agent_scope_agent/src/planner.rs`
- [x] T019 [P] Add serde round-trip tests for plan entities in `crates/agent_scope_agent/tests/planner_plan_tests.rs`
- [x] T020 [P] Add validation tests for invalid goal, duplicate step IDs, empty plan, terminal transitions, and revision consistency in `crates/agent_scope_agent/tests/planner_plan_tests.rs`
- [x] T021 [P] Add planner error category and conversion tests in `crates/agent_scope_agent/tests/planner_error_tests.rs`
- [x] T022 [P] Add planning trace sequence, boundary validation, and redaction tests in `crates/agent_scope_agent/tests/planner_trace_tests.rs`
- [x] T023 Wire planner modules into `crates/agent_scope_agent/src/lib.rs` after foundational tests compile

**Checkpoint**: Foundation ready — user story implementation can now begin in parallel.

---

## Phase 3: User Story 1 - Plan and Execute a Multi-Step Task (Priority: P1) 🎯 MVP

**Goal**: A developer can submit a multi-step goal, receive an explicit ordered plan, execute each step through ReActAgent behavior, and inspect the final answer plus trace.

**Independent Test**: Run a deterministic three-step planned task with scripted planner/ReAct responses and fixed tools; verify plan creation, ordered step execution, tool activity correlation, final `Completed` outcome, and redacted trace.

### Tests for User Story 1

- [x] T024 [P] [US1] Add successful planned task integration test in `crates/agent_scope_agent/tests/planner_execution_tests.rs`
- [x] T025 [P] [US1] Add tool-using plan step integration test in `crates/agent_scope_agent/tests/planner_execution_tests.rs`
- [x] T026 [P] [US1] Add non-streaming planner API contract test in `crates/agent_scope_agent/tests/planner_execution_tests.rs`
- [x] T027 [P] [US1] Add redacted final trace assertion for successful execution in `crates/agent_scope_agent/tests/planner_trace_tests.rs`

### Implementation for User Story 1

- [x] T028 [US1] Implement `Planner` struct and constructor around a ReAct-capable agent in `crates/agent_scope_agent/src/planner.rs`
- [x] T029 [US1] Implement deterministic plan generation response parsing into `Plan` in `crates/agent_scope_agent/src/planner.rs`
- [x] T030 [US1] Implement non-streaming `run` planned task entry point in `crates/agent_scope_agent/src/planner.rs`
- [x] T031 [US1] Implement ordered step execution loop using existing ReActAgent reply behavior in `crates/agent_scope_agent/src/planner.rs`
- [x] T032 [US1] Implement step-to-ReAct event and tool activity correlation in `crates/agent_scope_agent/src/planning_trace.rs`
- [x] T033 [US1] Implement final `PlannerOutcome::Completed` and completed step trace emission in `crates/agent_scope_agent/src/planner.rs`
- [x] T034 [US1] Enforce max step limit during non-streaming execution in `crates/agent_scope_agent/src/planner.rs`
- [x] T035 [US1] Add public exports for `Planner`, `PlannerConfig`, `PlannedTask`, `Plan`, `PlanStep`, `PlanningTrace`, and `PlannerOutcome` in `crates/agent_scope_agent/src/lib.rs`
- [x] T036 [US1] Update root crate re-exports for planner public types in `src/lib.rs`

**Checkpoint**: User Story 1 is fully functional and independently testable as the MVP.

---

## Phase 4: User Story 2 - Revise a Plan When Execution Changes the Situation (Priority: P2)

**Goal**: The agent explicitly replans after recoverable failures or new information, preserving original failed/skipped/replaced steps and revision rationale.

**Independent Test**: Run a deterministic task where the first tool step fails recoverably; verify failed step reason, `PlanRevision`, replacement plan version, replanning limit behavior, and final outcome.

### Tests for User Story 2

- [x] T037 [P] [US2] Add recoverable failure replanning test in `crates/agent_scope_agent/tests/planner_replan_tests.rs`
- [x] T038 [P] [US2] Add obsolete step skipped-with-reason test in `crates/agent_scope_agent/tests/planner_replan_tests.rs`
- [x] T039 [P] [US2] Add replanning limit exceeded test in `crates/agent_scope_agent/tests/planner_replan_tests.rs`
- [x] T040 [P] [US2] Add revision history preservation trace test in `crates/agent_scope_agent/tests/planner_trace_tests.rs`

### Implementation for User Story 2

- [x] T041 [US2] Implement recoverable step failure classification in `crates/agent_scope_agent/src/planner.rs`
- [x] T042 [US2] Implement replanning prompt/request construction and response parsing in `crates/agent_scope_agent/src/planner.rs`
- [x] T043 [US2] Implement `PlanRevision` creation and superseded plan preservation in `crates/agent_scope_agent/src/plan.rs`
- [x] T044 [US2] Implement skipped obsolete step handling with reason in `crates/agent_scope_agent/src/planner.rs`
- [x] T045 [US2] Implement replanning lifecycle events in `crates/agent_scope_agent/src/planning_trace.rs`
- [x] T046 [US2] Enforce replanning attempt limit and `ReplanLimitExceeded` error in `crates/agent_scope_agent/src/planner.rs`
- [x] T047 [US2] Implement `PlannerOutcome::PartiallyCompleted` and failure summary generation in `crates/agent_scope_agent/src/planner.rs`

**Checkpoint**: User Stories 1 and 2 both work independently; replanning is explicit and auditable.

---

## Phase 5: User Story 3 - Use Planner and ReActAgent in Streaming Applications (Priority: P3)

**Goal**: Streaming consumers can observe plan creation, step lifecycle, ReAct events, replanning, cancellation boundaries, and final task outcome in stable chronological order.

**Independent Test**: Run a deterministic planned task through the streaming API and verify ordered planning + ReAct lifecycle events, task terminal event, backpressure behavior, and cancellation handling.

### Tests for User Story 3

- [x] T048 [P] [US3] Add successful streaming planned task event order test in `crates/agent_scope_agent/tests/planner_stream_tests.rs`
- [x] T049 [P] [US3] Add streaming tool step correlation test in `crates/agent_scope_agent/tests/planner_stream_tests.rs`
- [x] T050 [P] [US3] Add streaming replanning event order test in `crates/agent_scope_agent/tests/planner_stream_tests.rs`
- [x] T051 [P] [US3] Add cancellation during planning, step execution, and replanning tests in `crates/agent_scope_agent/tests/planner_cancel_tests.rs`
- [x] T052 [P] [US3] Add planner event serde tests in `crates/agent_scope_event/tests/event_serde_tests.rs` if new AgentEvent variants are added
- [x] T053 [P] [US3] Add planner event sequence tests in `crates/agent_scope_agent/tests/planner_stream_tests.rs`

### Implementation for User Story 3

- [x] T054 [US3] Decide and implement additive planning event representation in `crates/agent_scope_event/src/lib.rs`
- [x] T055 [US3] Implement conversion from `PlanningEvent` to public stream events in `crates/agent_scope_agent/src/planner_stream.rs`
- [x] T056 [US3] Implement streaming `run_stream` planned task entry point in `crates/agent_scope_agent/src/planner.rs`
- [x] T057 [US3] Implement bounded stream delivery and backpressure handling in `crates/agent_scope_agent/src/planner_stream.rs`
- [x] T058 [US3] Interleave planning lifecycle events with existing ReActAgent events in `crates/agent_scope_agent/src/planner_stream.rs`
- [x] T059 [US3] Implement cancellation propagation through planning, step execution, and replanning in `crates/agent_scope_agent/src/planner.rs`
- [x] T060 [US3] Preserve existing single-active-reply or `AlreadyStreaming` guard behavior during planner streaming in `crates/agent_scope_agent/src/planner.rs`

**Checkpoint**: Streaming planned tasks are independently observable and cancellable without breaking ReActAgent event order.

---

## Phase 6: User Story 4 - Preserve Python AgentScope Compatibility Evidence (Priority: P4)

**Goal**: Maintainers can prove supported Planner + ReActAgent behavior against Python AgentScope reference traces and clearly document unsupported/deferred capabilities.

**Independent Test**: Run at least five deterministic Python reference scenarios and equivalent Rust scenarios; compare normalized traces for event order, step transitions, tool activity, errors, cancellation state, final outcome, and unsupported markers.

### Tests for User Story 4

- [x] T061 [P] [US4] Add Python fixture generation cases for successful planned task in `tests/compatibility/generate_fixtures.py`
- [x] T062 [P] [US4] Add Python fixture generation cases for tool step and replanning in `tests/compatibility/generate_fixtures.py`
- [x] T063 [P] [US4] Add Python fixture generation cases for cancellation and unsupported capability in `tests/compatibility/generate_fixtures.py`
- [x] T064 [P] [US4] Add Rust normalized trace comparison test for successful planned task in `crates/agent_scope_agent/tests/planner_compatibility_tests.rs`
- [x] T065 [P] [US4] Add Rust normalized trace comparison tests for tool step, replanning, cancellation, and unsupported capability in `crates/agent_scope_agent/tests/planner_compatibility_tests.rs`
- [x] T066 [P] [US4] Add existing non-planning ReActAgent regression tests in `crates/agent_scope_agent/tests/planner_regression_tests.rs`

### Implementation for User Story 4

- [x] T067 [US4] Extend compatibility trace normalization rules for planner IDs and timestamps in `specs/001-compatibility-baseline/normalization-rules.json`
- [x] T068 [US4] Extend compatibility trace schema documentation for planner sections in `specs/001-compatibility-baseline/trace-schema.json`
- [x] T069 [US4] Update planner supported/deferred/deviation entries in `specs/001-compatibility-baseline/capability-matrix.json`
- [x] T070 [US4] Implement unsupported capability detection and `UnsupportedCapability` outcomes in `crates/agent_scope_agent/src/planner.rs`
- [x] T071 [US4] Implement trace export helpers for compatibility fixtures in `crates/agent_scope_agent/src/planning_trace.rs`
- [x] T072 [US4] Document Python-vs-Rust trace comparison process in `specs/021-planner-react-agent/quickstart.md`

**Checkpoint**: Supported behavior has compatibility evidence; unsupported behavior is explicit and documented.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, examples, validation, cleanup, and release-gate work that affects multiple user stories.

- [x] T073 [P] Update English agent documentation with Planner usage, streaming, replanning, errors, and unsupported scope in `docs/en/modules/agent.md`
- [x] T074 [P] Update Chinese agent documentation with Planner usage, streaming, replanning, errors, and unsupported scope in `docs/zh/modules/agent.md`
- [x] T075 [P] Update agent demo documentation with an optional Planner scenario in `examples/agent-demo/README.md`
- [x] T076 Add optional Planner example wiring to agent demo only if it compiles deterministically in `examples/agent-demo/main.rs`
- [x] T077 [P] Add public API documentation comments for planner types in `crates/agent_scope_agent/src/planner.rs`
- [x] T078 [P] Add public API documentation comments for plan data types in `crates/agent_scope_agent/src/plan.rs`
- [x] T079 [P] Add public API documentation comments for planning trace and errors in `crates/agent_scope_agent/src/planning_trace.rs`
- [x] T080 [P] Add public API documentation comments for planner errors in `crates/agent_scope_agent/src/planner_error.rs`
- [x] T081 Run `rtk cargo test -p agent_scope_agent planner` and fix failures in `crates/agent_scope_agent/src/`
- [x] T082 Run `rtk cargo test -p agent_scope_event` and fix failures in `crates/agent_scope_event/src/lib.rs`
- [x] T083 Run `rtk cargo test --workspace` and fix workspace regressions in affected crates
- [x] T084 Run `rtk cargo check --workspace` and fix compile errors in affected crates
- [x] T085 Run `rtk cargo clippy --workspace --all-targets -- -D warnings` and fix warnings in affected crates
- [x] T086 Run `rtk cargo fmt --check` and format changed Rust files
- [x] T087 Verify quickstart scenarios and update validation notes in `specs/021-planner-react-agent/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies; can start immediately.
- **Foundational (Phase 2)**: Depends on Setup completion; blocks all user stories.
- **User Story 1 (Phase 3)**: Depends on Foundation; MVP scope.
- **User Story 2 (Phase 4)**: Depends on Foundation and benefits from US1 execution loop; can be tested independently with scripted failed steps.
- **User Story 3 (Phase 5)**: Depends on Foundation and US1 lifecycle semantics; can be developed after the non-streaming model is stable.
- **User Story 4 (Phase 6)**: Depends on supported behavior from US1–US3; compatibility fixtures can begin earlier but final comparison requires implemented traces.
- **Polish (Phase 7)**: Depends on desired user stories being complete.

### User Story Dependencies

- **US1 (P1)**: Start after Phase 2; no dependency on other stories; MVP.
- **US2 (P2)**: Start after Phase 2 and the basic US1 execution entry point; independent tests use recoverable failure fixtures.
- **US3 (P3)**: Start after Phase 2 and basic planner lifecycle events; independent tests use streaming fixtures.
- **US4 (P4)**: Fixture preparation can start after Phase 2; final trace comparison depends on US1–US3 behavior.

### Within Each User Story

- Tests are written first and should fail before implementation.
- Data structures and errors before orchestration.
- Orchestration before streaming.
- Trace correlation before compatibility comparison.
- Documentation and compatibility matrix updates before final validation.

### Parallel Opportunities

- Setup scaffold files T002–T007 can run in parallel after T001 planning.
- Foundational tests T019–T022 can run in parallel after corresponding foundational APIs are sketched.
- US1 tests T024–T027 can run in parallel.
- US2 tests T037–T040 can run in parallel.
- US3 tests T048–T053 can run in parallel.
- US4 fixture/test tasks T061–T066 can run in parallel by file group.
- Documentation tasks T073–T075 and doc-comment tasks T077–T080 can run in parallel.

---

## Parallel Example: User Story 1

```bash
# Launch independent US1 test authoring tasks together:
Task: "Add successful planned task integration test in crates/agent_scope_agent/tests/planner_execution_tests.rs"
Task: "Add redacted final trace assertion in crates/agent_scope_agent/tests/planner_trace_tests.rs"

# Launch implementation after tests are in place:
Task: "Implement Planner struct and constructor in crates/agent_scope_agent/src/planner.rs"
Task: "Implement step-to-ReAct event correlation in crates/agent_scope_agent/src/planning_trace.rs"
```

## Parallel Example: User Story 2

```bash
Task: "Add recoverable failure replanning test in crates/agent_scope_agent/tests/planner_replan_tests.rs"
Task: "Add revision history preservation trace test in crates/agent_scope_agent/tests/planner_trace_tests.rs"
Task: "Implement PlanRevision creation in crates/agent_scope_agent/src/plan.rs"
```

## Parallel Example: User Story 3

```bash
Task: "Add streaming event order test in crates/agent_scope_agent/tests/planner_stream_tests.rs"
Task: "Add cancellation tests in crates/agent_scope_agent/tests/planner_cancel_tests.rs"
Task: "Implement bounded stream delivery in crates/agent_scope_agent/src/planner_stream.rs"
```

## Parallel Example: User Story 4

```bash
Task: "Add Python fixture generation cases in tests/compatibility/generate_fixtures.py"
Task: "Add Rust normalized trace comparison tests in crates/agent_scope_agent/tests/planner_compatibility_tests.rs"
Task: "Update capability matrix in specs/001-compatibility-baseline/capability-matrix.json"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational data/error/trace/config infrastructure.
3. Complete Phase 3: US1 non-streaming planned task execution.
4. Stop and validate with `rtk cargo test -p agent_scope_agent planner_execution` and `rtk cargo test -p agent_scope_agent planner_trace`.
5. Confirm existing non-planning ReActAgent tests still pass before moving to replanning or streaming.

### Incremental Delivery

1. Setup + Foundation → planner types, validation, errors, trace contracts compile.
2. US1 → basic planned task MVP with final trace.
3. US2 → explicit replanning and partial/failure outcomes.
4. US3 → streaming lifecycle and cancellation.
5. US4 → compatibility fixtures, matrix updates, and trace diff evidence.
6. Polish → docs, examples, full workspace validation.

### Parallel Team Strategy

With multiple implementers:

1. One implementer owns foundational planner data/error modules.
2. One implementer owns trace/events/redaction modules.
3. One implementer owns tests and scripted fixtures.
4. After Phase 2, split US1 orchestration, US2 replanning, US3 streaming, and US4 compatibility fixtures by file groups.

---

## Notes

- `[P]` tasks use different files or can proceed without depending on incomplete tasks.
- `[US#]` labels map directly to user stories in `specs/021-planner-react-agent/spec.md`.
- All implementation tasks include exact file paths.
- Tests are intentionally included because compatibility, event ordering, and cancellation behavior are release gates.
- Do not claim full Python parity; deferred capabilities must remain explicit in the compatibility matrix.
- Preserve existing ReActAgent behavior when Planner is not enabled.
