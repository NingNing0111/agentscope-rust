# Tasks: Microsandbox Sandbox Backend

**Feature**: `035-microsandbox-sandbox-backend`
**Date**: 2026-08-24

## Phase 1 — Spec-kit artifacts

- [x] T001 Create `spec.md`
- [x] T002 Create `plan.md`
- [x] T003 Create `research.md`
- [x] T004 Create `data-model.md`
- [x] T005 Create contracts for session, backend selection, and workspace backend
- [x] T006 Create `quickstart.md`

## Phase 2 — Cargo and public API

- [x] T007 Add workspace dependency `microsandbox = "0.6.10"`
- [x] T008 Add `microsandbox` feature and optional dependency to `agent_scope_sandbox`
- [x] T009 Add feature-gated `microsandbox` module and public re-exports

## Phase 3 — Adapter compatibility

- [x] T010 Generalize `SandboxWorkspaceBackend` from `LocalSandboxSession` to `Box<dyn SandboxSession>`
- [x] T011 Preserve `SandboxWorkspaceBackend::new(LocalSandboxSession)`
- [x] T012 Add `from_session` and `from_boxed_session`
- [x] T013 Add local regression tests for workspace backend constructors

## Phase 4 — Deterministic microsandbox model tests

- [x] T014 Add config validation tests
- [x] T015 Add policy mapping tests
- [x] T016 Add capability report tests
- [x] T017 Add stable error mapping tests
- [x] T018 Add guest path sanitizer tests

## Phase 5 — Microsandbox backend implementation

- [x] T019 Implement `MicrosandboxConfig` and validation
- [x] T020 Implement microsandbox-specific policy helpers
- [x] T021 Implement `CapabilityReport::microsandbox()`
- [x] T022 Implement stable SDK/runtime error mapping helpers
- [x] T023 Implement `MicrosandboxSession` lifecycle
- [x] T024 Implement command execution, timeout validation, output summaries, and history
- [x] T025 Implement file APIs through microsandbox SDK fs API or a documented equivalent
- [x] T026 Implement guest path authorization and mount checks
- [x] T027 Ensure unsupported capabilities fail explicitly and never fallback to local-process

## Phase 6 — Runtime tests, docs, examples

- [x] T028 Add feature-gated ignored real-runtime integration tests
- [x] T029 Update sandbox example with microsandbox feature-gated path
- [x] T030 Update docs to explain local-process vs microsandbox isolation

## Phase 7 — Verification

- [x] T031 Run `rtk cargo fmt --check`
- [x] T032 Run `rtk cargo check --workspace --all-targets`
- [x] T033 Run `rtk cargo test --workspace`
- [x] T034 Run `rtk cargo clippy --workspace --all-targets -- -D warnings`
- [x] T035 Run `rtk cargo check -p agent_scope_sandbox --features microsandbox`
- [x] T036 Run `rtk cargo test -p agent_scope_sandbox --features microsandbox`
- [ ] T037 Optionally run ignored real-runtime tests when runtime is installed
