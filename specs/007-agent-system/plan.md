# Implementation Plan: Agent System

**Branch**: `007-agent-system` | **Date**: 2026-07-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/007-agent-system/spec.md`

## Summary

Build the `agent_scope_agent` crate — the orchestration layer above all 6 foundation crates (model, tool, state, message, event, types). The crate defines the `Agent` trait (common interface for all agent types), the `ReActAgent` (reasoning→acting loop with tool execution), a `Middleware` trait (8 hook points for extension), context compression, permission checking, and interruption handling. This is Feature 007 in AgentScope Rust, and it enables the first end-to-end agent workflow: receive user input → reason with LLM → optionally execute tools → produce final response.

## Technical Context

**Language/Version**: Rust edition 2024 (stable, matching workspace)

**Primary Dependencies**:
- `agent_scope_model` — `ChatModel` trait, `ChatResponse`, `ModelCallResult`, `StreamAccumulator`
- `agent_scope_tool` — `Tool` trait, `ToolKit`, `ToolError`, `ToolExecOutput`
- `agent_scope_state` — `AgentState`, `ReplyContext`, `PermissionContext`, `ToolContext`
- `agent_scope_message` — `Msg`, `ContentBlock`, `Role`, factory functions
- `agent_scope_event` — all 28 `AgentEvent` variants, `EventBase`
- `agent_scope_types` — `hook` constants (`agent_hooks`, `react_agent_hooks`), `ReplyFinishedReason`
- `tokio` (async runtime), `futures` (Stream trait), `serde` / `serde_json`, `async-trait`

**Storage**: In-memory only for this feature. `AgentState` is serializable for future persistence but no disk/DB storage is in scope.

**Testing**: `cargo test` with `#[tokio::test]`. Mock/scripted models for deterministic testing (per Constitution Article 6). Event trace comparison using `AgentEvent` sequence assertions.

**Target Platform**: Cross-platform Rust (Linux, macOS, Windows). No OS-specific code.

**Project Type**: Library crate (`agent_scope_agent`) within a Cargo workspace.

**Performance Goals**: Agent loop overhead < 10ms per iteration (excluding model API latency). Context compression should complete in a single model call round-trip. No unbounded memory growth — `AgentState::context` bounded by configurable limits.

**Constraints**:
- All public types use `Arc<dyn Trait>` for dynamic dispatch (per Constitution Article 8)
- `#![deny(unsafe_code)]` — no unsafe Rust
- `#[serde(deny_unknown_fields)]` with catch-all variants for stable data protocol (Article 12)
- Every fallible function returns typed errors (Article 13)
- `tokio::spawn` tasks must have explicit owners and cancellation (Article 10)

