# Tasks: Pi Coding Agent (Rust)

**Input**: Design documents from `/specs/023-pi-coding-agent/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md, constitution.md

**Tests**: Tests are included because the feature specification and implementation plan explicitly require deterministic unit/integration validation for tools, sessions, CLI behavior, and ReAct tool flow.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- CLI example source: `examples/pi-rust/src/`
- CLI example tests: `examples/pi-rust/tests/`
- Feature documents: `specs/023-pi-coding-agent/`
- Repository-level validation: run from repository root with `cargo`/`rtk cargo`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Initialize the `pi-rust` example crate and module skeleton without depending on `pi-ts` runtime code.

- [X] T001 Update `examples/pi-rust/Cargo.toml` with package metadata, binary target, and dependencies on `agent_scope_agent`, `agent_scope_dashscope`, `agent_scope_tool`, `agent_scope_message`, `agent_scope_event`, `agent_scope_memory`, `agent_scope_workspace`, `agent_scope_rag`, `agent_scope_embedding`, `tokio`, `clap`, `serde`, `serde_json`, `uuid`, `chrono`, `anyhow`, `thiserror`, `tracing`, and `tracing-subscriber`
- [X] T002 Replace placeholder `examples/pi-rust/src/main.rs` with module declarations and a Tokio async entry point that delegates to `config`, `agent`, `repl`, `render`, `session`, and `tools`
- [X] T003 [P] Create empty Rust module files `examples/pi-rust/src/config.rs`, `examples/pi-rust/src/error.rs`, `examples/pi-rust/src/agent.rs`, `examples/pi-rust/src/tools.rs`, `examples/pi-rust/src/session.rs`, `examples/pi-rust/src/render.rs`, and `examples/pi-rust/src/repl.rs`
- [X] T004 [P] Create integration test scaffolds `examples/pi-rust/tests/cli_contract.rs`, `examples/pi-rust/tests/tool_contracts.rs`, `examples/pi-rust/tests/session_persistence.rs`, and `examples/pi-rust/tests/react_flow.rs`
- [X] T005 [P] Add `#![deny(unsafe_code)]` and crate-level documentation to `examples/pi-rust/src/main.rs` explaining that `pi-ts` is reference-only and not a Rust dependency

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core configuration, errors, redaction, and runtime data types that MUST be complete before ANY user story can be implemented.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T006 Implement `PiError`, `PiResult`, stable error categories, exit-code mapping, and non-secret display formatting in `examples/pi-rust/src/error.rs`
- [X] T007 Implement `RuntimeConfig` and clap argument parsing for `--api-key`, `--model`, `--workdir`, `--cwd`, `--prompt`, `--resume`, `--list-sessions`, `--no-tools`, `--no-memory`, `--no-rag`, `--max-iters`, `--command-timeout-secs`, `--show-events`, and `--show-json-events` in `examples/pi-rust/src/config.rs`
- [X] T008 [P] Implement API key resolution from `--api-key`, `API_KEY`, and `DASHSCOPE_API_KEY` with masking helpers in `examples/pi-rust/src/config.rs`
- [X] T009 [P] Implement path validation helpers that reject paths outside `--cwd` and normalize workspace-relative paths in `examples/pi-rust/src/tools.rs`
- [X] T010 [P] Implement truncation helpers for large file, command, and event output with visible truncation notices in `examples/pi-rust/src/render.rs`
- [X] T011 [P] Add CLI configuration validation tests for missing credentials, invalid numeric options, default model, default workdir, and masked API key output in `examples/pi-rust/tests/cli_contract.rs`
- [X] T012 [P] Add path validation and output truncation unit tests in `examples/pi-rust/tests/tool_contracts.rs`
- [X] T013 Initialize tracing subscriber from `RUST_LOG` without printing secrets in `examples/pi-rust/src/main.rs`
- [X] T014 Wire top-level error handling so configuration failures exit with code `2`, runtime failures exit with code `1`, and success exits with code `0` in `examples/pi-rust/src/main.rs`

**Checkpoint**: Foundation ready — user story implementation can now begin in priority order or in parallel where task ownership allows.

---

## Phase 3: User Story 1 - Interactive Coding Assistant (Priority: P1) 🎯 MVP

