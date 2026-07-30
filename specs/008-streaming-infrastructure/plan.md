# Implementation Plan: Streaming Infrastructure

**Branch**: `008-streaming-infrastructure` | **Date**: 2026-07-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/008-streaming-infrastructure/spec.md`

## Summary

将 ReActAgent 的流式管道从当前的 "accumulate-then-process" 模式升级为真正的实时流式处理。核心变更：重构 `react_loop.rs` 使其在模型流到达时逐 chunk 处理和转发事件（而非先累积整个响应），支持流式工具调用检测和执行，用 `tokio::sync::mpsc` 替代 `broadcast` channel 以提供可靠反压，并添加并发保护和流取消机制。

## Technical Context

**Language/Version**: Rust 2021 edition, MSRV 1.75+

**Primary Dependencies**: tokio (async runtime, mpsc), futures (Stream trait, StreamExt), async-stream (stream! generator), agent_scope_model (StreamAccumulator, ChatModel), agent_scope_tool (Tool, ToolExecOutput), agent_scope_event (AgentEvent, existing event types)

**Storage**: N/A (no persistent storage changes)

**Testing**: cargo test, with MockModel and ScriptedModel for deterministic streaming scenarios, tokio::test for async tests

**Target Platform**: Linux/macOS server, library crate

**Project Type**: Rust library (single crate: agent_scope_agent)

**Performance Goals**: First event (ReplyStart) <5ms from invocation; text delta latency <10ms from model chunk arrival; cancellation <50ms from stream drop

**Constraints**: Backward compatible — `reply()` API unchanged and 47 existing tests must pass unmodified; bounded channel mode must not drop events

**Scale/Scope**: ~4 source files modified (`react_loop.rs`, `react_agent.rs`, `event_emitter.rs`, `agent_error.rs`), 1-2 new files, ~10 test additions

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Article 1: 兼容性优先

- [x] **PASS**: `reply()` 和 `reply_stream()` 公开 API 签名不变，行为等价。事件类型和顺序不变。
- [x] **PASS**: Message/ContentBlock 结构不变。Tool 生命周期不变。

### Article 4: 先定义契约，再实现代码

- [x] **PASS**: Spec 已批准（spec.md），contracts/ 将在 Phase 1 定义。

### Article 5: 不允许伪兼容

- [x] **PASS**: 所有功能都是真实实现，不使用占位符或静默忽略。

### Article 6: 测试驱动兼容性

- [x] **PASS**: MockModel 已在 test infrastructure 中。每个 US 都有明确的独立测试场景。

### Article 7: Trace 是核心验收产物

- [x] **PASS**: 事件序列在 streaming 和 non-streaming 模式下必须一致（SC-007），FR-003 明确定义事件顺序。

### Article 8: Rust 原生设计

- [x] **PASS**: 使用 `tokio::sync::mpsc` 替代 `broadcast` 以匹配反压语义。使用 `futures::Stream` 和 `async-stream` 宏。

### Article 9: 安全 Rust 优先

- [x] **PASS**: 无 `unsafe` 代码。`unwrap()` 仅在不可失败的路径使用（channel 创建）。

### Article 10: 结构化并发

- [x] **PASS**: Stream drop 触发 cancellation。`StreamHandle` 管理任务生命周期。无 orphan task。

### Article 12: 稳定的数据协议

- [x] **PASS**: `AgentEvent` 类型不变。`ToolResultEndEvent` 序列化格式不变。

### Article 16: 小步交付

- [x] **PASS**: 4 个 US 按 P1→P4 顺序递增交付，每个独立可测。

### Article 17: 完成的定义

- [x] **PASS**: 完成定义已包含测试、clippy、fmt、向后兼容性检查。

**Gate Result**: ALL PASS. No violations.

## Project Structure

### Documentation (this feature)

```text
specs/008-streaming-infrastructure/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/agent_scope_agent/
├── src/
│   ├── react_loop.rs        # MODIFY: progressive model stream processing
│   ├── react_agent.rs       # MODIFY: mpsc-based reply_stream, AlreadyStreaming guard
│   ├── event_emitter.rs     # REPLACE: broadcast → mpsc for backpressure support
│   ├── stream_handle.rs     # NEW: cancellation handle tied to stream lifetime
│   ├── streaming_reactor.rs # NEW: progressive stream processing logic
│   ├── agent_error.rs       # MODIFY: add AlreadyStreaming variant
│   ├── agent_trait.rs       # UNCHANGED
│   ├── config.rs            # UNCHANGED (StreamChannelConfig deferred to agent config)
│   ├── middleware.rs         # UNCHANGED
│   ├── context_compression.rs # UNCHANGED
│   └── lib.rs               # MODIFY: export new types
└── tests/
    ├── streaming_tests.rs   # EXTEND: real-time streaming, backpressure, cancellation tests
    ├── mocks.rs             # EXTEND: streaming tool-call mock model
    └── react_agent_tests.rs # UNCHANGED (backward compat check)
```

**Structure Decision**: 单 crate 修改（agent_scope_agent）。`StreamingReactor` 逻辑从 `react_loop.rs` 提取为独立模块以保持关注点分离。现有 `react_loop.rs` 保留用于 `reply()` 的 accumulate-then-process 路径（向后兼容），`streaming_reactor.rs` 提供新的 progressive stream 路径。`EventEmitter` 重写为 mpsc。

## Complexity Tracking

> No violations to justify. All 22 Constitution checks pass.
