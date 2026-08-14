# Research: Dependency Simplification

**Feature**: Dependency Simplification  
**Date**: 2026-08-11  
**Spec**: [spec.md](./spec.md)

## Research Scope

This research inventories basic in-project implementations that could be simplified with mature third-party crates or dependency-policy consolidation. The feature intentionally targets commodity helpers and infrastructure, not AgentScope compatibility semantics, public wire formats, or security boundaries.

Decision statuses:

- **adopt**: Suitable for the first implementation phase when paired with compatibility tests and dependency review.
- **adopt cautiously**: Suitable only behind a project-local compatibility wrapper or legacy migration path.
- **defer**: Valid simplification direction, but not safe for the first phase without deeper compatibility/platform research.
- **reject**: Do not replace with a third-party abstraction; the code defines project-specific protocol, compatibility, or security behavior.

## Candidate Inventory

### Decision: Adopt shared YAML/frontmatter parsing for `SKILL.md`

**Affected area**: skill loading in `agent_scope_tool` and `agent_scope_workspace`  
**Current implementation**:

- `crates/agent_scope_tool/src/skill_loader.rs` parses `---` frontmatter manually in `parse_skill_md`.
- `crates/agent_scope_workspace/src/skill.rs` mirrors the same logic exactly.
- Both implementations extract `name`, `description`, and body, including a custom subset of YAML block scalar behavior for `description: |` and `description: >`.

**Rationale**:

YAML frontmatter splitting, quoted scalar handling, and block scalar folding are commodity parsing concerns. The current duplicate implementation has already required edge-case fixes and must remain synchronized across crates. A shared helper backed by a mature YAML/frontmatter parser can reduce duplicated code and future parsing drift.

**Preferred replacement direction**:

- Use a project-local helper API for skill frontmatter parsing.
- Internally evaluate `serde_yaml`, `gray_matter`, or `yaml-front-matter`.
- Preserve the existing fallback contract: malformed or missing frontmatter should return empty metadata and leave content usable rather than turning skill discovery into a hard failure.

**Compatibility evidence required**:

- Golden tests for inline `name`/`description`.
- Golden tests for quoted values.
- Golden tests for `description: |` and `description: >` block scalar behavior.
- Malformed frontmatter tests that preserve current graceful fallback behavior.
- Cross-crate tests proving tool and workspace skill discovery still agree.

**Alternatives considered**:

- Keep both hand-written parsers: rejected because duplication is already documented as a maintenance hazard.
- Only extract the current parser into a shared helper: acceptable as a staging step, but does not fully satisfy dependency simplification unless paired with a vetted parser or explicit rationale for keeping the subset.

### Decision: Adopt shared YAML/frontmatter support for memory files

**Affected area**: `agent_scope_memory` markdown memory persistence  
**Current implementation**:

- `crates/agent_scope_memory/src/frontmatter.rs` uses regexes to find YAML frontmatter fields.
- `yaml_quote` and `yaml_unescape` implement a small YAML scalar subset manually.
- `body_after_frontmatter` performs custom frontmatter/body splitting.

**Rationale**:

Memory frontmatter parsing and scalar escaping are commodity YAML concerns, and they overlap with skill frontmatter needs. A single reviewed frontmatter dependency plus project-local compatibility wrapper can reduce custom parsing code while keeping the persisted memory format stable.

**Preferred replacement direction**:

- Reuse the same project-local frontmatter helper selected for `SKILL.md` where practical.
- Preserve current persisted field names: `name`, `description`, `type`, `created_at`, `updated_at`, and optional `tags`.
- Preserve legacy `tags` as a comma-separated scalar unless a migration is explicitly designed later.

**Compatibility evidence required**:

- Round-trip tests for existing memory markdown shape.
- Tests for quoted descriptions with backslashes, quotes, and newlines.
- Tests for CRLF and end-of-file delimiter cases.
- Tests that existing memory files remain readable without migration.

**Alternatives considered**:

- Switch directly to full YAML document serialization: rejected for this feature because it could alter persistent file shape.
- Keep regex parser unchanged: rejected as a first-choice direction because the parser duplicates frontmatter concerns already found elsewhere.

### Decision: Adopt `ignore`/`walkdir` plus `globset` for pi-rust file discovery

**Affected area**: `examples/pi-rust/src/tools.rs`  
**Current implementation**:

- `glob_to_regex` manually translates `*`, `**`, `**/`, and `?` into regex fragments.
- `glob_tool` manually performs DFS, skips hidden entries and symlinks, tracks scan/result caps, and returns relative paths.
- `grep_tool` manually walks directories, filters hidden/symlink entries, checks file size/binary bytes, and truncates output.
- `list_dir_tool` manually reads and sorts directory entries.

**Rationale**:

Recursive directory walking, hidden/gitignore-aware traversal, symlink policy, and glob compilation are commodity file-search concerns. The current implementation is useful but accumulates infrastructure details in a demo agent. Mature crates can simplify code while preserving the tool contracts through a thin project-local wrapper.

**Preferred replacement direction**:

- Use `globset` for glob compilation/matching.
- Use `ignore` when gitignore-aware traversal is desired, or `walkdir` if current behavior must remain exactly “skip dot-prefixed hidden entries, do not follow symlinks”.
- Keep public tool schemas and output shapes unchanged.

**Compatibility evidence required**:

- Golden tests for `**/` matching zero or more path segments.
- Tests for hidden file behavior.
- Tests for symlink skipping.
- Tests for scan caps, result caps, relative path shape, and deterministic sort order.
- Grep tests proving default matching remains literal if current behavior is literal.

**Alternatives considered**:

- Use ripgrep crates for all grep behavior: deferred unless literal-vs-regex semantics are explicitly specified.
- Keep hand-written traversal: acceptable for very small examples, but lower maintainability and more edge-case surface.

### Decision: Adopt `thiserror` for remaining hand-written error enums

**Affected area**: core error modules, including agent/model/workspace error types  
**Current implementation**:

- Several crates still hand-write `fmt::Display`, `std::error::Error::source`, and `From` conversions.
- The workspace already uses or tolerates `thiserror` in some areas, so the dependency is not conceptually new to the project.

**Rationale**:

Error derivation is a low-risk commodity concern. Using `thiserror` reduces maintenance burden and makes it harder to forget source chaining or conversion implementations, while preserving enum variants and display strings.

**Preferred replacement direction**:

- Move eligible error enums to `#[derive(thiserror::Error)]`.
- Keep display messages byte-for-byte compatible where tests or examples assert them.
- Do not change error enum variant names or public matching semantics.

**Compatibility evidence required**:

- Compile-time checks for public enum usage.
- Unit tests for representative `Display` strings and `source()` chains.
- Existing crate tests and examples.

**Alternatives considered**:

- `anyhow`/`eyre`: rejected for public/library error enums because they erase typed error contracts.
- Keep manual impls: acceptable but lower value where `thiserror` can express the same behavior declaratively.

### Decision: Adopt cautiously a stable helper for path/filename component sanitization

**Affected area**: workspace offload, pi-rust sessions/tools, RAG middleware/tool names, embedding cache keys  
**Current implementation**:

- Multiple modules manually replace unsafe characters with `_` or `-`.
- Some implementations append hashes or otherwise preserve uniqueness.
- Similar concepts appear as `sanitize_component`, `safe_component`, `sanitize_memory_name`, `sanitize_kb_name`, and cache key path conversion.

**Rationale**:

Filename/path component sanitization is a commodity concern, and duplicated local variants can drift in Unicode, collision, and platform behavior. However, these functions often feed persisted paths or public tool names, so direct algorithm replacement could break existing data.

**Preferred replacement direction**:

- First introduce a project-local sanitization contract with named policies, for example `safe_filename_component`, `safe_tool_component`, and `legacy_cache_key_component`.
- Evaluate `sanitize-filename`, `slug`, `slugify`, or `deunicode` for the non-legacy internals.
- Preserve legacy persisted mappings where existing file lookup depends on them.

**Compatibility evidence required**:

- Tests for empty input, Unicode, path separators, Windows-reserved characters, and collision handling.
- Tests that existing persisted files remain discoverable.
- Tests that tool names still satisfy provider/function-name constraints.

**Alternatives considered**:

- Replace all sanitizers with one crate call: rejected because persisted path and tool-name semantics differ.
- Leave all copies in place: rejected as a long-term direction because duplicate policies invite inconsistent behavior.

### Decision: Adopt cautiously an internal cache helper for `ToolContext` read-file cache

**Affected area**: `crates/agent_scope_state/src/agent_state.rs`  
**Current implementation**:

- `ToolContext.read_file_cache` is a `Vec<ReadCacheEntry>` with manual lookup, removal, and size eviction.
- Comments describe LRU-like behavior, but current access does not necessarily refresh recency.

**Rationale**:

Size-limited cache eviction is a commodity concern. A mature structure such as `indexmap` or a small wrapper around an LRU crate can make intent clearer and avoid O(n) front removals. Because the state is serializable, the replacement must not leak third-party cache types into persisted format without a migration plan.

**Preferred replacement direction**:

- Prefer an internal helper that can convert to/from the current serialized `Vec<ReadCacheEntry>` shape.
- Consider `indexmap` first if serialized order compatibility matters.
- Consider `lru` only if true LRU behavior is explicitly accepted and tests are updated.

**Compatibility evidence required**:

- Serialization shape tests for agent state.
- Eviction tests for `max_cache_files` and `max_cache_bytes`.
- Tests documenting whether reads refresh recency.

**Alternatives considered**:

- Directly expose `lru::LruCache` in state: rejected unless serialization and behavior changes are approved.
- Keep current `Vec`: acceptable short-term, but the behavior/name mismatch should be documented or fixed.

### Decision: Defer DashScope SSE framing replacement

**Affected area**: `crates/agent_scope_dashscope/src/model.rs`  
**Current implementation**:

- Streaming response parsing manually buffers `bytes_stream()` chunks.
- SSE line ingestion manually handles `\n`, CRLF trimming, strict UTF-8, `data:` lines, `[DONE]`, heartbeat/comment lines, and provider error events.

**Rationale**:

SSE framing is a standard protocol with mature crates such as `eventsource-stream` or `reqwest-eventsource`. However, DashScope/OpenAI-compatible streaming often has provider-specific deviations and the payload-to-`ChatResponse` mapping is project-specific. The current byte and line behavior has compatibility implications for streaming event order and error propagation.

**Deferred direction**:

- Only consider replacing the byte/line framing layer, not provider chunk mapping.
- Require real DashScope streaming tests before adopting.

**Alternatives considered**:

- Full replacement with an eventsource client: deferred because it may change connection/error semantics.
- Keep current parser: acceptable until a targeted streaming refactor is planned.

### Decision: Defer model retry/backoff abstraction

**Affected area**: `crates/agent_scope_model/src/model_trait.rs` and provider retry configuration  
**Current implementation**:

- `ChatModel::call` performs a simple bounded retry loop with fixed delay and retryable error classification.

**Rationale**:

Async retry/backoff can be handled by crates such as `backoff`, `tokio-retry`, or `tower::retry`. But default retry timing is externally observable in rate limit and outage scenarios. Introducing exponential backoff or jitter may improve operations but changes behavior beyond simple simplification.

**Deferred direction**:

- Keep current fixed-delay defaults unless a separate behavior-change spec approves backoff semantics.
- If adopted later, wrap the crate so existing `max_retries()` and `retry_delay()` defaults remain compatible.

**Alternatives considered**:

- Immediate `backoff` adoption: deferred due to timing compatibility.

### Decision: Defer JSON repair consolidation; reject broad JSON repair replacement for first phase

**Affected area**: `crates/agent_scope_tool/src/json_repair.rs`, `crates/agent_scope_model/src/json_repair.rs`, and schema flattening in `crates/agent_scope_model/src/schema_flat.rs`  
**Current implementation**:

- JSON repair code accepts valid JSON unchanged, generates conservative repair candidates, and for tool inputs requires repaired values to be JSON objects.
- Schema flattening expands a bounded subset of local `$defs` references with limits for nodes, bytes, depth, and expansion count.

**Rationale**:

JSON repair and schema transformation affect provider compatibility, tool-call tolerance, and structured-output behavior. Mature libraries may repair more aggressively or validate schemas differently, which could silently widen or narrow accepted model outputs.

**Deferred/rejected direction**:

- Consolidate duplicate local JSON repair helpers only after tests lock current behavior.
- Do not adopt a broad JSON repair crate as a first-batch simplification.
- Do not replace schema flattening with a validator-focused JSON Schema crate unless the output shape remains provider-compatible.

**Alternatives considered**:

- Use a JSON repair crate directly: rejected for first phase due to high compatibility risk.
- Use `jsonschema` resolver directly: deferred because the project needs flattened provider payloads, not just validation.

### Decision: Defer JSON file session-store write abstraction

**Affected area**: `crates/agent_scope_state/src/json_file_store.rs`  
**Current implementation**:

- Session state persistence performs session-id validation, atomic temporary-file write, fsync, rename, and compatibility handling for old/new fields.

**Rationale**:

Atomic file writes can be simplified with crates such as `atomic-write-file` or `tempfile`, but durability and rename semantics vary by platform and filesystem. The current code protects persisted agent state and path traversal boundaries.

**Deferred direction**:

- Consider a dependency only if it can preserve current fsync/rename behavior and session-id security checks.
- Keep persisted JSON shape unchanged.

**Alternatives considered**:

- Immediate atomic-write crate adoption: deferred pending platform behavior verification.

### Decision: Reject replacement of event protocol types

**Affected area**: `agent_scope_event`  
**Current implementation**:

- Event types define public AgentScope event protocol, including tagged serde serialization, event ordering, stream replay, and conversion back into messages.

**Rationale**:

This is not commodity infrastructure. It is a project compatibility boundary and wire format. Replacing it with a generic event-bus or telemetry crate would risk serialization tags, replay behavior, and UI/tooling expectations.

**Alternatives considered**:

- Use a generic event framework: rejected because it does not preserve AgentScope event protocol semantics.

### Decision: Reject replacement of message content-block protocol types

**Affected area**: `agent_scope_message`  
**Current implementation**:

- Message and content block types define public serialized shapes and provider interoperability, including tagged/untagged serde behavior, raw tool-call JSON, and provider-specific extras.

**Rationale**:

These types are stable data protocol, not a basic helper. They must remain project-owned to preserve Python compatibility and provider behavior.

**Alternatives considered**:

- Replace with a third-party chat-message model: rejected because external models do not encode AgentScope-specific compatibility semantics.

### Decision: Reject replacement of sandbox path containment logic

**Affected area**: `crates/agent_scope_sandbox/src/path.rs` and related workspace containment checks  
**Current implementation**:

- Path handling rejects traversal, canonicalizes ancestors, handles symlink containment, and protects workspace/sandbox roots.

**Rationale**:

This code protects a security boundary and has known symlink edge cases. A generic path utility can help ergonomics, but cannot replace the project’s security policy or validation order.

**Alternatives considered**:

- Use a generic path traversal prevention crate: rejected because security behavior must stay explicit and test-covered.
- Use `camino` for UTF-8 path ergonomics: possible future enhancement, not a replacement of containment policy.

### Decision: Defer deeper MCP model replacement

**Affected area**: `agent_scope_mcp`  
**Current implementation**:

- The project already uses `rmcp = 3.1.1` for MCP client integration while preserving AgentScope `Tool` and workspace abstractions through an adapter layer.

**Rationale**:

The MCP boundary already depends on an external protocol crate, and its dependency versions require careful governance. Replacing internal tool/workspace abstractions with MCP-native models would collapse a deliberate adapter boundary.

**Deferred direction**:

- Keep `rmcp` as the external protocol dependency.
- Preserve AgentScope adapters until MCP/rmcp APIs are stable enough for deeper integration.

**Alternatives considered**:

- Make MCP types the internal tool model: rejected for this feature because it violates layering and compatibility boundaries.

## Dependency Evaluation Policy

Every adopted dependency must be evaluated before implementation tasks are generated:

1. **Maintenance health**: recent release history, issue activity, Rust edition compatibility, and ecosystem usage.
2. **License compatibility**: must be compatible with Apache-2.0 project distribution.
3. **Security posture**: no known unresolved advisories for the selected version and no unnecessary unsafe exposure.
4. **Transitive dependency footprint**: dependency tree increase must be justified by code removal and behavior reliability.
5. **Compatibility fit**: dependency behavior must be wrapped or configured to preserve public APIs, serialized shapes, event ordering, errors, cancellation, and examples.
6. **Layering fit**: dependency must not introduce reverse dependencies, provider/core coupling, or duplicate responsibility without documented approval.

## First Implementation Batch Recommendation

The first batch should target low-risk, high-value simplifications:

1. Shared skill frontmatter parsing.
2. Memory frontmatter parsing/serialization, if compatibility tests are written first.
3. pi-rust `Glob`/directory traversal using `globset` and `walkdir`/`ignore`.
4. Remaining typed error enums using `thiserror`.

The batch should not include event protocol replacement, message protocol replacement, sandbox containment replacement, broad JSON repair replacement, or schema flattening replacement.