**Goal**: Users can start `pi-rust`, enter a REPL or one-shot prompt, have ReActAgent read/analyze files through tools, see streamed responses and tool status, and continue multi-turn conversations with recent context.

**Independent Test**: Start the CLI in a disposable project containing `src/main.rs`, ask it to read and explain the file, then ask a follow-up referring to "this file" and verify the tool call and contextual answer.

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation.**

- [X] T015 [P] [US1] Add CLI help and one-shot prompt contract tests for `--help` and `--prompt` in `examples/pi-rust/tests/cli_contract.rs`
- [X] T016 [P] [US1] Add Read tool contract tests for successful UTF-8 reads, missing files, directories, binary files, offsets, limits, and line-numbered output in `examples/pi-rust/tests/tool_contracts.rs`
- [X] T017 [P] [US1] Add mock ReAct integration test that verifies a prompt can trigger Read tool execution and produce a final answer in `examples/pi-rust/tests/react_flow.rs`
- [X] T018 [P] [US1] Add REPL command tests for empty input, unknown slash command, `/help`, `/model`, `/tools`, `/events on`, `/events off`, `/json on`, and `/json off` in `examples/pi-rust/tests/cli_contract.rs`

### Implementation for User Story 1

- [X] T019 [US1] Implement `Read` tool input schema, execution logic, UTF-8 validation, line numbering, and structured result conversion in `examples/pi-rust/src/tools.rs`
- [X] T020 [US1] Implement coding-agent system prompt and tool registration for `Read` in `examples/pi-rust/src/agent.rs`
- [X] T021 [US1] Build DashScope-backed ReActAgent runtime with configurable model, max iterations, optional tools, optional memory, and optional RAG flags in `examples/pi-rust/src/agent.rs`
- [X] T022 [US1] Implement human-readable streaming renderer for assistant text, lifecycle events, tool-call start/end, errors, and optional JSON event lines in `examples/pi-rust/src/render.rs`
- [X] T023 [US1] Implement interactive REPL loop with banner, line reading, empty-input ignore, local slash command dispatch, and per-turn streaming in `examples/pi-rust/src/repl.rs`
- [X] T024 [US1] Implement one-shot prompt mode that sends exactly one user turn, streams the answer, saves runtime state placeholder data, and exits in `examples/pi-rust/src/main.rs`
- [X] T025 [US1] Ensure multi-turn in-process context is preserved by appending user and assistant messages between turns in `examples/pi-rust/src/agent.rs`
- [X] T026 [US1] Update `/help`, `/model`, and `/tools` output to match `specs/023-pi-coding-agent/contracts/cli-contract.md` without printing secrets in `examples/pi-rust/src/repl.rs`

**Checkpoint**: User Story 1 is fully functional and testable independently as the MVP.

---

## Phase 4: User Story 2 - Code Editing and File Operations (Priority: P1)

**Goal**: Users can ask the Agent to create, overwrite with confirmation semantics, and precisely edit files inside the configured project working directory.

**Independent Test**: In a disposable directory, ask the Agent to create `hello.txt`, then ask it to replace `World` with `Rust`, and verify the final file content is `Hello, Rust!`.

### Tests for User Story 2

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation.**

- [X] T027 [P] [US2] Add Write tool contract tests for create, existing-file rejection, overwrite behavior, parent directory handling, UTF-8 content, and outside-workspace denial in `examples/pi-rust/tests/tool_contracts.rs`
- [X] T028 [P] [US2] Add Edit tool contract tests for exact replacement, `replace_all`, `pattern_not_found`, `ambiguous_edit`, missing file, outside-workspace denial, and atomic write outcome in `examples/pi-rust/tests/tool_contracts.rs`
- [X] T029 [P] [US2] Add mock ReAct integration test that creates and edits `hello.txt` through Write/Edit tool calls in `examples/pi-rust/tests/react_flow.rs`

### Implementation for User Story 2

