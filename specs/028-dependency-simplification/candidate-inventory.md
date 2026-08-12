# Candidate Inventory: Dependency Simplification

**Feature**: 028-dependency-simplification  
**Review Date**: 2026-08-11  
**Spec**: [spec.md](./spec.md)  
**Dependency Evaluations**: [dependency-evaluations.md](./dependency-evaluations.md)  
**Behavior Evidence**: [behavior-evidence.md](./behavior-evidence.md)

## Summary Counts

### By decision status

| Status | Count |
|--------|-------|
| adopt | 4 |
| adopt-cautiously | 2 |
| defer | 5 |
| reject | 3 |
| **Total** | **14** |

### By risk level

| Risk | Count |
|------|-------|
| low | 1 |
| medium | 9 |
| high | 3 |
| security-critical | 1 |
| **Total** | **14** |

## First Batch

The first implementation batch contains these approved candidates:

1. `skill-frontmatter-parser`
2. `memory-frontmatter-parser`
3. `pi-rust-file-discovery`
4. `typed-error-derives`

This satisfies SC-001 by reviewing 14 candidates and SC-002 by identifying at least 3 low-risk or bounded first-batch simplifications with dependency evaluations and required behavior evidence.

## Candidate Records

### skill-frontmatter-parser

- **Title**: Shared `SKILL.md` frontmatter parser
- **Status**: adopt
- **Priority**: P1
- **Risk**: medium
- **Affected crates**: `agent_scope_tool`, `agent_scope_workspace`, new `agent_scope_frontmatter`
- **Affected files**:
  - `crates/agent_scope_tool/src/skill_loader.rs`
  - `crates/agent_scope_workspace/src/skill.rs`
  - `crates/agent_scope_frontmatter/src/lib.rs`
- **Current responsibility**: Two crates each parse `SKILL.md` frontmatter with mirrored custom parsing logic.
- **Commodity rationale**: Markdown/YAML frontmatter parsing is commodity behavior and duplication increases compatibility drift risk.
- **Public behavior to preserve**:
  - Inline `name` and `description` parsing.
  - Quoted scalar handling.
  - Literal and folded description block scalar behavior.
  - Missing or malformed frontmatter fallback.
  - Existing skill discovery skip/validation behavior in caller crates.
- **Replacement direction**: `gray_matter` behind `agent_scope_frontmatter` compatibility wrapper.
- **Dependency evaluations**: `gray_matter`
- **Evidence requirements**: BE-SKILL-001, BE-SKILL-002, BE-SKILL-003
- **Decision rationale**: Adopt because the behavior is duplicated commodity parsing and can be guarded by golden tests.
- **Alternatives considered**: Keep duplicated parsers; adopt a raw YAML parser directly in each crate; use deprecated `serde_yaml` directly.
- **Legacy compatibility notes**: Wrapper owns delimiter and fallback behavior; callers should not expose raw parser errors.

### memory-frontmatter-parser

- **Title**: Memory markdown frontmatter parser/serializer
- **Status**: adopt
- **Priority**: P1
- **Risk**: medium
- **Affected crates**: `agent_scope_memory`, new `agent_scope_frontmatter`
- **Affected files**:
  - `crates/agent_scope_memory/src/frontmatter.rs`
  - `crates/agent_scope_memory/tests/frontmatter_compat.rs`
- **Current responsibility**: Regex-based frontmatter extraction plus hand-written quoted scalar escaping/unescaping.
- **Commodity rationale**: Frontmatter field extraction is commodity parsing, but persisted layout remains project-owned.
- **Public behavior to preserve**:
  - Existing memory markdown field names.
  - Serialized field order and body layout.
  - Legacy quoted description and tags scalar handling.
  - CRLF and EOF delimiter compatibility.
  - Missing or malformed frontmatter skipping behavior.
- **Replacement direction**: Reuse `agent_scope_frontmatter` parsing while keeping serializer layout project-owned.
- **Dependency evaluations**: `gray_matter`
- **Evidence requirements**: BE-MEM-001, BE-MEM-002
- **Decision rationale**: Adopt because parser internals can be simplified while persistent file format remains stable.
- **Alternatives considered**: Serialize with a general YAML serializer; keep regex parser; migrate tags to YAML lists.
- **Legacy compatibility notes**: Serializer must not emit a different persisted schema or convert tags to YAML arrays.

### pi-rust-file-discovery

