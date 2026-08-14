# Implementation Notes: Dependency Simplification

**Feature**: 028-dependency-simplification  
**Date**: 2026-08-11  
**Status**: Complete

## Current Workspace Dependency Baseline

Recorded before first-batch dependency additions.

### Root workspace dependencies

```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
chrono-tz = "0.10"
base64 = "0.22"
schemars = "0.8"
clap = { version = "4", features = ["derive", "env"] }
```

### Direct dependency observations before changes

- `agent_scope_tool` already has a direct `thiserror = "2"` dependency.
- `agent_scope_agent` already has a direct `thiserror = "2"` dependency.
- `examples/pi-rust` already has direct `regex = "1"` and `thiserror = "2"` dependencies.
- `agent_scope_memory` currently has `regex = "1"` for frontmatter parsing.
- `agent_scope_model` and `agent_scope_workspace` do not yet depend on `thiserror`.
- Root workspace dependencies do not yet centralize `thiserror`, `gray_matter`, `globset`, or `walkdir`.

## First-Batch Implementation Scope

Approved first-batch replacements are limited to:

1. Shared `SKILL.md` frontmatter parsing via `agent_scope_frontmatter`.
2. Memory markdown frontmatter parsing reuse through the same helper while preserving serialized layout.
3. pi-rust file discovery simplification using `globset` and `walkdir` wrappers.
4. Eligible typed error enum migration to `thiserror`.

Out of scope for this implementation batch:

- Event protocol model replacement.
- Message/content protocol replacement.
- Sandbox path containment replacement.
- Provider streaming/SSE framing replacement.
- Persisted session-state format replacement.
- Broad JSON repair/schema flattening replacement.

## Wrapper Boundaries

### `agent_scope_frontmatter`

The helper crate owns compatibility behavior rather than exposing raw parser semantics directly. It preserves:

- `SKILL.md` missing or malformed frontmatter fallback to empty metadata and original content body.
- `SKILL.md` delimiter handling for `---\n`, `\n---\n`, and EOF `\n---`.
- Rejection of malformed delimiter suffixes such as `---suffix`.
- Inline and quoted scalar extraction for `name` and `description`.
- Literal block scalar and folded block scalar compatibility for descriptions, including legacy trimming of trailing block-scalar newlines for skill descriptions.
- Memory frontmatter field map parsing with legacy scalar handling, including empty YAML fields as empty strings.
- Memory body layout after frontmatter, including CRLF normalization behavior.

Raw `gray_matter` semantics remain hidden behind this project-owned wrapper so callers depend on AgentScope compatibility rules rather than dependency-specific parsing details.

### pi-rust file discovery wrapper

`globset` and `walkdir` replace commodity matching/traversal mechanics only. Project-owned behavior remains in `examples/pi-rust/src/tools.rs`:

- Workspace path containment via `resolve_workspace_path`.
- Hidden dot entry skipping by default.
- Symlink skipping.
- Literal substring `Grep` semantics.
- Scan caps and result caps.
- Stable relative path output and deterministic ordering.
- Tool result shape, summary text categories, and truncation policy.

### `thiserror` migration

`thiserror` replaces boilerplate only. Project-owned compatibility remains:

- Public enum variants and field names.
- Byte-for-byte `Display` text for covered representative cases.
- Existing `source()` Some/None behavior.
- Existing `From` conversions, especially context-bearing conversions that `#[from]` cannot derive exactly.
- `ModelError::kind()` classification behavior.

## Code Reduction Tracking

| Candidate | Custom implementation reduced | Compatibility notes | Status |
|-----------|-------------------------------|---------------------|--------|
| skill-frontmatter-parser | Consolidated duplicated SKILL.md frontmatter parsing from `agent_scope_tool` and `agent_scope_workspace` into the shared `agent_scope_frontmatter` wrapper backed by `gray_matter`. | Guarded by `rtk cargo test -p agent_scope_frontmatter`, `rtk cargo test -p agent_scope_tool skill`, and `rtk cargo test -p agent_scope_workspace skill`; wrapper preserves malformed fallback, EOF delimiter handling, CRLF normalization, scalar parsing, block scalar folding/literal behavior, and legacy skill-body trimming. | complete |
| memory-frontmatter-parser | Removed the memory crate's local regex-based frontmatter parser and reused `agent_scope_frontmatter::parse_frontmatter_fields` / `body_after_frontmatter`; memory serialization layout remains project-owned. | Guarded by `rtk cargo test -p agent_scope_memory frontmatter`; empty YAML fields remain empty strings, quoted descriptions/tags round-trip, CRLF/EOF delimiter inputs remain readable, and serialized field names/body layout remain stable. | complete |
| pi-rust-file-discovery | Replaced hand-rolled glob regex matching and recursive traversal mechanics with `globset` and `walkdir` helpers while keeping pi-rust's policy checks in `examples/pi-rust/src/tools.rs`. | Guarded by `rtk cargo test -p pi-rust --test tools_file_discovery` and `rtk cargo test -p pi-rust tools`; relative output, `**/` matching, hidden skip, symlink skip, scan/result caps, literal grep semantics, and deterministic ordering are preserved. | complete |
| typed-error-derives | Migrated eligible `AgentError`, `ModelError`, and `WorkspaceError` display/error boilerplate to `thiserror` derives while retaining explicit conversions where needed. | Guarded by `rtk cargo test -p agent_scope_agent --test error_compat`, `rtk cargo test -p agent_scope_model --test error_compat`, and `rtk cargo test -p agent_scope_workspace --test error_compat`; public variants, representative `Display` text, `source()` behavior, `From` conversions, and `ModelError::kind()` mapping remain stable. | complete |

## Out-of-Scope Semantic Review

No implementation in this batch replaced the following governed semantics:

- Event protocol types, event ordering, or event serialization.
- Message/content protocol model or public content serialization.
- Sandbox path containment or workspace path security boundary.
- Provider streaming/SSE framing and provider-specific stream parsing semantics.
- Persisted session-state schema, file format, or load/save semantics.

Any future attempt to revisit those areas requires a separate spec and compatibility plan.

## Final Workspace Validation

- `rtk cargo fmt --check`: passed (command completed with no output).
- `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed (`cargo clippy: No issues found`).
- `rtk cargo test --workspace --all-features`: passed (`cargo test: 1021 passed, 3 ignored (121 suites, 19.00s)`).

## Governance Notes

No implementation in this feature may replace protocol, security-boundary, provider-streaming, or persisted-state semantics. Any future attempt to revisit those areas requires a separate spec and compatibility plan.
