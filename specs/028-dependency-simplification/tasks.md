# Tasks: Dependency Simplification

**Input**: Design documents from `/specs/028-dependency-simplification/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/dependency-evaluation-contract.md, quickstart.md

**Tests**: Included because FR-007 and SC-003 explicitly require regression evidence for every completed replacement.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Every task includes an exact project-relative file path

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare tracking artifacts and dependency-governance locations before any replacement work starts.

- [X] T001 Create dependency evaluation records file from the contract template in specs/028-dependency-simplification/dependency-evaluations.md
- [X] T002 Create behavior preservation evidence matrix in specs/028-dependency-simplification/behavior-evidence.md
- [X] T003 [P] Create first-batch implementation notes file in specs/028-dependency-simplification/implementation-notes.md
- [X] T004 [P] Record current workspace dependency baseline before additions in specs/028-dependency-simplification/implementation-notes.md

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Complete dependency approvals and establish shared compatibility wrappers that all user-story implementation depends on.

**⚠️ CRITICAL**: No user story implementation may begin until these dependency evaluations and wrapper boundaries are complete.

- [X] T005 Complete YAML/frontmatter parser dependency evaluation for skill-frontmatter-parser and memory-frontmatter-parser in specs/028-dependency-simplification/dependency-evaluations.md
- [X] T006 [P] Complete glob/traversal dependency evaluation for pi-rust-file-discovery in specs/028-dependency-simplification/dependency-evaluations.md
- [X] T007 [P] Complete thiserror dependency evaluation for typed-error-derives in specs/028-dependency-simplification/dependency-evaluations.md
- [X] T008 Add approved first-batch dependencies and feature policies to Cargo.toml
- [X] T009 Create shared frontmatter helper crate manifest in crates/agent_scope_frontmatter/Cargo.toml
- [X] T010 Create shared frontmatter helper crate entry point in crates/agent_scope_frontmatter/src/lib.rs

**Checkpoint**: Dependency approvals and shared helper boundary are ready; user story implementation can now begin.

---

## Phase 3: User Story 1 - Identify Simplification Candidates (Priority: P1) 🎯 MVP

**Goal**: Produce a bounded, reviewable candidate inventory with adopt/defer/reject decisions and evidence requirements.

**Independent Test**: Review the inventory and confirm that at least 10 candidates include current responsibility, affected files, replacement rationale, risk, decision status, and required acceptance evidence.

### Implementation for User Story 1

- [X] T011 [US1] Create candidate inventory file with at least 10 reviewed candidates in specs/028-dependency-simplification/candidate-inventory.md
- [X] T012 [P] [US1] Document adopted first-batch candidates and behavior requirements in specs/028-dependency-simplification/candidate-inventory.md
- [X] T013 [P] [US1] Document adopt-cautiously candidates path-component-sanitization and tool-context-cache in specs/028-dependency-simplification/candidate-inventory.md
- [X] T014 [P] [US1] Document deferred candidates dashscope-sse-framing, model-retry-backoff, json-repair-and-schema-flatten, json-file-session-store, and mcp-internal-model-replacement in specs/028-dependency-simplification/candidate-inventory.md
- [X] T015 [P] [US1] Document rejected protocol and security candidates event-protocol-types, message-content-protocol, and sandbox-path-containment in specs/028-dependency-simplification/candidate-inventory.md
- [X] T016 [US1] Add summary counts and SC-001/SC-002 traceability to specs/028-dependency-simplification/candidate-inventory.md
- [X] T017 [US1] Cross-link candidate inventory decisions to dependency evaluations and behavior evidence in specs/028-dependency-simplification/candidate-inventory.md

**Checkpoint**: User Story 1 is independently complete when candidate-inventory.md satisfies SC-001 and identifies at least 3 approved first-batch simplifications or documents why they are unsuitable.

---

## Phase 4: User Story 2 - Replace Low-Risk Basic Implementations (Priority: P2)

**Goal**: Replace approved low-risk commodity implementations with vetted dependencies or dependency-backed wrappers while preserving public behavior.

**Independent Test**: Select each approved replacement, run its targeted regression checks, and verify that public APIs, serialization, event ordering, error categories, examples, and persisted formats remain compatible.

### Tests for User Story 2

- [X] T018 [P] [US2] Add shared skill frontmatter golden tests for inline, quoted, block scalar, folded scalar, and malformed fallback cases in crates/agent_scope_frontmatter/tests/skill_frontmatter.rs
- [X] T019 [P] [US2] Add memory frontmatter round-trip and legacy-read compatibility tests in crates/agent_scope_memory/tests/frontmatter_compat.rs
- [X] T020 [P] [US2] Add pi-rust Glob/Grep/ListDir behavior tests for relative paths, **/ matching, hidden entries, symlink skipping, caps, and ordering in examples/pi-rust/tests/tools_file_discovery.rs
- [X] T021 [P] [US2] Add representative Display/source compatibility tests for migrated typed errors in crates/agent_scope_agent/tests/error_compat.rs
- [X] T022 [P] [US2] Add representative Display/source compatibility tests for migrated typed errors in crates/agent_scope_model/tests/error_compat.rs
- [X] T023 [P] [US2] Add representative Display/source compatibility tests for migrated typed errors in crates/agent_scope_workspace/tests/error_compat.rs

### Implementation for User Story 2

- [X] T024 [US2] Implement dependency-backed frontmatter parsing API with graceful malformed-frontmatter fallback in crates/agent_scope_frontmatter/src/lib.rs
- [X] T025 [US2] Replace duplicated SKILL.md parser in crates/agent_scope_tool/src/skill_loader.rs with the shared frontmatter helper
- [X] T026 [US2] Replace mirrored SKILL.md parser in crates/agent_scope_workspace/src/skill.rs with the shared frontmatter helper
- [X] T027 [US2] Add agent_scope_frontmatter dependency wiring to crates/agent_scope_tool/Cargo.toml
- [X] T028 [US2] Add agent_scope_frontmatter dependency wiring to crates/agent_scope_workspace/Cargo.toml
- [X] T029 [US2] Preserve memory markdown field names and body layout while reusing approved frontmatter parsing in crates/agent_scope_memory/src/frontmatter.rs
- [X] T030 [US2] Add approved frontmatter dependency wiring to crates/agent_scope_memory/Cargo.toml
- [X] T031 [US2] Refactor pi-rust glob traversal to approved globset/walkdir/ignore wrapper while preserving output shape and caps in examples/pi-rust/src/tools.rs
- [X] T032 [US2] Add approved glob/traversal dependency wiring to examples/pi-rust/Cargo.toml
- [X] T033 [US2] Migrate eligible agent error enum implementations to thiserror without changing public variants or Display text in crates/agent_scope_agent/src/error.rs
- [X] T034 [US2] Migrate eligible model error enum implementations to thiserror without changing public variants or Display text in crates/agent_scope_model/src/error.rs
- [X] T035 [US2] Migrate eligible workspace error enum implementations to thiserror without changing public variants or Display text in crates/agent_scope_workspace/src/error.rs
- [X] T036 [US2] Record per-candidate code-reduction and compatibility notes for completed replacements in specs/028-dependency-simplification/implementation-notes.md

**Checkpoint**: User Story 2 is independently complete when at least 3 approved replacements pass targeted regression checks and behavior-evidence.md records zero undocumented compatibility regressions.

---

## Phase 5: User Story 3 - Preserve Project Governance and Regression Safety (Priority: P3)

**Goal**: Prove dependency-driven simplification preserved AgentScope Rust governance, compatibility, safety, and documentation expectations.

**Independent Test**: Run release-gate evidence for changed areas and confirm that no new dependency violates license, security, layering, duplicate-responsibility, or behavior-compatibility rules.

### Implementation for User Story 3

- [X] T037 [US3] Record results of rtk cargo test -p agent_scope_frontmatter in specs/028-dependency-simplification/behavior-evidence.md
- [X] T038 [US3] Record results of rtk cargo test -p agent_scope_tool skill in specs/028-dependency-simplification/behavior-evidence.md
- [X] T039 [US3] Record results of rtk cargo test -p agent_scope_workspace skill in specs/028-dependency-simplification/behavior-evidence.md
- [X] T040 [US3] Record results of rtk cargo test -p agent_scope_memory frontmatter in specs/028-dependency-simplification/behavior-evidence.md
- [X] T041 [US3] Record results of rtk cargo test -p pi-rust tools in specs/028-dependency-simplification/behavior-evidence.md
- [X] T042 [US3] Record results of rtk cargo test -p agent_scope_agent error in specs/028-dependency-simplification/behavior-evidence.md
- [X] T043 [US3] Record results of rtk cargo test -p agent_scope_model error in specs/028-dependency-simplification/behavior-evidence.md
- [X] T044 [US3] Record results of rtk cargo test -p agent_scope_workspace error in specs/028-dependency-simplification/behavior-evidence.md
- [X] T045 [US3] Record workspace-level rtk cargo fmt --check result in specs/028-dependency-simplification/behavior-evidence.md
- [X] T046 [US3] Record workspace-level rtk cargo clippy --workspace --all-targets --all-features -- -D warnings result in specs/028-dependency-simplification/behavior-evidence.md
- [X] T047 [US3] Record workspace-level rtk cargo test --workspace --all-features result in specs/028-dependency-simplification/behavior-evidence.md
- [X] T048 [US3] Verify no event, message, sandbox containment, provider streaming, or persisted-state semantics were replaced in specs/028-dependency-simplification/implementation-notes.md
- [X] T049 [US3] Update dependency simplification guidance and validation expectations in specs/028-dependency-simplification/quickstart.md

**Checkpoint**: User Story 3 is independently complete when behavior-evidence.md and implementation-notes.md demonstrate all quality gates passed and no undocumented compatibility, security, or layering regression remains.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final documentation, formatting, and traceability cleanup across all stories.

- [X] T050 [P] Update feature task completion notes in specs/028-dependency-simplification/tasks.md
- [X] T051 [P] Update crate-level dependency notes if public maintainer guidance changes in README.md
- [X] T052 [P] Update AgentScope guide notes if dependency adoption changes troubleshooting expectations in docs/agentscope-guide.md
- [X] T053 Confirm all task evidence links and candidate IDs are consistent in specs/028-dependency-simplification/candidate-inventory.md
- [X] T054 Confirm all dependency evaluation records satisfy the contract in specs/028-dependency-simplification/dependency-evaluations.md
- [X] T055 Confirm final acceptance checklist is complete in specs/028-dependency-simplification/behavior-evidence.md

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies; can start immediately.
- **Foundational (Phase 2)**: Depends on Setup; blocks all implementation stories because dependency approvals and wrapper boundaries must be settled first.
- **User Story 1 (Phase 3)**: Depends on Foundational for dependency-evaluation structure, but can be delivered as the MVP before code replacement.
- **User Story 2 (Phase 4)**: Depends on Foundational and uses US1 decisions as its approved first-batch scope.
- **User Story 3 (Phase 5)**: Depends on completed US2 replacements for final evidence, but individual evidence-recording tasks can run as each replacement lands.
- **Polish (Phase 6)**: Depends on the desired stories being complete.

### User Story Dependencies

- **US1 Identify Simplification Candidates (P1)**: MVP; no dependency on US2 or US3 after Phase 2.
- **US2 Replace Low-Risk Basic Implementations (P2)**: Depends on approved candidate decisions from US1 and dependency evaluations from Phase 2.
- **US3 Preserve Project Governance and Regression Safety (P3)**: Depends on completed or attempted replacements from US2; validates and documents final evidence.

### Within Each User Story

- US1 inventory category tasks can run in parallel, then T016 and T017 consolidate counts and links.
- US2 tests T018–T023 should be written before replacement implementation T024–T035.
- US2 frontmatter implementation order is T024 before T025/T026/T029.
- US2 Cargo.toml wiring tasks T027/T028/T030/T032 should be coordinated with the implementation tasks that need them.
- US3 evidence tasks can run as soon as their corresponding US2 implementation and tests are ready.

---

## Parallel Opportunities

- T003 and T004 can run in parallel after T001/T002 are started because they write different sections/files.
- T006 and T007 can run in parallel with T005 because each evaluates a different dependency class in dependency-evaluations.md; merge carefully if multiple authors edit the same file.
- T012, T013, T014, and T015 can run in parallel while building the candidate inventory because they cover different decision groups.
- T018 through T023 can run in parallel because they create independent compatibility tests in different crates/examples.
- T025 and T026 can run in parallel after T024 because they replace skill parsing in different crates.
- T021, T022, and T023 can run in parallel with T018–T020 because they cover independent error modules.
- T037 through T044 can run in parallel after the relevant crate-level changes are implemented.
- T050, T051, and T052 can run in parallel during polish because they update different documentation paths.

---

## Parallel Example: User Story 1

```bash
Task: "Document adopted first-batch candidates and behavior requirements in specs/028-dependency-simplification/candidate-inventory.md"
Task: "Document adopt-cautiously candidates path-component-sanitization and tool-context-cache in specs/028-dependency-simplification/candidate-inventory.md"
Task: "Document deferred candidates in specs/028-dependency-simplification/candidate-inventory.md"
Task: "Document rejected protocol and security candidates in specs/028-dependency-simplification/candidate-inventory.md"
```

## Parallel Example: User Story 2

```bash
Task: "Add shared skill frontmatter golden tests in crates/agent_scope_frontmatter/tests/skill_frontmatter.rs"
Task: "Add memory frontmatter compatibility tests in crates/agent_scope_memory/tests/frontmatter_compat.rs"
Task: "Add pi-rust file discovery behavior tests in examples/pi-rust/tests/tools_file_discovery.rs"
Task: "Add typed error compatibility tests in crates/agent_scope_agent/tests/error_compat.rs"
Task: "Add typed error compatibility tests in crates/agent_scope_model/tests/error_compat.rs"
Task: "Add typed error compatibility tests in crates/agent_scope_workspace/tests/error_compat.rs"
```

## Parallel Example: User Story 3

```bash
Task: "Record rtk cargo test -p agent_scope_tool skill evidence in specs/028-dependency-simplification/behavior-evidence.md"
Task: "Record rtk cargo test -p agent_scope_workspace skill evidence in specs/028-dependency-simplification/behavior-evidence.md"
Task: "Record rtk cargo test -p agent_scope_memory frontmatter evidence in specs/028-dependency-simplification/behavior-evidence.md"
Task: "Record rtk cargo test -p pi-rust tools evidence in specs/028-dependency-simplification/behavior-evidence.md"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational dependency evaluation structure.
3. Complete Phase 3: US1 candidate inventory and decision classification.
4. **STOP and VALIDATE**: Confirm candidate-inventory.md has at least 10 categorized candidates, behavior requirements, decision rationales, and SC-001/SC-002 traceability.