- **Title**: pi-rust Glob/Grep/ListDir traversal and matching
- **Status**: adopt
- **Priority**: P2
- **Risk**: medium
- **Affected crates**: `examples/pi-rust`
- **Affected files**:
  - `examples/pi-rust/src/tools.rs`
  - `examples/pi-rust/tests/tools_file_discovery.rs`
- **Current responsibility**: Hand-written DFS traversal and custom glob-to-regex conversion for file discovery tools.
- **Commodity rationale**: Directory traversal and glob matching are commodity responsibilities; tool safety and output policy remain project-owned.
- **Public behavior to preserve**:
  - Relative path outputs.
  - `**/` matching including zero nested directories.
  - Hidden dot entry skipping.
  - Symlink skipping.
  - Scan/result caps and deterministic ordering.
  - Literal substring Grep semantics.
- **Replacement direction**: `globset` for glob matching and `walkdir` for traversal behind compatibility helpers.
- **Dependency evaluations**: `globset`, `walkdir`
- **Evidence requirements**: BE-PI-001, BE-PI-002
- **Decision rationale**: Adopt because mature crates reduce custom traversal/matching code without changing path containment policy.
- **Alternatives considered**: Use `ignore` crate with gitignore semantics; keep manual DFS; use regex glob conversion.
- **Legacy compatibility notes**: Do not adopt gitignore-aware behavior in this batch because current tools only skip dot entries.

### typed-error-derives

- **Title**: Replace hand-written error impls with `thiserror`
- **Status**: adopt
- **Priority**: P2
- **Risk**: low
- **Affected crates**: `agent_scope_agent`, `agent_scope_model`, `agent_scope_workspace`
- **Affected files**:
  - `crates/agent_scope_agent/src/agent_error.rs`
  - `crates/agent_scope_model/src/model_error.rs`
  - `crates/agent_scope_workspace/src/error.rs`
- **Current responsibility**: Hand-written `Display`, `Error::source`, and selected `From` implementations.
- **Commodity rationale**: Deriving typed error boilerplate is a standard Rust practice.
- **Public behavior to preserve**:
  - Public enum variants and fields remain matchable.
  - Display strings remain stable.
  - Existing `source()` chains remain stable.
  - Existing `From` conversions remain stable.
  - `ModelError::kind()` remains stable.
- **Replacement direction**: `thiserror` derive while preserving context-bearing manual conversions where needed.
- **Dependency evaluations**: `thiserror`
- **Evidence requirements**: BE-ERR-001, BE-ERR-002, BE-ERR-003
- **Decision rationale**: Adopt because it removes boilerplate and the project already uses `thiserror` in some crates.
- **Alternatives considered**: Keep manual implementations; switch to untyped `anyhow`/`eyre`.
- **Legacy compatibility notes**: Do not add `#[source]` to variants that previously returned `None` from `source()`.

### path-component-sanitization

- **Title**: Consolidate safe filename/tool/path component policies
- **Status**: adopt-cautiously
- **Priority**: P3
- **Risk**: medium
- **Affected crates**: `agent_scope_embedding`, `agent_scope_rag`, `examples/pi-rust`
- **Affected files**: Candidate locations that normalize file names, tool names, or path components.
- **Current responsibility**: Multiple local helpers convert user/provider names into filesystem-safe or tool-safe components.
- **Commodity rationale**: Slug/filename sanitization is commodity, but collision and persisted-path policy is project-owned.
- **Public behavior to preserve**: Existing persisted paths, collision behavior, provider tool-name constraints, and migration safety.
- **Replacement direction**: Future wrapper around a filename/slug helper after collision policy is specified.
- **Dependency evaluations**: Required before implementation; not in first batch.
- **Evidence requirements**: Legacy path read/write tests, collision tests, provider tool-name tests.
- **Decision rationale**: Adopt cautiously because the area is a good simplification target but touches persisted paths.
- **Alternatives considered**: Keep local helpers; migrate all paths immediately; use raw dependency behavior.
- **Legacy compatibility notes**: Requires explicit migration or no-op wrapper before adoption.

### tool-context-cache

