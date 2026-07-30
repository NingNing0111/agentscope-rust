# Implementation Plan: Memory System

**Branch**: `009-memory-system` | **Date**: 2026-07-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/009-memory-system/spec.md`

## Summary

Implement a persistent long-term memory system for AgentScope Rust agents. The system provides a `Memory` trait with file-based storage (`FileMemory`), manages a `MEMORY.md` index for efficient context-window usage, supports LLM-driven relevance retrieval, and integrates with the agent lifecycle via `MemoryMiddleware`. This is the 8th capability module per Constitution §16 (small-step delivery).

**Technical approach**: New `agent_scope_memory` crate with `Memory` trait + `FileMemory` implementation using a `Backend` abstraction. Extend the existing `Middleware` trait with `on_system_prompt` hook. MemoryMiddleware uses `pre_reply` for async retrieval initiation and `pre_reasoning` for result injection via `HintBlock`.

## Technical Context

**Language/Version**: Rust 2024 Edition (workspace), MSRV per workspace settings

**Primary Dependencies**:
- `agent_scope_message` — `ContentBlock`, `HintBlock`, `Msg` types
- `agent_scope_model` — `ChatModel` trait (for `retrieve_relevant`)
- `agent_scope_agent` — `Middleware` trait (extended with `on_system_prompt`)
- `serde`, `serde_json` — serialization
- `tokio` — async runtime, filesystem
- `uuid` — unique ID generation
- `regex` — frontmatter parsing (already in workspace)

**Storage**: Filesystem via `LocalBackend` (markdown files + `MEMORY.md` index); `Backend` trait for future remote storage

**Testing**: `cargo test`, per-crate `tests/` directory pattern, mock model for retrieval tests

**Target Platform**: All platforms supported by tokio (Linux, macOS, Windows)

**Project Type**: Library crate (`agent_scope_memory`)

**Performance Goals**:
- 1000 memory files: listing + index generation < 500ms
- 10 entry write+search: < 1 second (excluding model calls)
- Middleware system prompt injection: < 50ms overhead
- Index truncation: O(n) scan, single-pass

**Constraints**:
- `#![deny(unsafe_code)]` on all modules (Constitution §9)
- No circular dependencies (Constitution §11)
- Frontmatter format compatible with Python AgentScope `AgenticMemoryMiddleware`
- MemoryMiddleware must not hold Read/Write locks across `.await` points

**Scale/Scope**:
- Anticipated memory stores: 10-1000 files
- Index tokens: configurable, default 4000
- 4 memory types: User, Feedback, Project, Reference

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | Principle | Status | Notes |
|---|-----------|--------|-------|
| §1 | 兼容性优先 | ✅ PASS | Frontmatter format, memory types, index structure match Python `AgenticMemoryMiddleware` |
| §2 | 锁定上游版本 | ✅ PASS | Reference implementation committed in `agentscope/` repo |
| §3 | Python 是行为基准 | ✅ PASS | Retrieval behavior and frontmatter parsing match Python reference |
| §4 | 先定义契约 | ✅ PASS | Spec exists (`spec.md`), contracts defined in `/contracts/` |
| §5 | 不允许伪兼容 | ✅ PASS | No stub/silent-degrade behavior planned; FRs are concrete |
| §6 | 测试驱动兼容性 | ✅ PASS | Mock model for retrieval tests; file-based unit tests for CRUD |
| §7 | Trace 是核心验收产物 | ✅ PASS | Memory writes/deletes/index updates are observable and testable |
| §8 | Rust 原生设计 | ✅ PASS | Trait-based `Memory`, `Backend`; enum for `MemoryType`; `Result<T, E>` |
| §9 | 安全 Rust 优先 | ✅ PASS | No unsafe code needed; `#![deny(unsafe_code)]` applied |
| §10 | 结构化并发 | ✅ PASS | Async retrieval task has explicit lifecycle: spawned in pre_reply, cancelled/polled in pre_reasoning |
| §11 | 分层与依赖方向 | ✅ PASS | `agent_scope_memory` depends only on `agent_scope_message` + `agent_scope_model` (core abstractions) |
| §12 | 稳定数据协议 | ✅ PASS | `MemoryType` enum uses `#[serde(untagged)]` with `Unknown(String)` variant |
| §13 | 稳定错误模型 | ✅ PASS | Typed error enum distinguishing I/O, parse, validation, and retrieval errors |
| §14 | 可观测性 | ✅ PASS | All write/delete/retrieve operations emit tracing spans with memory name, type, timing |
| §15 | 性能不牺牲正确性 | ✅ PASS | Index consistency guaranteed; no speculative caching that bypasses disk state |
| §16 | 小步交付 | ✅ PASS | Standalone capability: trait → FileMemory → index → retrieval → middleware |
| §17 | 完成定义 | ✅ PASS | Checklist items will be met for each phase deliverable |
| §18 | 兼容性分级 | ✅ PASS | Target L2 (核心行为兼容) for memory storage and retrieval |
| §19 | 变更治理 | ✅ PASS | No constitution violations |

**All 19 principles pass. No violations to justify.**

## Project Structure

### Documentation (this feature)

```text
specs/009-memory-system/
├── plan.md              # This file
├── research.md          # Phase 0: design decisions
├── data-model.md        # Phase 1: entities and relationships
├── quickstart.md        # Phase 1: validation scenarios
├── contracts/           # Phase 1: interface contracts
│   ├── memory-trait.md
│   ├── backend-trait.md
│   └── middleware-contract.md
└── tasks.md             # Phase 2: /speckit-tasks output
```

### Source Code (repository root)

```text
crates/agent_scope_memory/      # NEW crate
├── Cargo.toml
├── src/
│   ├── lib.rs                  # crate root, re-exports
│   ├── memory_trait.rs         # Memory trait definition
│   ├── memory_entry.rs         # MemoryEntry, MemoryMetadata, MemoryType
│   ├── file_memory.rs          # FileMemory implementation
│   ├── memory_config.rs        # MemoryConfig
│   ├── backend.rs              # Backend trait + LocalBackend
│   ├── index.rs                # MEMORY.md index read/write/truncate
│   ├── frontmatter.rs          # YAML frontmatter parser
│   ├── retrieval.rs            # Relevance-based retrieval logic
│   └── memory_error.rs         # Typed error enum
└── tests/
    ├── memory_trait_tests.rs   # Trait contract tests
    ├── file_memory_tests.rs    # FileMemory integration tests
    ├── index_tests.rs          # Index truncation tests
    └── retrieval_tests.rs      # Retrieval tests (mock model)

crates/agent_scope_agent/src/   # MODIFIED
├── middleware.rs               # ADD: on_system_prompt hook
└── memory_middleware.rs        # NEW: MemoryMiddleware
```

**Structure Decision**: New `agent_scope_memory` crate follows the project pattern (one capability = one crate). MemoryMiddleware stays in `agent_scope_agent` because it depends on both `Memory` trait and `Middleware` trait, and the crate already hosts middleware implementations. This avoids introducing a 4th crate just for middleware.

## Complexity Tracking

> No constitution violations to justify. All checks passed.