- [X] T030 [US2] Implement `Write` tool input schema, file creation, overwrite gating, parent directory creation policy, UTF-8 writes, and structured results in `examples/pi-rust/src/tools.rs`
- [X] T031 [US2] Implement `Edit` tool input schema, exact string replacement, ambiguity detection, pattern-not-found error, `replace_all`, and atomic write behavior in `examples/pi-rust/src/tools.rs`
- [X] T032 [US2] Add permission classification for file overwrite and destructive file mutations with confirm/deny result states in `examples/pi-rust/src/tools.rs`
- [X] T033 [US2] Register `Write` and `Edit` tools with model-facing descriptions matching `specs/023-pi-coding-agent/contracts/tool-contracts.md` in `examples/pi-rust/src/agent.rs`
- [X] T034 [US2] Update renderer to show safe file mutation summaries and redacted/truncated tool results in `examples/pi-rust/src/render.rs`
- [ ] T035 [US2] Update REPL confirmation flow so pending file mutations can be approved or denied before execution in `examples/pi-rust/src/repl.rs`

**Checkpoint**: User Stories 1 and 2 both work independently: read/analyze and file create/edit flows are functional.

---

## Phase 5: User Story 3 - Shell Command Execution (Priority: P2)

**Goal**: Users can ask the Agent to execute shell commands in the configured working directory, with timeout, output capture/truncation, and confirmation for risky commands.

**Independent Test**: Ask the Agent to run `pwd` in a disposable working directory and verify the output path; ask for a destructive command and verify confirmation is required before execution.

### Tests for User Story 3

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation.**

- [X] T036 [P] [US3] Add Bash tool contract tests for `pwd`, stdout/stderr capture, non-zero exit, timeout with partial output, working-directory enforcement, and output truncation in `examples/pi-rust/tests/tool_contracts.rs`
- [X] T037 [P] [US3] Add destructive command classification tests for `rm`, `unlink`, `rmdir`, `git reset`, `git clean`, package install commands, file-writing shell redirection, and network script execution in `examples/pi-rust/tests/tool_contracts.rs`
- [X] T038 [P] [US3] Add mock ReAct integration test that executes a safe Bash command and receives a summarized result in `examples/pi-rust/tests/react_flow.rs`

### Implementation for User Story 3

- [X] T039 [US3] Implement `Bash` tool input schema, command execution from `--cwd`, stdout/stderr/exit-code capture, and structured result conversion in `examples/pi-rust/src/tools.rs`
- [X] T040 [US3] Implement per-command timeout using `--command-timeout-secs` and return `command_timeout` with partial output in `examples/pi-rust/src/tools.rs`
- [X] T041 [US3] Implement destructive command classifier and confirmation requirement for file deletion, git mutation, install/lockfile mutation, file-writing redirection, and network-script commands in `examples/pi-rust/src/tools.rs`
- [X] T042 [US3] Register `Bash` tool with model-facing description and timeout defaults in `examples/pi-rust/src/agent.rs`
- [ ] T043 [US3] Extend REPL confirmation flow for risky Bash commands and ensure denied commands are not executed in `examples/pi-rust/src/repl.rs`
- [X] T044 [US3] Update renderer to show command, exit code, timeout status, truncated stdout/stderr summaries, and non-zero failures in `examples/pi-rust/src/render.rs`

**Checkpoint**: User Story 3 is independently functional and safe-command/risky-command behavior is covered by tests.

---

## Phase 6: User Story 4 - Session Persistence and Recovery (Priority: P2)

**Goal**: Users can save sessions on each turn or exit, list saved sessions, and resume the latest or selected session with prior conversation context restored.

**Independent Test**: Run a one-shot or REPL session that records a fact, exit, restart with `--resume`, ask about the prior fact, and verify the session history was loaded.

### Tests for User Story 4

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation.**

- [X] T045 [P] [US4] Add `SessionRecord` JSON round-trip tests with stable IDs, timestamps, turns, errors, summaries, and no API key leakage in `examples/pi-rust/tests/session_persistence.rs`
- [X] T046 [P] [US4] Add session loading tests for latest session, selected session ID, missing session, corrupt JSON, empty session directory, and `/sessions` listing in `examples/pi-rust/tests/session_persistence.rs`
- [X] T047 [P] [US4] Add REPL persistence integration test for `/save`, `/exit`, and resume context reconstruction in `examples/pi-rust/tests/react_flow.rs`

