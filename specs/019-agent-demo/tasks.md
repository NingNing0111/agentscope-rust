# Tasks: Complete Agent Demo

**Input**: Design documents from `/specs/019-agent-demo/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/, quickstart.md, constitution.md

**Tests**: This feature explicitly requires deterministic validation instructions and maintainable regression evidence. Test tasks below focus on build/run validation, CLI error paths, trace contract checks, and secret-safety checks rather than adding production crate tests.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the canonical example structure and register the executable entry point.

- [X] T001 Create `examples/agent-demo/` directory structure with `fixtures/` subdirectory and placeholder module files in `examples/agent-demo/main.rs`, `examples/agent-demo/scenario.rs`, `examples/agent-demo/deterministic.rs`, `examples/agent-demo/live.rs`, `examples/agent-demo/tools.rs`, `examples/agent-demo/trace.rs`, and `examples/agent-demo/middleware.rs`
- [X] T002 Register the directory example by adding `[[example]] name = "agent_demo" path = "examples/agent-demo/main.rs"` to `Cargo.toml`
- [X] T003 [P] Add initial demo README skeleton with setup, run modes, expected output, coverage checklist, limitations, and troubleshooting headings in `examples/agent-demo/README.md`
- [X] T004 [P] Add deterministic fixture notes or expected summary fixture for the canonical walkthrough in `examples/agent-demo/fixtures/expected_summary.md`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Define shared CLI, trace, coverage, scenario, and error foundations that all user stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T005 Define `RunMode`, `Cli`, and option parsing for `--mode`, `--api-key`, `--model`, `--workspace-dir`, `--trace-json`, `--show-coverage`, `--fail-tool`, `--cancel-after-step`, and `--verbose` in `examples/agent-demo/main.rs`
- [X] T006 Implement `DemoError` with stable categories and exit-code mapping for `config_error`, `tool_error`, `model_error`, `unsupported_feature`, `cancelled`, and `internal_error` in `examples/agent-demo/main.rs`
- [X] T007 [P] Define `DemoTrace`, `TraceEvent`, `CoverageItem`, `ArtifactSummary`, and `FinalStatus` serde models matching `specs/019-agent-demo/contracts/trace-schema.md` in `examples/agent-demo/trace.rs`
- [X] T008 [P] Implement secret masking and safe metadata helpers for API keys, tool arguments, workspace paths, and verbose diagnostics in `examples/agent-demo/trace.rs`
- [X] T009 [P] Define stable scenario step IDs, capability IDs, and the `complete-agent-walkthrough` scenario model in `examples/agent-demo/scenario.rs`
- [X] T010 [P] Define the canonical capability checklist with statuses, evidence fields, and non-goal items from `specs/019-agent-demo/contracts/coverage-checklist.md` in `examples/agent-demo/scenario.rs`
- [X] T011 Implement terminal timeline rendering, coverage table rendering, JSON trace writing, and artifact summaries in `examples/agent-demo/trace.rs`
- [X] T012 Implement preflight validation for deterministic mode, live mode credential checks, workspace directory reporting, and trace output path reporting in `examples/agent-demo/main.rs`
- [X] T013 Implement mode dispatch from `main.rs` to deterministic and live runners with structured error handling and final trace flushing in `examples/agent-demo/main.rs`

**Checkpoint**: Foundation ready - user story implementation can now begin in priority order or in parallel where marked.

---

## Phase 3: User Story 1 - Run a complete single-agent walkthrough (Priority: P1) 🎯 MVP

**Goal**: Provide a runnable deterministic primary scenario from `examples/agent-demo` that shows a coherent Agent experience end to end without network or credentials.

**Independent Test**: Run `rtk cargo run --example agent_demo -- --mode deterministic --show-coverage` and confirm the walkthrough prints model/agent progress, tool use, memory/context behavior, session continuity, and final summary.

### Validation for User Story 1

- [X] T014 [P] [US1] Build the registered example with `rtk cargo build --example agent_demo` and record any required fixes in `examples/agent-demo/README.md`
- [X] T015 [P] [US1] Run deterministic primary scenario with `rtk cargo run --example agent_demo -- --mode deterministic --show-coverage` and verify the expected sections from `specs/019-agent-demo/contracts/cli-contract.md` appear in terminal output

### Implementation for User Story 1

- [X] T016 [P] [US1] Implement deterministic scripted user turns and agent response plan for `complete-agent-walkthrough` in `examples/agent-demo/deterministic.rs`
- [X] T017 [P] [US1] Implement safe demo tools `calculator`, `knowledge_lookup`, and `workspace_writer` with deterministic results and safe argument summaries in `examples/agent-demo/tools.rs`
- [X] T018 [P] [US1] Implement observation/enrichment/policy middleware simulation or public API integration with `middleware_entered` and `middleware_completed` trace events in `examples/agent-demo/middleware.rs`
- [X] T019 [US1] Wire deterministic scenario execution to emit `preflight_completed`, `agent_started`, `message_sent`, `message_received`, and `scenario_completed` events in `examples/agent-demo/deterministic.rs`
- [X] T020 [US1] Wire deterministic tool invocation lifecycle to emit `tool_called` and `tool_completed` events and include the tool result in the agent summary in `examples/agent-demo/deterministic.rs`
- [X] T021 [US1] Wire deterministic streaming-like incremental output to emit ordered `stream_delta` events without relying on live provider text in `examples/agent-demo/deterministic.rs`
- [X] T022 [US1] Implement two-turn session continuity evidence with `session_saved` and `session_loaded` events in `examples/agent-demo/deterministic.rs`
- [X] T023 [US1] Implement memory/context write and recall evidence with `memory_written` and `memory_recalled` events in `examples/agent-demo/deterministic.rs`
- [X] T024 [US1] Implement workspace artifact creation under the configured demo workspace with `workspace_artifact_written` event and safe artifact summary in `examples/agent-demo/deterministic.rs`
- [X] T025 [US1] Integrate RAG/context enrichment as deterministic lookup or documented mock enrichment with `rag_context_added` event in `examples/agent-demo/deterministic.rs`
- [X] T026 [US1] Integrate sandbox or permission/policy handling as an explicit safe check with `sandbox_checked` event and skipped/optional reason in `examples/agent-demo/deterministic.rs`
- [X] T027 [US1] Print final answer and action summary that references tool output, recalled context, session continuity, workspace artifact, and trace location in `examples/agent-demo/deterministic.rs`
- [X] T028 [US1] Complete deterministic setup, run, expected output, offline behavior, and primary scenario explanation in `examples/agent-demo/README.md`

**Checkpoint**: User Story 1 is independently functional as the MVP and does not require live credentials.

---

## Phase 4: User Story 2 - Observe every major framework capability in context (Priority: P2)

**Goal**: Make all major demo-relevant capabilities observable through runtime output, trace coverage, README coverage, or clearly labeled optional/skipped/unsupported status.

**Independent Test**: Run the deterministic demo with `--show-coverage --trace-json target/agent-demo/trace.json` and confirm the runtime coverage table, README checklist, and trace coverage all use the same capability IDs and evidence mapping.

### Validation for User Story 2

- [X] T029 [P] [US2] Run deterministic trace output with `rtk cargo run --example agent_demo -- --mode deterministic --show-coverage --trace-json target/agent-demo/trace.json` and verify `target/agent-demo/trace.json` follows `specs/019-agent-demo/contracts/trace-schema.md`
- [X] T030 [P] [US2] Compare capability IDs in `examples/agent-demo/README.md`, runtime `--show-coverage` output, and `target/agent-demo/trace.json` against `specs/019-agent-demo/contracts/coverage-checklist.md`

### Implementation for User Story 2

- [X] T031 [P] [US2] Expand `CoverageItem` definitions for `agent-interaction`, `structured-messages`, `event-progress`, `streaming-incremental-output`, `tool-invocation`, `session-continuity`, `memory-context-recall`, `middleware-observation`, `trace-observability`, `configuration-handling`, `safe-secret-handling`, `typed-error-handling`, `cancellation-handling`, `rag-context-enrichment`, `workspace-artifact`, `sandbox-policy`, and `live-provider` in `examples/agent-demo/scenario.rs`
- [X] T032 [US2] Ensure every demonstrated capability in `examples/agent-demo/scenario.rs` has at least one scenario step and one trace event evidence emitted by `examples/agent-demo/deterministic.rs`
- [X] T033 [US2] Implement `--show-coverage` table totals for demonstrated, optional, skipped, and unsupported capabilities in `examples/agent-demo/trace.rs`
- [X] T034 [US2] Implement `--trace-json` serialization with ordered `events[]`, `coverage[]`, `artifacts[]`, and `final_status` in `examples/agent-demo/trace.rs`
- [X] T035 [US2] Add capability coverage table to `examples/agent-demo/README.md` with columns for Capability, Status, Where to observe, and Notes / requirements
- [X] T036 [US2] Document optional, skipped, and unsupported capability boundaries for sandbox policy, live provider, multi-agent collaboration, distributed runtime, and production hardening in `examples/agent-demo/README.md`
- [X] T037 [US2] Implement live-mode preflight and optional DashScope runner skeleton that masks credentials, reports provider/model errors by category, and never runs without explicit credentials in `examples/agent-demo/live.rs`
- [X] T038 [US2] Wire `--mode live` from `examples/agent-demo/main.rs` to `examples/agent-demo/live.rs` while keeping deterministic validation independent of API key, network, and provider output
- [X] T039 [US2] Add live run instructions, `.env`/`API_KEY` guidance, credential masking expectations, and live validation boundaries in `examples/agent-demo/README.md`

**Checkpoint**: User Stories 1 and 2 together provide a complete capability showcase with explicit coverage evidence.

---

## Phase 5: User Story 3 - Use the demo as a reliable learning and regression artifact (Priority: P3)

**Goal**: Make the demo repeatable, maintainable, clear on failures, and safe by default for future regression validation.

**Independent Test**: Run the quickstart validation commands and confirm deterministic success, actionable missing-config failure, controlled tool failure, controlled cancellation, ordered trace preservation, and no raw secret exposure.

### Validation for User Story 3

- [X] T040 [P] [US3] Run live missing-configuration validation with `rtk cargo run --example agent_demo -- --mode live` and verify exit code 2 plus actionable `API_KEY` or `--api-key` guidance without provider calls
- [X] T041 [P] [US3] Run controlled tool failure validation with `rtk cargo run --example agent_demo -- --mode deterministic --fail-tool --show-coverage` and verify stable tool error category/code plus preserved prior trace events
- [X] T042 [P] [US3] Run controlled cancellation validation with `rtk cargo run --example agent_demo -- --mode deterministic --cancel-after-step tool-use --trace-json target/agent-demo/cancelled-trace.json` and verify cancellation event plus preserved completed events
- [X] T043 [P] [US3] Inspect terminal output, `target/agent-demo/trace.json`, `target/agent-demo/cancelled-trace.json`, and workspace artifacts to verify no raw API key, access token, or sensitive credential is present

### Implementation for User Story 3

- [X] T044 [US3] Implement missing live configuration failure path before any provider call with category `config_error`, stable code, recovery hint, and exit code 2 in `examples/agent-demo/live.rs`
- [X] T045 [US3] Implement `--fail-tool` failure injection with `tool_called`, `tool_failed`, `error_reported`, stable `tool_error` category/code, and documented exit behavior in `examples/agent-demo/tools.rs`
- [X] T046 [US3] Implement `--cancel-after-step` handling with `cancellation_requested`, final status `cancelled`, trace preservation, and recommended exit code 130 in `examples/agent-demo/deterministic.rs`
- [X] T047 [US3] Harden JSON trace and terminal rendering so `--verbose` still applies secret masking and never prints raw `--api-key` or `API_KEY` values in `examples/agent-demo/trace.rs`
- [X] T048 [US3] Complete troubleshooting, failure injection, cancellation, secret-safety, maintainer validation, and non-production-template sections in `examples/agent-demo/README.md`
- [X] T049 [US3] Update `specs/019-agent-demo/quickstart.md` if implementation commands, exit behavior, or expected output details differ from the final demo behavior

**Checkpoint**: All user stories are independently functional and suitable for maintainer regression validation.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final quality gates, documentation consistency, formatting, linting, and handoff evidence.

- [X] T050 [P] Run `rtk cargo fmt --check` and fix formatting issues in `examples/agent-demo/*.rs` and `Cargo.toml`
- [X] T051 [P] Run `rtk cargo clippy --workspace --all-targets -- -D warnings` and fix warnings in `examples/agent-demo/*.rs` or related example dependencies
- [X] T052 [P] Run `rtk cargo build --examples` and fix example registration or compilation issues in `Cargo.toml` and `examples/agent-demo/*.rs`
- [X] T053 Run full deterministic quickstart validation `rtk cargo run --example agent_demo -- --mode deterministic --show-coverage --trace-json target/agent-demo/trace.json` and verify final status completed in `target/agent-demo/trace.json`
- [X] T054 Validate task completion evidence against `specs/019-agent-demo/spec.md` FR-001 through FR-018 and update `examples/agent-demo/README.md` for any missing documented behavior
- [X] T055 Update `specs/019-agent-demo/tasks.md` checkboxes as tasks complete and record final validation evidence in `examples/agent-demo/README.md` or implementation notes

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational; delivers MVP deterministic walkthrough
- **User Story 2 (Phase 4)**: Depends on Foundational and benefits from US1 events; can refine coverage in parallel once US1 event names stabilize
- **User Story 3 (Phase 5)**: Depends on Foundational and core US1 runner; failure/cancellation validation can proceed once relevant steps exist
- **Polish (Phase 6)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2); no dependency on US2 or US3
- **User Story 2 (P2)**: Can start after Foundational (Phase 2), but final evidence mapping depends on US1 trace events
- **User Story 3 (P3)**: Can start after Foundational (Phase 2), but tool failure and cancellation paths depend on US1 tool-use and scenario step execution

### Within Each User Story

- Validation commands should be run once the relevant implementation task is complete
- Scenario and trace models before deterministic/live runners
- Tool and middleware helpers before runner integration
- Runtime output before README expected-output finalization
- Error/cancellation behavior before final quickstart validation

### Parallel Opportunities

- T003 and T004 can run in parallel after T001
- T007, T008, T009, and T010 can run in parallel after module files exist
- T016, T017, and T018 can run in parallel after foundational models are defined
- T029 and T030 can run in parallel after trace JSON and coverage output exist
- T040, T041, T042, and T043 can run in parallel after the corresponding error paths are implemented
- T050, T051, and T052 can run in parallel during final quality gates if separate workers coordinate fixes

---

## Parallel Example: User Story 1

```bash
# Independent implementation lanes after Phase 2:
Task: "Implement deterministic scripted user turns and agent response plan in examples/agent-demo/deterministic.rs"
Task: "Implement safe demo tools in examples/agent-demo/tools.rs"
Task: "Implement middleware trace simulation in examples/agent-demo/middleware.rs"

# Independent validation/build lane once files compile:
Task: "Build the registered example with rtk cargo build --example agent_demo"
```

---

## Parallel Example: User Story 2

```bash
# Coverage and trace can be checked by separate maintainers once runtime output exists:
Task: "Verify trace JSON follows specs/019-agent-demo/contracts/trace-schema.md"
Task: "Compare capability IDs across README, runtime coverage, and trace JSON"

# Documentation and live mode boundaries can be authored in parallel:
Task: "Add capability coverage table to examples/agent-demo/README.md"
Task: "Implement live-mode preflight in examples/agent-demo/live.rs"
```

---

## Parallel Example: User Story 3

```bash
# Failure-path validations are independent after implementation:
Task: "Validate live missing configuration path"
Task: "Validate controlled tool failure path"
Task: "Validate controlled cancellation path"
Task: "Validate output and artifacts contain no raw secrets"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Run `rtk cargo run --example agent_demo -- --mode deterministic --show-coverage`
5. Demo is usable as an offline complete single-agent walkthrough

### Incremental Delivery

1. Complete Setup + Foundational → CLI, trace, coverage, and scenario foundation ready
2. Add User Story 1 → deterministic MVP walkthrough → validate offline
3. Add User Story 2 → full capability coverage and optional live path → validate coverage/trace mapping
4. Add User Story 3 → regression/error/cancellation/secret-safety → validate quickstart
5. Run final polish gates before marking Feature 019 complete

### Parallel Team Strategy

With multiple implementers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: deterministic runner and primary walkthrough (US1)
   - Developer B: coverage checklist, trace JSON, live preflight docs (US2)
   - Developer C: failure injection, cancellation, secret validation (US3)
3. Integrate through shared `scenario.rs` and `trace.rs`, then run Phase 6 quality gates

---

## Notes

- [P] tasks = different files, no dependency on incomplete same-file edits
- [Story] label maps task to the user story from `specs/019-agent-demo/spec.md`
- Each user story has an independent command-level validation path
- Deterministic mode must not require network, provider credentials, or live LLM output
- Live mode is optional and must fail before provider calls when credentials are missing
- Optional/skipped/unsupported capabilities must be explicit and must not be counted as demonstrated
- Secret masking applies to normal output, verbose output, trace JSON, and workspace artifacts
