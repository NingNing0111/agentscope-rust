# Tasks: SubAgent Collaboration

**Input**: Design documents from `/specs/020-subagent/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/, quickstart.md, constitution.md

**Tests**: This feature explicitly requires deterministic compatibility tests and trace validation. Test tasks below use scripted/mock agents and MUST NOT depend on live model output.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the SubAgent module structure and connect public exports without changing existing single-agent behavior.

- [X] T001 Add SubAgent module declarations for `subagent`, `delegation`, `context_policy`, `delegation_trace`, and `subagent_error` in `crates/agent_scope_agent/src/lib.rs`
- [X] T002 Create placeholder module files `crates/agent_scope_agent/src/subagent.rs`, `crates/agent_scope_agent/src/delegation.rs`, `crates/agent_scope_agent/src/context_policy.rs`, `crates/agent_scope_agent/src/delegation_trace.rs`, and `crates/agent_scope_agent/src/subagent_error.rs`
- [X] T003 [P] Add focused SubAgent test files `crates/agent_scope_agent/tests/subagent_template_tests.rs`, `crates/agent_scope_agent/tests/subagent_delegation_tests.rs`, `crates/agent_scope_agent/tests/multi_subagent_tests.rs`, `crates/agent_scope_agent/tests/subagent_error_tests.rs`, `crates/agent_scope_agent/tests/subagent_scope_tests.rs`, and `crates/agent_scope_agent/tests/subagent_trace_tests.rs`
- [X] T004 [P] Verify `agent_scope_agent` has required dependencies for SubAgent code (`serde`, `serde_json`, `thiserror`, `uuid`, `chrono`, `tokio`, `tokio-util`, `futures`, `async-trait`) in `crates/agent_scope_agent/Cargo.toml`
- [X] T005 [P] Add a deterministic scripted-agent test helper module in `crates/agent_scope_agent/tests/support/subagent_test_agent.rs`
- [X] T006 Wire the `tests/support/subagent_test_agent.rs` helper into each SubAgent integration test file that needs a mock `Agent`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Define stable data structures, typed errors, context policy, trace records, and deterministic test helpers used by every user story.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T007 Implement `SubAgentError`, `SubAgentErrorCategory`, stable error codes, `Display`, and `std::error::Error` in `crates/agent_scope_agent/src/subagent_error.rs`
- [X] T008 [P] Implement `TemplateStatus`, `SubAgentState`, `SelectionPolicy`, `DelegationReplyMode`, and `CollaborationStatus` enums with serde derives in `crates/agent_scope_agent/src/subagent.rs`
- [X] T009 [P] Implement `MessageContextPolicy`, `ResourceSharingPolicy`, `ModelAccessPolicy`, `SideEffectPolicy`, `CapabilityScope`, and `ContextSharingPolicy` in `crates/agent_scope_agent/src/context_policy.rs`
- [X] T010 [P] Implement `SharedContext`, context redaction notes, and message-preserving helpers in `crates/agent_scope_agent/src/context_policy.rs`
- [X] T011 [P] Implement `DelegationBudget` with default limits and budget validation in `crates/agent_scope_agent/src/delegation.rs`
- [X] T012 [P] Implement `SubAgentErrorInfo`, `SideEffectType`, `SideEffectRecord`, and redacted side-effect summaries in `crates/agent_scope_agent/src/delegation.rs`
- [X] T013 [P] Implement `DelegationEventType`, `DelegationEvent`, `DelegationTrace`, monotonic sequence appending, and terminal-event checks in `crates/agent_scope_agent/src/delegation_trace.rs`
- [X] T014 Implement safe summary and redaction helpers for delegation task summaries, result summaries, errors, and secret-like values in `crates/agent_scope_agent/src/delegation_trace.rs`
- [X] T015 Implement `SubAgentTemplate`, `SubAgent`, and validation methods for non-empty names/descriptions/instructions/capability scope in `crates/agent_scope_agent/src/subagent.rs`
- [X] T016 Implement `SubAgentRegistry` storage, normalized name lookup, duplicate detection, enable/disable, and list operations in `crates/agent_scope_agent/src/subagent.rs`
- [X] T017 Implement conversions or wrappers from existing `AgentError` categories into `SubAgentErrorCategory::ExecutionFailure`, `Timeout`, `Cancellation`, and `UnsupportedFeature` in `crates/agent_scope_agent/src/subagent_error.rs`
- [X] T018 Add public re-exports for SubAgent types, delegation types, context policy types, trace types, and errors in `crates/agent_scope_agent/src/lib.rs`
- [X] T019 [P] Add compile-time and serde round-trip tests for foundational enums and structs in `crates/agent_scope_agent/tests/subagent_template_tests.rs`
- [X] T020 [P] Add deterministic trace sequence and redaction unit tests in `crates/agent_scope_agent/tests/subagent_trace_tests.rs`

**Checkpoint**: Foundation ready - user story implementation can now begin in priority order or in parallel where marked.

---

## Phase 3: User Story 1 - Delegate a task to a SubAgent and receive its result (Priority: P1) 🎯 MVP

**Goal**: A primary agent can register one SubAgent, delegate a bounded task, receive an attributable result, and incorporate that result while preserving `Msg.name` speaker identity.

**Independent Test**: Create a primary agent with one configured scripted SubAgent, send a task requiring delegation, and verify the SubAgent receives the task, returns `CollaborationResult::Succeeded`, and the parent can observe the result message.

### Tests for User Story 1

- [X] T021 [P] [US1] Add template validation success/failure tests for required fields and `TemplateValidated` trace evidence in `crates/agent_scope_agent/tests/subagent_template_tests.rs`
- [X] T022 [P] [US1] Add registry tests for registering one SubAgent, listing it, looking it up by name, and preserving enabled state in `crates/agent_scope_agent/tests/subagent_template_tests.rs`
- [X] T023 [P] [US1] Add successful single delegation test using scripted `researcher` SubAgent in `crates/agent_scope_agent/tests/subagent_delegation_tests.rs`
- [X] T024 [P] [US1] Add parent observation test verifying successful SubAgent `Msg.name == "researcher"` is preserved when observed by parent in `crates/agent_scope_agent/tests/subagent_delegation_tests.rs`
- [X] T025 [P] [US1] Add trace order test for `DelegationRequested`, `SubAgentSelected`, `SubAgentStarted`, `SubAgentCompleted`, and `ResultObservedByParent` in `crates/agent_scope_agent/tests/subagent_trace_tests.rs`

### Implementation for User Story 1

- [X] T026 [US1] Implement `SubAgentTemplate::validate` and `SubAgentTemplate::create_subagent` behavior in `crates/agent_scope_agent/src/subagent.rs`
- [X] T027 [US1] Implement `SubAgentRegistry::register_template`, `register_subagent`, `get`, `list`, `enable`, and `disable` in `crates/agent_scope_agent/src/subagent.rs`
- [X] T028 [US1] Implement `DelegationRequest` and `CollaborationResult` data structures with validation in `crates/agent_scope_agent/src/delegation.rs`
- [X] T029 [US1] Implement final-only `delegate` orchestration that calls target `Agent::reply` with allowed `SharedContext` in `crates/agent_scope_agent/src/delegation.rs`
- [X] T030 [US1] Implement successful result conversion from SubAgent `Msg` to `CollaborationResult` while enforcing `message.name == subagent_name` in `crates/agent_scope_agent/src/delegation.rs`
- [X] T031 [US1] Implement parent observation helper for promoted successful SubAgent results using existing `Agent::observe` in `crates/agent_scope_agent/src/delegation.rs`
- [X] T032 [US1] Emit delegation lifecycle trace events for template validation, registration, request, selection, start, completion, and parent observation in `crates/agent_scope_agent/src/delegation_trace.rs`
- [X] T033 [US1] Ensure no existing `Agent::reply`, `Agent::reply_stream`, or `ReActAgent` behavior changes when no SubAgents are configured in `crates/agent_scope_agent/src/react_agent.rs`

**Checkpoint**: User Story 1 is independently functional as the MVP.

---

## Phase 4: User Story 2 - Coordinate multiple SubAgents in one parent task (Priority: P2)

**Goal**: A primary agent can coordinate multiple registered SubAgents with distinct responsibilities while keeping each SubAgent task, result, and speaker identity attributable.

**Independent Test**: Configure `researcher` and `writer`, delegate scoped subtasks to both, and verify each receives only its intended input and the parent combines both outputs coherently.

### Tests for User Story 2

- [X] T034 [P] [US2] Add two-SubAgent registration and lookup tests for `researcher` and `writer` in `crates/agent_scope_agent/tests/multi_subagent_tests.rs`
- [X] T035 [P] [US2] Add multi-delegation test verifying `researcher` and `writer` each receive distinct task text in `crates/agent_scope_agent/tests/multi_subagent_tests.rs`
- [X] T036 [P] [US2] Add multi-agent conversation test verifying user, parent, `researcher`, and `writer` messages preserve `Msg.name` in `crates/agent_scope_agent/tests/multi_subagent_tests.rs`
- [X] T037 [P] [US2] Add non-applicable SubAgent test verifying unselected collaborators are not invoked and no empty result is fabricated in `crates/agent_scope_agent/tests/multi_subagent_tests.rs`
- [X] T038 [P] [US2] Add `AlreadyStreaming` or concurrent-use guard test for delegating twice to the same busy SubAgent in `crates/agent_scope_agent/tests/multi_subagent_tests.rs`

### Implementation for User Story 2

- [X] T039 [US2] Implement `MultiAgentConversation`, `Participant`, and message attribution helpers in `crates/agent_scope_agent/src/delegation.rs`
- [X] T040 [US2] Implement explicit multi-target delegation API that accepts multiple `DelegationRequest` values and returns ordered `CollaborationResult` values in `crates/agent_scope_agent/src/delegation.rs`
- [X] T041 [US2] Implement deterministic sequential multi-SubAgent execution by default when `DelegationBudget.allow_concurrent == false` in `crates/agent_scope_agent/src/delegation.rs`
- [X] T042 [US2] Implement optional concurrent delegation path guarded by `DelegationBudget.allow_concurrent` and existing target-agent single-reply constraints in `crates/agent_scope_agent/src/delegation.rs`
- [X] T043 [US2] Implement `SelectionPolicy::ExplicitOnly`, `ResponsibilityMatch`, and `ManualApprovalRequired` error behavior for ambiguous or unconfirmed selections in `crates/agent_scope_agent/src/subagent.rs`
- [X] T044 [US2] Implement no-fabrication behavior for non-selected, missing, disabled, or ambiguous SubAgents in `crates/agent_scope_agent/src/delegation.rs`
- [X] T045 [US2] Add trace sequence support for multiple SubAgent invocations and out-of-order completion reconstruction in `crates/agent_scope_agent/src/delegation_trace.rs`

**Checkpoint**: User Stories 1 and 2 support single and multi-SubAgent collaboration independently.

---

## Phase 5: User Story 3 - Observe and debug SubAgent execution safely (Priority: P3)

**Goal**: SubAgent execution is observable through structured trace information for successful, failed, timed-out, cancelled, and redacted runs.

**Independent Test**: Run SubAgent scenarios with tracing enabled and verify parent start, SubAgent invocation, completion/failure, returned result, cancellation state, redaction notes, and parent response order.

### Tests for User Story 3

- [X] T046 [P] [US3] Add execution failure test mapping scripted SubAgent error to `CollaborationStatus::Failed` and category `ExecutionFailure` in `crates/agent_scope_agent/tests/subagent_error_tests.rs`
- [X] T047 [P] [US3] Add timeout test mapping delayed scripted SubAgent to `CollaborationStatus::TimedOut` and `SubAgentTimedOut` trace event in `crates/agent_scope_agent/tests/subagent_error_tests.rs`
- [X] T048 [P] [US3] Add parent cancellation test verifying active SubAgent work stops within the configured cancellation window in `crates/agent_scope_agent/tests/subagent_error_tests.rs`
- [X] T049 [P] [US3] Add unsupported distributed/app-service pattern test returning `UnsupportedFeature` instead of no-op success in `crates/agent_scope_agent/tests/subagent_error_tests.rs`
- [X] T050 [P] [US3] Add trace redaction test with credential-like delegated task and tool argument values in `crates/agent_scope_agent/tests/subagent_trace_tests.rs`
- [X] T051 [P] [US3] Add stream delegation test verifying forwarded SubAgent `AgentEvent` records remain correlated and distinguishable from parent events in `crates/agent_scope_agent/tests/subagent_trace_tests.rs`

### Implementation for User Story 3

- [X] T052 [US3] Implement timeout handling for SubAgent delegation using `DelegationBudget.timeout_ms` in `crates/agent_scope_agent/src/delegation.rs`
- [X] T053 [US3] Implement parent cancellation propagation to active SubAgent delegation using existing cancellation semantics in `crates/agent_scope_agent/src/delegation.rs`
- [X] T054 [US3] Implement `delegate_stream` API returning correlated delegation events and terminal `CollaborationResult` in `crates/agent_scope_agent/src/delegation.rs`
- [X] T055 [US3] Implement forwarding and correlation of SubAgent `AgentEvent` records as `SubAgentEventForwarded` trace entries in `crates/agent_scope_agent/src/delegation_trace.rs`
- [X] T056 [US3] Implement terminal trace validation ensuring every delegation ends with exactly one terminal event in `crates/agent_scope_agent/src/delegation_trace.rs`
- [X] T057 [US3] Implement redacted user-facing diagnostics for all `SubAgentErrorInfo` values in `crates/agent_scope_agent/src/subagent_error.rs`
- [X] T058 [US3] Implement explicit `UnsupportedFeature` helpers for remote worker, durable queue, cross-host migration, and full app-service dispatch requests in `crates/agent_scope_agent/src/delegation.rs`

**Checkpoint**: User Stories 1–3 provide debuggable, traceable, and safe SubAgent lifecycle behavior.

---

## Phase 6: User Story 4 - Preserve context and resource boundaries between parent and SubAgents (Priority: P4)

**Goal**: Each SubAgent receives only explicitly shared context and capabilities, and side effects are attributed and scoped.

**Independent Test**: Configure a SubAgent with limited context and capability scope, delegate a task, and verify it cannot observe or act on data outside its assigned scope.

### Tests for User Story 4

- [X] T059 [P] [US4] Add context policy tests for `None`, `SummaryOnly`, `Selected`, and explicitly enabled `Full` message sharing in `crates/agent_scope_agent/tests/subagent_scope_tests.rs`
- [X] T060 [P] [US4] Add capability denial tests for tool, memory, session, workspace, and sandbox policies returning `PermissionDenied` in `crates/agent_scope_agent/tests/subagent_scope_tests.rs`
- [X] T061 [P] [US4] Add side-effect attribution tests for memory/session/workspace/tool/model records in `crates/agent_scope_agent/tests/subagent_scope_tests.rs`
- [X] T062 [P] [US4] Add delegation budget tests for max depth, max calls, max context messages, and budget exceeded errors in `crates/agent_scope_agent/tests/subagent_scope_tests.rs`
- [X] T063 [P] [US4] Add secret-safety tests verifying default trace and errors contain no raw secret values in `crates/agent_scope_agent/tests/subagent_trace_tests.rs`

### Implementation for User Story 4

- [X] T064 [US4] Implement `ContextSharingPolicy::build_shared_context` for none, summary-only, selected, and full message policies in `crates/agent_scope_agent/src/context_policy.rs`
- [X] T065 [US4] Implement capability checks for tool, memory, session, workspace, sandbox, model access, and side-effect permissions in `crates/agent_scope_agent/src/context_policy.rs`
- [X] T066 [US4] Integrate capability checks into delegation before invoking a SubAgent in `crates/agent_scope_agent/src/delegation.rs`
- [X] T067 [US4] Implement `SideEffectRecord` collection and parent-promotion behavior for SubAgent outputs in `crates/agent_scope_agent/src/delegation.rs`
- [X] T068 [US4] Implement delegation depth, call count, and context-size budget enforcement in `crates/agent_scope_agent/src/delegation.rs`
- [X] T069 [US4] Emit `ScopeDenied` and budget-related trace events with redacted summaries in `crates/agent_scope_agent/src/delegation_trace.rs`

**Checkpoint**: All user stories are independently functional with safe context and resource boundaries.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, compatibility matrix, examples, formatting, linting, and final validation evidence.

- [X] T070 [P] Update English Agent documentation with SubAgent concepts, API examples, scope boundaries, and unsupported distributed patterns in `docs/en/modules/agent.md`
- [X] T071 [P] Update Chinese Agent documentation with SubAgent concepts, API examples, scope boundaries, and unsupported distributed patterns in `docs/zh/modules/agent.md`
- [X] T072 [P] Add or update a SubAgent section in `examples/agent-demo/README.md` describing how the demo will expose SubAgent once implemented
- [X] T073 Update `specs/001-compatibility-baseline/capability-matrix.json` for `app-sub-agent-template`, app-service/message-bus deferred patterns, and multi-agent formatter parity status
- [X] T074 Add quickstart validation evidence and any implementation-specific command adjustments to `specs/020-subagent/quickstart.md`
- [X] T075 [P] Run `rtk cargo fmt --check` and fix formatting issues in `crates/agent_scope_agent/src/*.rs` and `crates/agent_scope_agent/tests/*.rs`
- [X] T076 [P] Run `rtk cargo check --workspace` and fix compile errors related to SubAgent exports or tests
- [X] T077 [P] Run `rtk cargo test -p agent_scope_agent subagent` and fix SubAgent-specific test failures
- [X] T078 Run `rtk cargo test --workspace` and fix regressions in existing single-agent, streaming, memory, session, workspace, and sandbox tests
- [X] T079 Run `rtk cargo clippy --workspace --all-targets -- -D warnings` and fix all warnings in SubAgent code and tests
- [X] T080 Verify all FR-001 through FR-028 and SC-001 through SC-008 from `specs/020-subagent/spec.md` have implementation or documented unsupported/deferred evidence in `specs/020-subagent/quickstart.md`
- [X] T081 Update `specs/020-subagent/tasks.md` checkboxes as tasks complete and record final validation evidence in `specs/020-subagent/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational; delivers MVP single SubAgent delegation
- **User Story 2 (Phase 4)**: Depends on Foundational and benefits from US1 delegation primitives; can start once registry/request/result types stabilize
- **User Story 3 (Phase 5)**: Depends on Foundational and core delegation lifecycle from US1; streaming/failure tests can start once trace types stabilize
- **User Story 4 (Phase 6)**: Depends on Foundational and integrates with US1 delegation; policy tests can start once context policy types exist
- **Polish (Phase 7)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2); no dependency on other stories; MVP scope
- **User Story 2 (P2)**: Can start after Foundational (Phase 2), but final multi-delegation behavior depends on US1 request/result orchestration
- **User Story 3 (P3)**: Can start after Foundational (Phase 2), but timeout/cancellation/streaming behavior depends on US1 delegation lifecycle
- **User Story 4 (P4)**: Can start after Foundational (Phase 2), but final enforcement depends on US1 invoking through the policy boundary

### Within Each User Story

- Tests should be written first and fail before implementation
- Data structures and validation before orchestration
- Registry/selection before delegation
- Delegation lifecycle before stream forwarding
- Policy checks before side-effect promotion
- Trace terminal outcome checks before final documentation

### Parallel Opportunities

- T003, T004, and T005 can run in parallel after T001/T002
- T008, T009, T010, T011, T012, and T013 can run in parallel after module files exist
- T019 and T020 can run in parallel after foundational structures compile
- T021 through T025 can run in parallel because they target different test scenarios/files
- T034 through T038 can run in parallel for US2 test coverage
- T046 through T051 can run in parallel for US3 test coverage
- T059 through T063 can run in parallel for US4 policy and secret-safety tests
- T070, T071, and T072 can run in parallel after public API shape stabilizes
- T075, T076, and T077 can run in parallel only if fix ownership is coordinated to avoid conflicting edits

---

## Parallel Example: User Story 1

```bash
# Write independent failing tests first:
Task: "Add template validation success/failure tests in crates/agent_scope_agent/tests/subagent_template_tests.rs"
Task: "Add successful single delegation test in crates/agent_scope_agent/tests/subagent_delegation_tests.rs"
Task: "Add trace order test in crates/agent_scope_agent/tests/subagent_trace_tests.rs"

# Then implement independent modules with coordination:
Task: "Implement SubAgentTemplate and SubAgentRegistry in crates/agent_scope_agent/src/subagent.rs"
Task: "Implement DelegationRequest and CollaborationResult in crates/agent_scope_agent/src/delegation.rs"
Task: "Emit delegation lifecycle trace events in crates/agent_scope_agent/src/delegation_trace.rs"
```

---

## Parallel Example: User Story 2

```bash
# Independent test lanes:
Task: "Add two-SubAgent registration and lookup tests in crates/agent_scope_agent/tests/multi_subagent_tests.rs"
Task: "Add multi-agent conversation speaker identity test in crates/agent_scope_agent/tests/multi_subagent_tests.rs"
Task: "Add concurrent-use guard test in crates/agent_scope_agent/tests/multi_subagent_tests.rs"

# Implementation lanes after US1 primitives stabilize:
Task: "Implement MultiAgentConversation in crates/agent_scope_agent/src/delegation.rs"
Task: "Implement SelectionPolicy behavior in crates/agent_scope_agent/src/subagent.rs"
Task: "Add multi-SubAgent trace sequence support in crates/agent_scope_agent/src/delegation_trace.rs"
```

---

## Parallel Example: User Story 3

```bash
# Error and trace tests can be authored independently:
Task: "Add execution failure test in crates/agent_scope_agent/tests/subagent_error_tests.rs"
Task: "Add timeout test in crates/agent_scope_agent/tests/subagent_error_tests.rs"
Task: "Add trace redaction test in crates/agent_scope_agent/tests/subagent_trace_tests.rs"
Task: "Add stream delegation correlation test in crates/agent_scope_agent/tests/subagent_trace_tests.rs"
```

---

## Parallel Example: User Story 4

```bash
# Policy tests are independent after context types exist:
Task: "Add context policy tests in crates/agent_scope_agent/tests/subagent_scope_tests.rs"
Task: "Add capability denial tests in crates/agent_scope_agent/tests/subagent_scope_tests.rs"
Task: "Add delegation budget tests in crates/agent_scope_agent/tests/subagent_scope_tests.rs"
Task: "Add secret-safety tests in crates/agent_scope_agent/tests/subagent_trace_tests.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Run `rtk cargo test -p agent_scope_agent subagent_template` and `rtk cargo test -p agent_scope_agent subagent_delegation`
5. Confirm one primary agent can delegate to one SubAgent and observe an attributable result

### Incremental Delivery

1. Complete Setup + Foundational → stable data structures, errors, policy primitives, and trace records
2. Add User Story 1 → single SubAgent MVP → validate independently
3. Add User Story 2 → multiple SubAgents and speaker identity → validate independently
4. Add User Story 3 → trace, timeout, cancellation, failure, unsupported paths → validate independently
5. Add User Story 4 → context/capability boundaries and side-effect attribution → validate independently
6. Run Phase 7 quality gates and update compatibility matrix before marking Feature 020 complete

### Parallel Team Strategy

With multiple implementers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: US1 single delegation MVP
   - Developer B: US2 multi-SubAgent coordination and conversation identity
   - Developer C: US3 trace/error/timeout/cancellation behavior
   - Developer D: US4 context/capability policy enforcement
3. Integrate through shared `subagent.rs`, `delegation.rs`, `context_policy.rs`, and `delegation_trace.rs`, then run Phase 7 quality gates

---

## Notes

- [P] tasks = different files or independently authorable tests, no dependency on incomplete same-file edits
- [Story] label maps task to the user story from `specs/020-subagent/spec.md`
- Tests are included because the feature requires deterministic compatibility traces and explicit validation scenarios
- Deterministic scripted/mock agents must be used for acceptance tests; live model output is not sufficient
- `Msg.name` is the required speaker identity field for all SubAgent-authored messages
- Unsupported distributed/app-service/message-bus/provider-formatter patterns must return stable `UnsupportedFeature` or be recorded as deferred in the compatibility matrix
- Existing single-agent behavior must remain unchanged when no SubAgents are configured