### Incremental Delivery

1. Setup + Foundational → dependency governance ready.
2. US1 → candidate inventory MVP ready for maintainer review.
3. US2 → approved low-risk replacements completed behind compatibility evidence.
4. US3 → release-gate and governance evidence recorded.
5. Polish → documentation and traceability cleanup.

### Parallel Team Strategy

With multiple contributors:

1. One contributor prepares dependency-evaluations.md and behavior-evidence.md.
2. One contributor completes US1 inventory sections by decision group.
3. After dependency evaluations pass, separate contributors implement frontmatter, pi-rust file discovery, and thiserror migrations in parallel.
4. A separate verifier records US3 evidence and checks governance constraints before completion.

---

## Notes

- [P] tasks use different files or independent review sections and can run concurrently with careful merge discipline.
- [US1], [US2], and [US3] labels map directly to the user stories in spec.md.
- Tests are included because the feature requires behavior preservation evidence, not because a separate TDD mode was requested.
- Do not implement deferred or rejected candidates unless a later spec changes their compatibility or security boundary.
- Use RTK-prefixed commands for every shell validation command in this project.
- Commit after each completed story or coherent dependency-replacement batch.

## Feature 028 Completion Notes

- T001–T017 established dependency-governance artifacts, candidate inventory, and approved first-batch dependency evaluations.
- T018–T023 added compatibility tests before replacing low-risk implementations.
- T024–T035 implemented the first batch: shared frontmatter parsing, memory frontmatter reuse, pi-rust file discovery via `globset`/`walkdir`, and typed error boilerplate migration to `thiserror`.
- T036–T044 and T048–T049 recorded targeted behavior evidence and updated validation guidance.
- T045–T047 recorded final workspace gates: `rtk cargo fmt --check`, `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `rtk cargo test --workspace --all-features` all passed.
- T050–T055 completed polish and traceability checks. README.md and docs/agentscope-guide.md required no public maintainer/troubleshooting updates because dependency adoption is internal and wrapper-governed.
- Final verification evidence is recorded in `behavior-evidence.md` and `implementation-notes.md`; all task entries are marked `[X]`.
