# Research: Memory System Design Decisions

**Feature**: 009-memory-system  
**Date**: 2026-07-30  

## Decision 1: Memory Trait Method Signatures

**Decision**: `async_trait` with `&self` methods, `Box<dyn Error>` error handling via typed `MemoryError`.

**Rationale**:
- `async_trait` matches the existing `ChatModel` and `Middleware` trait patterns in the codebase.
- All I/O operations (file read/write, model calls) are async.
- `&self` allows `Arc<dyn Memory>` sharing across middleware and agent.
- Typed `MemoryError` enum per Constitution §13, not `Box<dyn Error>`.

**Alternatives considered**:
- `&mut self` for write operations: rejected because it prevents sharing `Arc<dyn Memory>`; write operations use internal synchronization (tokio::sync::Mutex on index file write).
- Synchronous trait: rejected because model-based retrieval requires async.

---

## Decision 2: File Format — Markdown + YAML Frontmatter

**Decision**: Each memory stored as a `.md` file with YAML-like frontmatter delimiters (`---`). Frontmatter fields: `name`, `description`, `type`, `created_at`, `updated_at`, `tags`.

**Rationale**:
- Directly compatible with Python AgentScope's `AgenticMemoryMiddleware` format.
- Human-readable and editable.
- LLMs natively understand Markdown, which aids retrieval quality.
- Frontmatter is parseable with simple regex (avoid full YAML parser dependency).

**Alternatives considered**:
- JSON files: rejected because LLMs process Markdown better for retrieval.
- SQLite database: rejected because file-per-memory matches Python reference and allows external editing.
- Full YAML parser (`serde_yaml`): rejected to minimize dependencies; regex-based `key: value` parsing suffices for flat frontmatter.

---

## Decision 3: Backend Abstraction

**Decision**: `Backend` trait with async methods for file I/O. `LocalBackend` uses `tokio::fs`. Trait is designed for future remote storage but only `LocalBackend` is implemented now.

**Rationale**:
- Python AgentScope uses a `BackendBase` abstraction supporting local and remote storage.
- Trait-based approach enables future MCP/sandbox storage without changing memory logic.
- `LocalBackend` is trivial (`tokio::fs::read/write/read_dir`).
- Trait is `Send + Sync` to support `Arc<dyn Backend>`.

**Alternatives considered**:
- Direct `tokio::fs` calls in `FileMemory`: rejected because it couples storage to local filesystem, blocking future remote backends.
- `object_store` crate: rejected as overengineered for current scope (no S3/GCS needed yet).

---

## Decision 4: Middleware Integration Strategy

**Decision**: Extend existing `Middleware` trait with `on_system_prompt` hook. Use existing `pre_reply` for async retrieval task launch and `pre_reasoning` for result injection. Do NOT introduce generator-style `on_reply`/`on_reasoning` hooks in this feature.

**Rationale**:
- The Python `AgenticMemoryMiddleware` uses generator-based `on_reply`/`on_reasoning` hooks, but the current Rust `Middleware` trait uses mutable reference hooks (`pre_reply`, `pre_reasoning`).
- `on_system_prompt` hook is missing from the current trait and is needed for memory instruction + index injection.
- Async retrieval can be implemented by:
  1. `pre_reply`: spawn `tokio::spawn` for retrieval task, store `JoinHandle` in middleware instance.
  2. `pre_reasoning`: poll the stored `JoinHandle`; if complete, inject `HintBlock` into messages.
- Generator-based hooks (yielding events) are a larger architectural change deferred to Feature 009-advanced or when streaming middleware is prioritized.

**Alternatives considered**:
- Full generator-based `Middleware` overhaul: rejected as scope creep — this would require rewriting all existing middleware dispatch in `ReActAgent`.
- Separate `MemoryMiddleware` trait: rejected because it fragments the extension interface; better to extend the existing `Middleware` trait.
- Synchronous retrieval only: rejected because concurrent retrieval + model call is a key feature of Python's `AgenticMemoryMiddleware`.

---

## Decision 5: Index Truncation

**Decision**: Token-based truncation using `count_tokens` from `ChatModel` trait (bytes/4 heuristic). Truncate from the end (keep first N tokens of the index). Append `<<<TRUNCATED>>>` notice with remaining line count.

**Rationale**:
- Matches Python AgentScope's approach of keeping the most-recently-referenced files (front of index).
- Reuses existing `ChatModel::count_tokens` method.
- Simple: scan lines, accumulate tokens, stop when budget exceeded.

**Alternatives considered**:
- LRU-based truncation: more complex, requires tracking access times; deferred.
- Priority-based truncation: requires additional metadata; overengineered for v1.

---

## Decision 6: Frontmatter Parsing Approach

**Decision**: Regex-based extraction: match `---\n...\n---` block, extract `key: value` lines. No full YAML parser. Gracefully skip files with malformed frontmatter.

**Rationale**:
- Python reference uses regex-based parsing (`_FRONTMATTER_RE`, `_FIELD_RE`).
- Avoids adding `serde_yaml` dependency.
- Malformed files are skipped rather than causing errors — consistent with Constitution §12 (robustness to unknown formats).
- Only scalar key-value pairs are needed; nested structures in frontmatter are not used.

**Alternatives considered**:
- `serde_yaml`: rejected to minimize dependency footprint.
- Custom parser crate: overkill for ~20 lines of regex.

---

## Decision 7: Crate Dependency Graph

**Decision**: New `agent_scope_memory` crate depends on:
- `agent_scope_message` — `ContentBlock`, `HintBlock`, `HintContent`
- `agent_scope_model` — `ChatModel` (for token counting, structured output)
- `serde`, `serde_json`, `uuid`, `regex`, `tokio`, `chrono`

`agent_scope_agent` depends on `agent_scope_memory` (for `MemoryMiddleware`).

**Rationale**:
- Follows Constitution §11: core abstractions (Memory trait) are independent of agent infrastructure.
- `agent_scope_memory` is a pure library crate — no agent coupling.
- `MemoryMiddleware` is the bridge that connects memory to agent, and belongs in the agent crate.

**Alternatives considered**:
- Single `agent_scope_memory` crate with built-in middleware: rejected because middleware depends on `Middleware` trait from `agent_scope_agent`, creating a circular dependency.

---

## Summary

| # | Decision | Choice | Key Reason |
|---|----------|--------|------------|
| 1 | Trait signature | `async_trait`, `&self`, `MemoryError` | Matches existing patterns |
| 2 | File format | Markdown + YAML frontmatter | Python compatibility |
| 3 | Backend | Trait + LocalBackend only | Future extensibility |
| 4 | Middleware integration | Extend existing `Middleware` trait | Avoid scope creep |
| 5 | Index truncation | Token-based, keep-first-N | Matches Python reference |
| 6 | Frontmatter parser | Regex-based | Minimal dependencies |
| 7 | Dep graph | `agent_scope_memory` ← msg, model | Constitution §11 compliance |