### Implementation for User Story 4

- [X] T048 [US4] Implement `SessionRecord`, `ConversationTurn`, `ToolInvocation`, and serializable `ErrorRecord` data structures in `examples/pi-rust/src/session.rs`
- [X] T049 [US4] Implement session directory creation and JSON save/load/list operations under `<workdir>/sessions/` in `examples/pi-rust/src/session.rs`
- [X] T050 [US4] Implement latest-session selection and explicit `--resume <SESSION_ID>` validation in `examples/pi-rust/src/session.rs`
- [X] T051 [US4] Persist each completed user turn with ordered events, assistant text, timestamps, errors, and redacted tool summaries in `examples/pi-rust/src/session.rs`
- [X] T052 [US4] Reconstruct ReActAgent conversation context from resumed session turns before accepting new input in `examples/pi-rust/src/agent.rs`
- [X] T053 [US4] Implement `/sessions`, `/save`, `/exit`, and `/quit` session behavior in `examples/pi-rust/src/repl.rs`
- [X] T054 [US4] Ensure one-shot mode saves a session after success or runtime error without serializing API keys in `examples/pi-rust/src/main.rs`

**Checkpoint**: User Story 4 is independently functional with durable local JSON session recovery.

---

## Phase 7: User Story 5 - Multi-Provider LLM Support (Priority: P3)

**Goal**: Users can run the CLI with DashScope by default and choose a model name at startup; the implementation leaves provider extension points explicit without adding fake unsupported providers.

**Independent Test**: Start without provider-specific flags using `API_KEY` and confirm DashScope is selected; start with `--model <MODEL>` and verify that model name is used and displayed by `/model` without secrets.

### Tests for User Story 5

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation.**

- [X] T055 [P] [US5] Add provider configuration tests for default DashScope selection, `API_KEY`, `DASHSCOPE_API_KEY`, `--api-key`, `--model`, and secret masking in `examples/pi-rust/tests/cli_contract.rs`
- [X] T056 [P] [US5] Add agent builder tests verifying unsupported provider paths return explicit unsupported-feature errors instead of no-op success in `examples/pi-rust/tests/react_flow.rs`

### Implementation for User Story 5

- [X] T057 [US5] Implement provider selection data model with DashScope default and explicit unsupported-provider error handling in `examples/pi-rust/src/config.rs`
- [X] T058 [US5] Create DashScope chat model from resolved credentials and configured model name in `examples/pi-rust/src/agent.rs`
- [X] T059 [US5] Update `/model`, startup banner, and JSON event metadata to show provider/model names without exposing credentials in `examples/pi-rust/src/repl.rs` and `examples/pi-rust/src/render.rs`

**Checkpoint**: User Story 5 is independently functional for default DashScope and model-name configuration, with honest unsupported-provider behavior.

---

## Final Phase: Polish & Cross-Cutting Concerns

**Purpose**: Validate the complete CLI, harden edge cases, update docs, and ensure constitution compliance.