- **Title**: Replace manual Vec cache helper
- **Status**: adopt-cautiously
- **Priority**: P3
- **Risk**: medium
- **Affected crates**: `agent_scope_state` and callers that manage tool context cache behavior.
- **Affected files**: Tool context cache helper locations identified during future implementation.
- **Current responsibility**: Manual bounded cache behavior.
- **Commodity rationale**: LRU/indexed cache behavior can be delegated to a mature crate.
- **Public behavior to preserve**: Serialized state shape, eviction order, capacity handling, and deterministic replay semantics.
- **Replacement direction**: Future `indexmap` or `lru` wrapper if serialized/eviction behavior is pinned.
- **Dependency evaluations**: Required before implementation; not in first batch.
- **Evidence requirements**: Eviction-order and serialized-state compatibility tests.
- **Decision rationale**: Adopt cautiously because cache mechanics are commodity but state semantics may be externally observable.
- **Alternatives considered**: Keep Vec helper; use raw dependency type in serialized state.
- **Legacy compatibility notes**: Do not expose dependency type directly in public or persisted state.

### dashscope-sse-framing

- **Title**: Replace streaming SSE byte/line framing
- **Status**: defer
- **Priority**: P3
- **Risk**: medium
- **Affected crates**: `agent_scope_dashscope`
- **Affected files**: Provider streaming/SSE parsing implementation.
- **Current responsibility**: Provider-specific streaming event framing.
- **Commodity rationale**: SSE parsing can be commodity, but provider quirks and event ordering are compatibility-sensitive.
- **Public behavior to preserve**: Streaming event order, tool-call deltas, cancellation, and provider-specific fallback behavior.
- **Replacement direction**: Deferred pending provider-specific compatibility fixture set.
- **Dependency evaluations**: None for this batch.
- **Evidence requirements**: Recorded SSE fixture replay tests and streaming integration tests.
- **Decision rationale**: Defer because provider streaming semantics are explicitly out of scope for this low-risk batch.
- **Alternatives considered**: Adopt SSE parser now; keep local parser.
- **Legacy compatibility notes**: Requires separate spec if revisited.

### model-retry-backoff

- **Title**: Use retry/backoff abstraction for model calls
- **Status**: defer
- **Priority**: P3
- **Risk**: medium
- **Affected crates**: `agent_scope_model`, provider crates
- **Affected files**: Model retry loop implementations.
- **Current responsibility**: Local retry and backoff behavior.
- **Commodity rationale**: Retry scheduling is commodity, but model-provider semantics are compatibility-sensitive.
- **Public behavior to preserve**: Error classification, cancellation, retry count, delay policy, and provider error surfaces.
- **Replacement direction**: Deferred until retry policy contract is specified.
- **Dependency evaluations**: None for this batch.
- **Evidence requirements**: Fake-clock retry tests and provider error mapping tests.
- **Decision rationale**: Defer because retry behavior can affect user-visible latency and errors.
- **Alternatives considered**: Use a retry crate immediately; keep local loop.
- **Legacy compatibility notes**: Requires fake-time compatibility evidence.

### json-repair-and-schema-flatten

- **Title**: Replace JSON repair/schema flatten helpers
- **Status**: defer
- **Priority**: P3
- **Risk**: high
- **Affected crates**: `agent_scope_model`, `agent_scope_tool`, examples using structured output repair.
- **Affected files**: JSON repair and schema flattening helpers.
- **Current responsibility**: Compatibility-oriented JSON repair and schema shaping.
- **Commodity rationale**: Some JSON repair/schema transforms are commodity, but current behavior is provider-facing and model-output-sensitive.
- **Public behavior to preserve**: Structured-output fallback behavior, schema compatibility, and error reporting.
- **Replacement direction**: Deferred pending exhaustive model-output fixture coverage.
- **Dependency evaluations**: None for this batch.
- **Evidence requirements**: Golden fixtures for malformed model outputs and schema transformations.
- **Decision rationale**: Defer because broad JSON repair changes are high risk.
- **Alternatives considered**: Adopt JSON repair crate now; keep custom compatibility helpers.
- **Legacy compatibility notes**: Requires separate compatibility plan.

### json-file-session-store

- **Title**: Replace atomic JSON file persistence helper
- **Status**: defer
- **Priority**: P3
- **Risk**: high
- **Affected crates**: `agent_scope_state`
- **Affected files**: Session persistence helpers.
- **Current responsibility**: Atomic JSON state persistence.
- **Commodity rationale**: Atomic file writes can be commodity, but session file compatibility and durability semantics are project-owned.
- **Public behavior to preserve**: Existing JSON shape, atomicity, recovery behavior, and path layout.
- **Replacement direction**: Deferred pending persistence migration tests.
- **Dependency evaluations**: None for this batch.
- **Evidence requirements**: Crash/recovery tests and legacy file read tests.
- **Decision rationale**: Defer because persisted-state changes are out of scope.
- **Alternatives considered**: Adopt atomic-write crate; keep local persistence.
- **Legacy compatibility notes**: Requires explicit migration/no-migration decision.