**Scale/Scope**: Single agent type (ReActAgent). ~15-20 public types. ~40-60 tests. One new crate. Multi-agent orchestrators and distributed runtime are out of scope.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Article | Name | Status | Notes |
|---------|------|--------|-------|
| 1 | 兼容性优先 | ✅ PASS | Agent trait, ReActAgent, middleware dispatch, and event ordering all align with Python AgentScope's `AgentBase` and `ReActAgent` |
| 2 | 锁定上游版本 | ✅ PASS | Compatibility target: AgentScope Python v1.0.0, commit locked in `specs/001-compatibility-baseline/spec.md` |
| 3 | Python AgentScope 是行为基准 | ✅ PASS | Event sequence order (FR-010) directly mirrors Python's event protocol |
| 4 | 先定义契约，再实现代码 | ✅ PASS | Spec defines 4 user stories, 26 functional requirements, entity definitions, success criteria |
| 5 | 不允许伪兼容 | ✅ PASS | `PermissionEngine` placeholder will be replaced with full implementation; no silent ignoring of unsupported features |
| 6 | 测试驱动兼容性 | ✅ PASS | Mock models + scripted models for deterministic testing; event trace comparison as primary verification |
| 7 | Trace 是核心验收产物 | ✅ PASS | SC-002 requires correct sequence of 10+ event types; tests verify complete AgentEvent trace |
| 8 | Rust 原生设计 | ✅ PASS | `trait Agent` (not Python class hierarchy), `Arc<dyn ChatModel>`, `Arc<dyn Tool>`, typed enums for errors |
| 9 | 安全 Rust 优先 | ✅ PASS | `#![deny(unsafe_code)]` on new crate; no unsafe needed |
| 10 | 结构化并发 | ✅ PASS | Agent owns its spawned tasks; explicit CancelToken for interruption; no fire-and-forget spawns |
| 11 | 分层与依赖方向 | ✅ PASS | `agent_scope_agent` depends on 6 foundation crates (all "core" layer); no circular deps; provider-specific crates not depended on |
| 12 | 稳定的数据协议 | ✅ PASS | `AgentConfig`/`ReActConfig` with `#[serde(default)]` for forward compatibility; `Middleware` hooks are additive |
| 13 | 稳定错误模型 | ✅ PASS | `AgentError` enum with `ValidationError`, `ModelError`, `ToolError`, `TimeoutError`, `CancellationError`, `PermissionDenied` |
| 14 | 可观测性 | ✅ PASS | Structured tracing via `tracing` crate for model calls, tool invocations, hook execution, errors, token usage |
| 15 | 性能不能牺牲正确性 | ✅ PASS | Event ordering preserved; no shortcut for performance; context compression verified to not lose messages |
| 16 | 小步交付 | ✅ PASS | Feature 007 is scoped to single agent + ReActAgent only; multi-agent, distributed, sandbox deferred |
| 17 | 完成的定义 | ✅ PASS | Will follow full checklist: spec approval → plan → tasks → tests → clippy clean → fmt clean → compatibility report |
| 18 | 兼容性分级 | ✅ PASS | Targets L2 (核心行为兼容) — event ordering, ReAct loop, tool lifecycle, middleware hooks; L3 API semantics validated through tests |
| 19 | 变更治理 | ✅ PASS | No constitutional violations anticipated |

**Gate Result**: ALL 19 articles PASS. No violations to justify.

## Project Structure

### Documentation (this feature)

```text
specs/007-agent-system/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── agent-trait.md   # Agent trait contract
│   ├── react-agent.md   # ReActAgent contract
│   └── middleware.md    # Middleware trait contract
└── tasks.md             # Phase 2 output (/speckit-tasks command)
```

### Source Code (repository root)

```text
crates/agent_scope_agent/          # NEW crate
├── Cargo.toml
├── src/
│   ├── lib.rs                     # Crate root, re-exports
│   ├── agent_trait.rs             # Agent trait definition
│   ├── agent_error.rs             # AgentError enum
│   ├── config.rs                  # AgentConfig, ReActConfig, ContextConfig
│   ├── react_agent.rs             # ReActAgent struct + impl
│   ├── react_loop.rs              # Reasoning→Acting loop (private)
│   ├── middleware.rs              # Middleware trait + dispatch
│   ├── context_compression.rs     # Context compression logic
│   ├── permission.rs              # PermissionEngine implementation
│   ├── event_emitter.rs           # Event emission helper
│   └── token_counter.rs           # Token counting for context management
└── tests/
    ├── agent_trait_tests.rs       # Trait contract tests
    ├── react_agent_tests.rs       # ReActAgent integration tests
    ├── middleware_tests.rs        # Hook dispatch tests
    ├── context_compression_tests.rs
    ├── interruption_tests.rs      # Cancellation + resumption tests
    ├── event_sequence_tests.rs    # Event order verification
    └── mocks.rs                   # MockModel, ScriptedModel test utilities
```

**Structure Decision**: Single new crate `crates/agent_scope_agent` following the existing project convention. Each existing crate (`agent_scope_model`, `agent_scope_tool`, etc.) is its own directory under `crates/`. Tests follow the per-crate `tests/` directory pattern established in prior features (see [[test-infrastructure-patterns]]).

## Complexity Tracking

> No constitutional violations. This section is empty.
