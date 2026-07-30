# Implementation Plan: Session Management（会话管理）

**Branch**: `010-session-management` | **Date**: 2026-07-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/010-session-management/spec.md`

## Summary

实现 Agent 会话的完整生命周期管理——Session trait、SessionStore 持久化抽象、上下文修剪与中间件上下文集成。Feature 010 在现有 `agent_scope_state` crate 基础上扩展，不在 `agent_scope_event` 中添加 5 个 session 事件类型。核心策略：Session trait 定义生命周期操作，SessionStore trait 抽象持久化后端，TrimStrategy 配置修剪行为。所有设计均为 Rust 原生 trait 模式，Session 数据格式与 Python AgentScope AgentState 保持 L2 兼容。

## Technical Context

**Language/Version**: Rust 1.75+ (workspace edition 2021)

**Primary Dependencies**: tokio (async runtime), serde/serde_json (serialization), async-trait, chrono, uuid, tokio-util (CancellationToken)

**Storage**: InMemorySessionStore (HashMap-based, default for testing); trait abstraction for future backends (file, Redis, database)

**Testing**: cargo test (unit + integration), per-crate tests/ layout, JSON round-trip tests

**Target Platform**: Linux/macOS server (single-process), library crate

**Project Type**: library (embedded in agent applications)

**Performance Goals**: SC-003: save → load round-trip < 100ms for 100 messages; SC-002: 100 concurrent sessions with zero cross-contamination

**Constraints**: Single-process (no distributed sessions in scope); Constitution §10 structured concurrency; Constitution §13 stable error model; Constitution §12 stable data protocol (backward-compatible serde)

**Scale/Scope**: 100 active sessions per process; message history per session: 1000+ messages (with trimming); 2 crates modified (agent_scope_state, agent_scope_event)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | Principle | Status | Notes |
|---|-----------|--------|-------|
| I | 兼容性优先 | ✅ PASS | Session data format (AgentState JSON) remains compatible with Python; Session trait is Rust-native (per Constitution §8) |
| II | 锁定上游版本 | ✅ PASS | Compatible with existing Python AgentState format captured in Features 002-003 |
| III | Python 是行为基准 | ✅ PASS | Python AgentState serialization format is the data compatibility baseline |
| IV | 先定义契约 | ✅ PASS | contracts/ 定义了 Session, SessionStore, TrimStrategy, Events 四个契约 |
| V | 不允许伪兼容 | ✅ PASS | SessionError::Closed explicitly rejects operations on closed sessions; no silent degradation |
| VI | 测试驱动兼容性 | ✅ PASS | quickstart.md defines 6 test scenarios; JSON round-trip tests for all events; InMemorySessionStore for deterministic tests |
| VII | Trace 是核心验收产物 | ✅ PASS | All session state changes emit AgentEvent variants; events are part of the trace |
| VIII | Rust 原生设计 | ✅ PASS | Session/SessionStore are traits (not Python class hierarchy); CancellationToken for structured concurrency; SessionImpl wraps AgentState |
| IX | 安全 Rust 优先 | ✅ PASS | No unsafe code; #![deny(unsafe_code)] already on both crates |
| X | 结构化并发 | ✅ PASS | CancellationToken per session; close() cancels token; Drop cancels as safety net; FR-004 enforces task termination timeout |
| XI | 分层与依赖方向 | ✅ PASS | agent_scope_state depends on message + types (already established); no new dependency edges; agent_scope_event depends on nothing new |
| XII | 稳定的数据协议 | ✅ PASS | AgentState serialization already backward-compatible; version field in serialized output; #[serde(default)] on all new fields |
| XIII | 稳定错误模型 | ✅ PASS | SessionError has 6 typed variants covering all error categories from §13 |
| XIV | 可观测性 | ✅ PASS | 5 new session event types cover all lifecycle transitions; session_id on all traces |
| XV | 性能不能牺牲正确性 | ✅ PASS | Trim is a pure state mutation (no I/O); token counting reused from existing Model trait; tool chain integrity enforced |
| XVI | 小步交付 | ✅ PASS | Feature 010 is a single capability module per §16 roadmap; independently testable |
| XVII | 完成的定义 | ✅ PASS | Will comply — tests, clippy, fmt, documentation, compatibility matrix all required before completion |
| XVIII | 兼容性分级 | ✅ PASS | Target: L2 (核心行为兼容) — data format compatible, API is Rust-native |
| XIX | 变更治理 | ✅ PASS | No constitution violations; Session trait design follows established patterns |

**Gate result**: ALL 19 principles PASS. No violations to justify.

## Project Structure

### Documentation (this feature)

```text
specs/010-session-management/
├── plan.md              # This file
├── spec.md              # Feature specification
├── research.md          # Phase 0: 9 research decisions
├── data-model.md        # Phase 1: 6 entities, state transitions
├── quickstart.md        # Phase 1: 6 validation scenarios
├── contracts/           # Phase 1: 4 interface contracts
│   ├── session-trait.md
│   ├── session-store-trait.md
│   ├── context-trimming.md
│   └── session-events.md
├── checklists/
│   └── requirements.md  # Spec quality checklist (16/16 ✅)
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/
├── agent_scope_state/          # EXTENDED
│   ├── Cargo.toml              # +tokio-util dependency (CancellationToken)
│   ├── src/
│   │   ├── lib.rs              # +pub mod session; +pub mod session_store; +pub mod trim;
│   │   ├── agent_state.rs      # (unchanged — existing AgentState struct)
│   │   ├── permission.rs       # (unchanged)
│   │   ├── task.rs             # (unchanged)
│   │   ├── session.rs          # NEW: Session trait, SessionImpl, SessionStatus, SessionMeta, SessionError
│   │   ├── session_store.rs    # NEW: SessionStore trait, InMemorySessionStore
│   │   └── trim.rs             # NEW: TrimStrategy, trim_context()
│   └── tests/
│       ├── session_tests.rs    # NEW: Session lifecycle + isolation tests
│       ├── session_store_tests.rs  # NEW: Save/load/delete/list tests
│       └── trim_tests.rs       # NEW: Trimming strategy tests
│
└── agent_scope_event/          # EXTENDED
    ├── src/
    │   ├── lib.rs              # +pub mod session_events; +5 AgentEvent variants
    │   ├── event_type.rs       # +5 EventType variants
    │   ├── session_events.rs   # NEW: 5 session event structs
    │   └── ... (unchanged)
    └── tests/
        └── session_events_tests.rs  # NEW: Event serialization/round-trip tests
```

**Structure Decision**: Feature 010 extends 2 existing crates (`agent_scope_state`, `agent_scope_event`) rather than creating a new crate. This follows research decision R1 — Session is tightly coupled with AgentState, and putting them together avoids circular dependencies. Event types follow the existing pattern of per-category event modules.

## Complexity Tracking

> No violations to justify. All 19 Constitution principles pass.
