# Quickstart: Dependency Simplification Validation

**Feature**: Dependency Simplification  
**Date**: 2026-08-11  
**Spec**: [spec.md](./spec.md)

## Purpose

Use this guide to validate that dependency-driven simplifications reduce custom basic implementation code without changing AgentScope Rust public behavior, compatibility semantics, persisted formats, or safety boundaries.

## Prerequisites

- Work from the repository root.
- Use `rtk` prefixes for shell commands in this project.
- Complete dependency evaluation records before adding or expanding third-party crates.
- Treat Python AgentScope compatibility, public serialization shape, event order, error categories, cancellation behavior, and security boundaries as non-negotiable unless a later spec explicitly approves an exception.

## Planning Validation

### 1. Confirm candidate inventory scope

Review the planning artifacts:

```text
specs/028-dependency-simplification/spec.md
specs/028-dependency-simplification/research.md
specs/028-dependency-simplification/data-model.md
specs/028-dependency-simplification/contracts/dependency-evaluation-contract.md
```

Expected outcome:

- At least 10 candidates are categorized as `adopt`, `adopt-cautiously`, `defer`, or `reject`.
- At least 3 candidates are suitable for first-batch implementation or all candidates are documented as unsuitable.
- High-risk protocol/security candidates are explicitly deferred or rejected.

### 2. Complete dependency evaluations before implementation

For every adopted candidate that introduces or expands a dependency, fill the contract in:

```text
specs/028-dependency-simplification/contracts/dependency-evaluation-contract.md
```

Expected outcome:

- Maintenance health is documented.
- License compatibility with Apache-2.0 distribution is documented.
- Security/advisory posture is documented.
- Transitive dependency footprint and feature flags are documented.
- Layering and duplicate-responsibility checks pass.
- Compatibility wrapper requirements are clear.

## Implemented First-Batch Validation Scenarios

### Scenario A: Shared skill frontmatter parsing

Candidate: `skill-frontmatter-parser`

Validation focus:

- Inline frontmatter values.
- Quoted scalar values.
- `description: |` block scalar behavior.
- `description: >` folded scalar behavior.
- Missing or malformed frontmatter fallback.
- Agreement between `agent_scope_tool` and `agent_scope_workspace` skill discovery behavior.
- CRLF and EOF-delimited frontmatter compatibility.

Required checks after implementation:

```bash
rtk cargo test -p agent_scope_frontmatter
rtk cargo test -p agent_scope_tool skill
rtk cargo test -p agent_scope_workspace skill
```

Expected outcome:

- Existing valid skills load unchanged.
- Malformed skills preserve current graceful fallback behavior.
- No crate dependency cycle is introduced.
- The raw YAML parser remains hidden behind `agent_scope_frontmatter`.

### Scenario B: Memory frontmatter compatibility

Candidate: `memory-frontmatter-parser`

Validation focus:

- Existing memory markdown remains readable.
- Serialized memory files keep the documented field names and body layout.
- Legacy `tags` scalar handling remains compatible.
- Quoting/unescaping behavior is preserved for descriptions and tags.
- Empty YAML fields remain empty strings.

Required checks after implementation:

```bash
rtk cargo test -p agent_scope_memory frontmatter
```

Expected outcome:

- Round-trip tests pass.
- Legacy-read tests pass.
- No silent data loss occurs for unknown or malformed frontmatter.

### Scenario C: pi-rust Glob/Grep/ListDir simplification

Candidate: `pi-rust-file-discovery`

Validation focus:

- `Glob` returns relative paths with stable ordering.
- `**/` behavior remains compatible, including zero-directory matches.
- Hidden and symlink behavior remains documented and tested.
- Result caps and scan caps still protect large trees.
- `Grep` default matching semantics remain literal substring matching.

Required checks after implementation:

```bash
rtk cargo test -p pi-rust --test tools_file_discovery
rtk cargo test -p pi-rust tools
```

Expected outcome:

- Tool output shape remains compatible with the demo agent UI and approval flow.
- Large directory scans remain bounded.
- No new path traversal or symlink-following behavior appears.

### Scenario D: `thiserror` migration for typed errors

Candidate: `typed-error-derives`

Validation focus:

- Public error enum variants remain matchable by downstream code.
- `Display` messages remain stable for representative cases.
- `source()` chains and `From` conversions remain correct.
- `ModelError::kind()` remains stable.

Required checks after implementation:

```bash
rtk cargo test -p agent_scope_agent --test error_compat
rtk cargo test -p agent_scope_model --test error_compat
rtk cargo test -p agent_scope_workspace --test error_compat
```

Expected outcome:

- Errors remain typed.
- No public error is replaced with untyped `anyhow`/`eyre` in library APIs.
- Existing examples compile.

## Workspace-Level Verification Commands

Run these after any first-batch implementation:

```bash
rtk cargo fmt --check
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk cargo test --workspace --all-features
```

If documentation or examples change, also run the relevant example/package commands. Keep command output as Behavior Preservation Evidence.

## Acceptance Checklist

Before marking an implementation candidate complete:

- [ ] Candidate has an `adopt` or `adopt-cautiously` decision.
- [ ] Dependency evaluation is complete or the change uses only existing dependencies.
- [ ] Public behavior and compatibility evidence are mapped to tests or review artifacts.
- [ ] No event protocol, message protocol, sandbox containment, provider streaming, persisted-state, or provider-specific stream semantics are replaced without a separate spec.
- [ ] All applicable targeted tests pass.
- [ ] Workspace `fmt`, `clippy`, and full test gates pass.
- [ ] Any documentation changes are updated.
- [ ] Code reduction or maintainability benefit is recorded.

## Expected Completion Outcome

A completed implementation batch should satisfy the feature success criteria by showing:

- at least 10 reviewed candidates,
- at least 3 completed low-risk simplifications or documented rationale that no candidate is safe,
- zero undocumented compatibility regressions,
- documented dependency governance for every newly introduced dependency,
- no dependency-direction violations.