### event-protocol-types

- **Title**: Replace AgentScope event protocol models
- **Status**: reject
- **Priority**: P3
- **Risk**: high
- **Affected crates**: `agent_scope_event`
- **Affected files**: Event protocol model definitions.
- **Current responsibility**: Stable AgentScope event protocol representation.
- **Commodity rationale**: Not commodity in this project; protocol compatibility is core product behavior.
- **Public behavior to preserve**: Serialization shape, event ordering, variant names, and Python AgentScope compatibility.
- **Replacement direction**: None.
- **Dependency evaluations**: Not applicable.
- **Evidence requirements**: Protocol compatibility tests if a later spec ever revisits this.
- **Decision rationale**: Reject because protocol models are project-owned compatibility boundaries, not basic implementation boilerplate.
- **Alternatives considered**: Use generic event crate; derive through external protocol types.
- **Legacy compatibility notes**: Must not be implemented under this feature.

### message-content-protocol

- **Title**: Replace message/content block models
- **Status**: reject
- **Priority**: P3
- **Risk**: high
- **Affected crates**: `agent_scope_message`
- **Affected files**: Message/content protocol models.
- **Current responsibility**: Stable message content representation.
- **Commodity rationale**: Not commodity because it encodes AgentScope interoperability semantics.
- **Public behavior to preserve**: Serialization shape, block ordering, role/content behavior, and provider formatting assumptions.
- **Replacement direction**: None.
- **Dependency evaluations**: Not applicable.
- **Evidence requirements**: Message compatibility fixtures if a later spec revisits this.
- **Decision rationale**: Reject because message protocol is a stable project API.
- **Alternatives considered**: External chat-message model crates; untyped JSON values.
- **Legacy compatibility notes**: Must not be implemented under this feature.

### sandbox-path-containment

- **Title**: Replace sandbox path containment policy
- **Status**: reject
- **Priority**: P3
- **Risk**: security-critical
- **Affected crates**: `agent_scope_sandbox`, workspace/path resolution callers
- **Affected files**: Sandbox and containment policy implementations.
- **Current responsibility**: Security-critical path traversal and containment enforcement.
- **Commodity rationale**: Path normalization helpers can assist, but containment policy itself is project-owned security behavior.
- **Public behavior to preserve**: Rejection of traversal, symlink escape protection, canonicalization behavior, and sandbox root enforcement.
- **Replacement direction**: None for containment policy.
- **Dependency evaluations**: Not applicable.
- **Evidence requirements**: Security regression tests if any helper is introduced later.
- **Decision rationale**: Reject because replacing containment policy would risk a security boundary regression.
- **Alternatives considered**: Path-cleaning crates; filesystem sandbox libraries.
- **Legacy compatibility notes**: Must not be implemented under this feature.

### mcp-internal-model-replacement

- **Title**: Replace internal tool/workspace abstractions with MCP-native types
- **Status**: defer
- **Priority**: P3
- **Risk**: high
- **Affected crates**: `agent_scope_mcp`, `agent_scope_tool`, `agent_scope_workspace`
- **Affected files**: MCP adapter and internal tool/workspace abstractions.
- **Current responsibility**: Project-owned adapter between internal abstractions and MCP SDK types.
- **Commodity rationale**: MCP SDK types are useful at boundaries, but internal model replacement would couple core abstractions to protocol implementation details.
- **Public behavior to preserve**: Tool schema shape, workspace resource behavior, MCP adapter compatibility, and dependency direction.
- **Replacement direction**: Deferred; keep boundary adapter model.
- **Dependency evaluations**: None for this batch.
- **Evidence requirements**: Adapter compatibility and layering review if revisited.
- **Decision rationale**: Defer because deeper MCP-native replacement may violate layering and duplicate responsibility constraints.
- **Alternatives considered**: Use MCP SDK types throughout; keep internal abstractions with adapter.
- **Legacy compatibility notes**: Requires separate architecture spec if revisited.

## Traceability

- SC-001: 14 reviewed candidates are listed above.
- SC-002: 4 first-batch candidates are approved for implementation.
- Dependency evaluations: `gray_matter`, `globset`, `walkdir`, and `thiserror` are recorded in [dependency-evaluations.md](./dependency-evaluations.md).
- Behavior evidence: Required evidence IDs are recorded in [behavior-evidence.md](./behavior-evidence.md).
