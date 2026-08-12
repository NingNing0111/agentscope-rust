# Behavior Preservation Evidence: Dependency Simplification

**Feature**: 028-dependency-simplification  
**Date**: 2026-08-11  
**Status**: Complete

This matrix records the compatibility evidence required for each adopted first-batch simplification. Entries begin as `required` and are updated with exact command results after implementation.

## Evidence Matrix

| ID | Candidate | Evidence Type | Command or Artifact | Behavior Covered | Expected Result | Actual Result | Status |
|----|-----------|---------------|---------------------|------------------|-----------------|---------------|--------|
| BE-SKILL-001 | skill-frontmatter-parser | golden-test | `rtk cargo test -p agent_scope_frontmatter` | Inline, quoted, literal block, folded block, EOF delimiter, malformed fallback | Golden tests pass and preserve legacy parser outputs | `cargo test: 8 passed (3 suites, 0.00s)` | passed |
| BE-SKILL-002 | skill-frontmatter-parser | integration-test | `rtk cargo test -p agent_scope_tool skill` | Existing tool skill discovery, malformed skill skip, scan behavior unchanged | Test command passes | `cargo test: 29 passed, 54 filtered out (5 suites, 0.01s)` | passed |
| BE-SKILL-003 | skill-frontmatter-parser | integration-test | `rtk cargo test -p agent_scope_workspace skill` | Workspace skill validation/listing behavior unchanged | Test command passes | `cargo test: 8 passed, 34 filtered out (6 suites, 0.00s)` | passed |
| BE-MEM-001 | memory-frontmatter-parser | integration-test | `rtk cargo test -p agent_scope_memory frontmatter` | Legacy memory frontmatter read, quote/unescape, CRLF, EOF delimiter, body layout | Compatibility tests pass | `cargo test: 6 passed, 64 filtered out (9 suites, 0.02s)` | passed |
| BE-MEM-002 | memory-frontmatter-parser | integration-test | `rtk cargo test -p agent_scope_memory frontmatter` | Existing unit tests and new compatibility tests pass | Test command passes | `cargo test: 6 passed, 64 filtered out (9 suites, 0.02s)` | passed |
| BE-PI-001 | pi-rust-file-discovery | integration-test | `rtk cargo test -p pi-rust --test tools_file_discovery` | Glob/Grep/ListDir relative paths, `**/`, hidden skip, symlink skip, caps, ordering | Compatibility tests pass | `cargo test: 5 passed (1 suite, 0.03s)` | passed |
| BE-PI-002 | pi-rust-file-discovery | example-run | `rtk cargo test -p pi-rust tools` | Existing pi-rust tool behavior and output shape unchanged | Test command passes | `cargo test: 3 passed, 112 filtered out (11 suites, 0.01s)` | passed |
| BE-ERR-001 | typed-error-derives | integration-test | `rtk cargo test -p agent_scope_agent --test error_compat` | `AgentError` Display/source/From compatibility | Compatibility tests pass | `cargo test: 2 passed (1 suite, 0.00s)` | passed |
| BE-ERR-002 | typed-error-derives | integration-test | `rtk cargo test -p agent_scope_model --test error_compat` | `ModelError` Display/source/From/kind compatibility | Compatibility tests pass | `cargo test: 1 passed (1 suite, 0.00s)` | passed |
| BE-ERR-003 | typed-error-derives | integration-test | `rtk cargo test -p agent_scope_workspace --test error_compat` | `WorkspaceError` Display compatibility and source absence | Compatibility tests pass | `cargo test: 3 passed (1 suite, 0.00s)` | passed |
| BE-GOV-001 | all first-batch candidates | doc-check | `specs/028-dependency-simplification/dependency-evaluations.md` | Every introduced dependency has governance rationale | Dependency evaluation contract satisfied | `gray_matter`, `globset`, `walkdir`, and `thiserror` records include decision, version policy, maintenance, license, security, footprint, layering, compatibility wrapper, and behavior evidence sections | passed |
| BE-GOV-002 | all first-batch candidates | manual-review | `specs/028-dependency-simplification/implementation-notes.md` | No event/message/sandbox/provider streaming/persisted-state replacement | Review records zero out-of-scope semantic replacement | Implementation notes explicitly retain event protocol, message/content protocol, sandbox containment, provider streaming/SSE framing, and persisted session-state semantics out of scope | passed |
| BE-WORKSPACE-001 | all first-batch candidates | doc-check | `rtk cargo fmt --check` | Workspace formatting | Command passes | Command completed with no output | passed |
| BE-WORKSPACE-002 | all first-batch candidates | integration-test | `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings` | Workspace lint and warnings gate | Command passes | `cargo clippy: No issues found` | passed |
| BE-WORKSPACE-003 | all first-batch candidates | integration-test | `rtk cargo test --workspace --all-features` | Full workspace regression suite | Command passes | `cargo test: 1021 passed, 3 ignored (121 suites, 19.00s)` | passed |

## Compatibility Exceptions

None approved. Any failed behavior evidence blocks completion unless the affected candidate is deferred or a later spec explicitly approves an exception.

## Final Acceptance Checklist

- [x] At least 10 simplification candidates are reviewed and categorized.
- [x] At least 3 low-risk simplifications are implemented, or all unsuitable candidates are documented.
- [x] Every introduced or expanded dependency has a complete evaluation record.
- [x] Every implemented replacement has behavior evidence with `passed` status.
- [x] No event protocol, message protocol, sandbox containment, provider streaming, or persisted-state semantics were replaced.
- [x] Workspace `fmt`, `clippy`, and `test` gates passed.
- [x] No undocumented compatibility regressions remain.