- [X] T060 [P] Add edge-case tests for extremely long user input, API timeout/error mapping, read-only working directory, concurrent external file modification before Edit, rapid sequential messages, and context growth handling in `examples/pi-rust/tests/react_flow.rs`
- [X] T061 [P] Add long-term memory enable/disable smoke tests for `MemoryMiddleware` integration in `examples/pi-rust/tests/react_flow.rs`
- [X] T062 [P] Add RAG enable/disable smoke tests for `RAGMiddleware` integration in `examples/pi-rust/tests/react_flow.rs`
- [X] T063 Implement optional MemoryMiddleware wiring with `<workdir>/Memory/` storage and `--no-memory` disable behavior in `examples/pi-rust/src/agent.rs`
- [ ] T064 Implement optional RAGMiddleware wiring with project-document retrieval and `--no-rag` disable behavior in `examples/pi-rust/src/agent.rs`
- [X] T065 Update `examples/pi-rust/README.md` with CLI usage, safety model, session storage layout, examples, and quickstart commands
- [X] T066 Update `specs/023-pi-coding-agent/quickstart.md` if implementation command names, paths, or validation steps changed during development
- [X] T067 Run `cargo fmt --check` from repository root and fix formatting issues in `examples/pi-rust/`
- [X] T068 Run `cargo clippy -p pi-rust --all-targets -- -D warnings` from repository root and fix all lints in `examples/pi-rust/`
- [X] T069 Run `cargo test -p pi-rust` from repository root and fix all test failures
- [X] T070 Run `cargo check -p pi-rust` and `cargo run -p pi-rust -- --help` from repository root to verify build and CLI contract
- [ ] T071 Perform manual quickstart validation from `specs/023-pi-coding-agent/quickstart.md` with a real DashScope API key and record any provider-specific caveats in `examples/pi-rust/README.md`
- [X] T072 Verify `examples/pi-rust` does not import, include, shell out to, or otherwise depend on `examples/pi-rust/pi-ts` runtime code in `examples/pi-rust/Cargo.toml` and `examples/pi-rust/src/`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories.
- **User Story 1 (Phase 3, P1 MVP)**: Depends on Foundational completion.
- **User Story 2 (Phase 4, P1)**: Depends on Foundational completion and integrates with REPL/agent wiring from US1; can develop tool logic/tests in parallel with US1 but final CLI integration depends on US1 runtime wiring.
- **User Story 3 (Phase 5, P2)**: Depends on Foundational completion and uses the same permission/rendering patterns as US2.
- **User Story 4 (Phase 6, P2)**: Depends on Foundational completion and integrates with turn/event output from US1-US3.
- **User Story 5 (Phase 7, P3)**: Depends on Foundational completion and agent builder wiring from US1.
- **Polish (Final Phase)**: Depends on all desired user stories being complete.

### User Story Dependencies

- **User Story 1 (P1)**: MVP baseline; no dependencies on other stories after Foundation.
- **User Story 2 (P1)**: Core file mutation; tool implementation can be independent, CLI confirmation integration benefits from US1 REPL flow.
- **User Story 3 (P2)**: Shell execution; depends on shared permission and rendering conventions established by US2.
- **User Story 4 (P2)**: Session persistence; can start after Foundation, but full event/turn persistence should integrate with completed US1-US3 flows.
- **User Story 5 (P3)**: Provider configuration; default DashScope support is needed by US1 runtime, while additional unsupported-provider handling can finish later.

### Within Each User Story

- Tests MUST be written and observed failing before implementation.
- Data structures and schemas before execution logic.
- Tool/service implementation before agent registration.
- Agent registration before REPL/renderer integration.
- Story complete before moving to next priority checkpoint unless parallel ownership is explicit.

### Parallel Opportunities

- T003, T004, and T005 can run in parallel after T001-T002 ownership is assigned.
- T008, T009, T010, T011, and T012 can run in parallel after T006-T007 interfaces are drafted.
- Test tasks inside each user story are parallelizable because they touch separate test concerns or can be coordinated within the same test file before implementation.
- Tool implementations for Read, Write/Edit, and Bash can be developed in parallel after foundational path/error helpers exist.
- Session persistence data model can be developed in parallel with US2/US3 tool execution once the event/result shapes are agreed.
- Memory and RAG polish smoke tests can be written in parallel with README updates.

---

## Parallel Example: User Story 1

```bash
# Independent tests before implementation:
Task: "T015 [US1] Add CLI help and one-shot prompt contract tests in examples/pi-rust/tests/cli_contract.rs"
Task: "T016 [US1] Add Read tool contract tests in examples/pi-rust/tests/tool_contracts.rs"
Task: "T017 [US1] Add mock ReAct integration test in examples/pi-rust/tests/react_flow.rs"
Task: "T018 [US1] Add REPL command tests in examples/pi-rust/tests/cli_contract.rs"

# Implementation sequence after tests fail:
Task: "T019 [US1] Implement Read tool in examples/pi-rust/src/tools.rs"
Task: "T020-T021 [US1] Build agent runtime in examples/pi-rust/src/agent.rs"
Task: "T022-T026 [US1] Wire renderer, REPL, one-shot mode, and help output"
```

## Parallel Example: User Story 2

```bash
# Contract and flow tests can be prepared together:
Task: "T027 [US2] Add Write tool contract tests in examples/pi-rust/tests/tool_contracts.rs"
Task: "T028 [US2] Add Edit tool contract tests in examples/pi-rust/tests/tool_contracts.rs"
Task: "T029 [US2] Add mock ReAct Write/Edit integration test in examples/pi-rust/tests/react_flow.rs"

# Then implementation follows shared tool file ordering:
Task: "T030-T032 [US2] Implement Write/Edit and permission classification in examples/pi-rust/src/tools.rs"
Task: "T033-T035 [US2] Register tools and wire renderer/confirmation flow"
```

## Parallel Example: User Story 3

```bash
Task: "T036 [US3] Add Bash execution contract tests in examples/pi-rust/tests/tool_contracts.rs"
Task: "T037 [US3] Add destructive command classification tests in examples/pi-rust/tests/tool_contracts.rs"
Task: "T038 [US3] Add mock ReAct Bash integration test in examples/pi-rust/tests/react_flow.rs"
Task: "T039-T044 [US3] Implement Bash tool, timeout, risk confirmation, registration, and rendering"
```

## Parallel Example: User Story 4

```bash
Task: "T045 [US4] Add SessionRecord JSON round-trip tests in examples/pi-rust/tests/session_persistence.rs"
Task: "T046 [US4] Add session loading/listing tests in examples/pi-rust/tests/session_persistence.rs"
Task: "T047 [US4] Add REPL persistence integration test in examples/pi-rust/tests/react_flow.rs"
Task: "T048-T054 [US4] Implement session data structures, storage, resume, and command wiring"
```

## Parallel Example: User Story 5

```bash
Task: "T055 [US5] Add provider configuration tests in examples/pi-rust/tests/cli_contract.rs"
Task: "T056 [US5] Add unsupported provider tests in examples/pi-rust/tests/react_flow.rs"
Task: "T057-T059 [US5] Implement provider selection, DashScope model construction, and redacted display"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational.
3. Complete Phase 3: User Story 1.
4. **STOP and VALIDATE**: Run `cargo test -p pi-rust --test cli_contract`, `cargo test -p pi-rust --test tool_contracts`, `cargo test -p pi-rust --test react_flow`, `cargo check -p pi-rust`, and `cargo run -p pi-rust -- --help`.
5. Demo MVP with a disposable project and a prompt that reads `src/main.rs`.

### Incremental Delivery

1. Complete Setup + Foundational → configuration, errors, redaction, path safety, and scaffolding ready.
2. Add User Story 1 → interactive read/analyze assistant MVP → validate independently.
3. Add User Story 2 → file create/edit capability → validate independently.
4. Add User Story 3 → Bash execution with safety → validate independently.
5. Add User Story 4 → durable sessions and resume → validate independently.
6. Add User Story 5 → provider/model configuration hardening → validate independently.
7. Complete Polish → memory/RAG smoke tests, docs, fmt, clippy, tests, quickstart, and pi-ts independence check.

### Parallel Team Strategy

With multiple developers or agents:

1. One owner completes T001-T014 to establish interfaces and scaffolding.
2. After Foundation:
   - Developer A: US1 runtime, REPL, Read flow.
   - Developer B: US2 Write/Edit contracts and tool implementation.
   - Developer C: US3 Bash contracts and tool implementation.
   - Developer D: US4 session data model and storage tests.
   - Developer E: US5 provider configuration tests and builder validation.
3. Integrate through `examples/pi-rust/src/agent.rs`, `examples/pi-rust/src/repl.rs`, and `examples/pi-rust/src/render.rs` in priority order.
4. Final owner runs T067-T072 and resolves cross-cutting issues before marking the feature complete.

---

## Notes

- [P] tasks use different files or isolated concerns and can be done without waiting on unrelated incomplete tasks.
- [Story] labels map directly to user stories in `specs/023-pi-coding-agent/spec.md`.
- Every user story has an independent test criterion and checkpoint.
- Tests are included because this feature explicitly requires deterministic validation and mock-model ReAct flow tests.
- Avoid adding any dependency on `examples/pi-rust/pi-ts`; it is a reference artifact only.
- Avoid fake provider support: unsupported providers must return typed unsupported-feature errors.
- Commit after each task or logical group once tests for that increment pass.
