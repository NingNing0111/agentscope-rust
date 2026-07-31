# Implementation Plan: End Event Content

**Branch**: `014-end-event-content` | **Date**: 2026-07-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/014-end-event-content/spec.md`

## Summary

扩展现有 Agent event 协议，让文本、thinking、工具调用和工具结果的 EndEvent 在保持生命周期语义不变的前提下，携带对应 block 从 Start 到 End 期间累积得到的完整内容快照。实现方式是在 `agent_scope_event` 的现有 EndEvent 数据结构上新增可选字段，并在 `agent_scope_agent` 的流式 `BlockTracker` 与非流式事件生产路径中填充这些字段；DeltaEvent 继续按原顺序发布，EndEvent 内容只作为便利快照和 trace 增强。

## Technical Context

**Language/Version**: Rust 2024 edition (workspace package edition)

**Primary Dependencies**: `agent_scope_event` (event protocol structs), `agent_scope_agent` (event production paths), `agent_scope_model` (stream accumulation reference), `agent_scope_message` (content/tool result data), `serde`/`serde_json`, `tokio`, `futures`

**Storage**: N/A — 仅扩展内存中的事件结构、序列化协议和 transient block 累积状态

**Testing**: `cargo test`; targeted crate tests for `agent_scope_event` and `agent_scope_agent`; serialization round-trip tests; scripted/mock stream tests for event order and accumulated content

**Target Platform**: macOS/Linux cross-platform library usage

**Project Type**: Rust workspace library crates (`agent_scope_event` + `agent_scope_agent` protocol/behavior extension)

**Performance Goals**: EndEvent 内容填充为 O(total delta bytes) 且不引入额外模型/tool I/O；事件序列数量与原先保持一致；新增累积状态只保存每个 active block 的当前内容片段

**Constraints**: 不改变现有 Start/Delta/End 事件发布顺序；不删除 DeltaEvent；不改变 ReplyEnd/error/cancellation 语义；新增字段必须向后兼容旧 JSON；不得引入新 crate 或公共事件类型分叉

**Scale/Scope**: 4 个 EndEvent 结构新增可选字段；2-3 条流式 close helper 路径填充内容；非流式文本/thinking/tool call/tool result 路径填充内容；覆盖协议层与 agent 事件生产层测试

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | Article | Status | Notes |
|---|---------|--------|-------|
| 1 | 兼容性优先 | ✅ PASS | 保持事件类型、顺序、数量和生命周期语义不变，仅增加可选字段；需与 Python/既有 trace 的可观察事件顺序对齐 |
| 2 | 锁定上游版本 | ✅ PASS | 基于当前仓库锁定的 AgentScope 兼容目标，不改变上游基线 |
| 3 | Python 是行为基准 | ✅ PASS | Python/既有行为中 End 表示生命周期结束；本 feature 是 Rust 侧明确协议增强，必须记录兼容性偏差/扩展 |
| 4 | 先定义契约 | ✅ PASS | `spec.md` 已定义用户场景、事件协议、边界情况和验收条件；本 plan 生成 contracts |
| 5 | 不允许伪兼容 | ✅ PASS | 内容不可用时使用 None/缺失字段表达未知，不用空字符串伪装完整内容 |
| 6 | 测试驱动兼容性 | ✅ PASS | 规划序列化、流式、非流式、取消/错误、交错 block 测试 |
| 7 | Trace 是核心验收 | ✅ PASS | EndEvent 新字段纳入 trace，可验证与 Delta 拼接结果一致 |
| 8 | Rust 原生设计 | ✅ PASS | 使用 Option 字段与局部 accumulator 状态，不模拟 Python 动态结构 |
| 9 | 安全 Rust 优先 | ✅ PASS | 不需要 unsafe；生产代码避免 unwrap/expect |
| 10 | 结构化并发 | ✅ PASS | 不新增 spawn/channel；沿用现有有界事件发送与 cancellation 路径 |
| 11 | 分层与依赖方向 | ✅ PASS | event crate 只定义协议；agent crate 填充内容；不让 core/event 反向依赖 agent |
| 12 | 稳定数据协议 | ✅ PASS | 新增字段使用 serde default + skip none，旧 JSON 缺失字段可反序列化 |
| 13 | 稳定错误模型 | ✅ PASS | 不新增错误字符串判断；错误/取消状态保持既有 typed state/ReplyEnd 语义 |
| 14 | 可观测性 | ✅ PASS | Trace 可直接展示 EndEvent 完整内容快照 |
| 15 | 性能不能牺牲正确性 | ✅ PASS | 不为便利字段改变事件顺序、取消检查点或错误传播 |
| 16 | 小步交付 | ✅ PASS | 独立事件协议增强，不混入 Sandbox/Multi-agent/Distributed runtime |
| 17 | 完成的定义 | ✅ PASS | 后续 tasks 需包含 tests、clippy、fmt、文档和兼容性说明 |
| 18 | 兼容性分级 | ✅ PASS | 目标 L1 协议兼容 + L2 核心行为兼容；新增字段作为向后兼容协议扩展 |
| 19 | 变更治理 | ✅ PASS | 无宪法违规；无需 Complexity Tracking |

**Gate result**: ALL 19 PASS — 无违规，无需 Complexity Tracking。

## Project Structure

### Documentation (this feature)

```text
specs/014-end-event-content/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── event-protocol.md
├── checklists/
│   └── requirements.md
├── spec.md              # Feature spec (input)
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/agent_scope_event/
├── src/
│   ├── block_events.rs      # +text/thinking optional fields on EndEvent structs
│   └── tool_events.rs       # +input/output optional fields on Tool*EndEvent structs
└── tests/
    └── event_serde_tests.rs # +missing/new/empty field round-trip coverage

crates/agent_scope_agent/
├── src/
│   ├── streaming_reactor.rs # fill EndEvent content in streaming + complete/tool paths
│   └── react_loop.rs        # fill EndEvent content in non-streaming loop path
└── tests/
    └── ...                  # event sequence/content regression tests where existing patterns fit

crates/agent_scope_event/tests/
├── append_event_tests.rs    # update constructors and optional content expectations as needed
└── cross_crate_tests.rs     # update EndEvent construction compatibility as needed
```

**Structure Decision**: 协议字段定义集中在 `agent_scope_event`，所有内容填充值来自 `agent_scope_agent` 的事件生产路径。`agent_scope_model::StreamAccumulator` 可作为拼接语义参考，但不承担 EndEvent 生产责任，因为它不拥有事件时序上下文。

## Complexity Tracking

> No violations — this section intentionally left empty.

## Phase 0 Research Summary

Research completed in [research.md](./research.md). Key decisions:

1. 在现有 EndEvent 上新增可选字段，而不是新增 CompleteEvent 或改变 DeltaEvent。
2. EndEvent 内容是便利快照，DeltaEvent 继续发布，不替代流式增量。
3. 流式模型输出在 `BlockTracker` 生命周期状态中累积并在 close helper 中填充。
4. 非流式路径直接从完整 block 或工具输出填充 EndEvent。
5. ToolResultEndEvent 的 output 以消费者已经通过 delta 看到的文本为准。
6. 序列化兼容策略使用 serde default + skip none。
7. 测试以事件协议回归和兼容性为中心。

## Phase 1 Design Summary

Design artifacts generated:

- [data-model.md](./data-model.md): EndEvent 字段、BlockContentAccumulator、事件消费者与状态转换。
- [contracts/event-protocol.md](./contracts/event-protocol.md): 事件 JSON 协议、字段语义、兼容规则、流式/非流式序列契约。
- [quickstart.md](./quickstart.md): 可运行验证场景、测试命令、预期结果。

## Post-Design Constitution Check

| # | Article | Status | Notes |
|---|---------|--------|-------|
| 1 | 兼容性优先 | ✅ PASS | 合同明确新增字段为 optional，并保留原事件序列 |
| 2 | 锁定上游版本 | ✅ PASS | 未改变兼容基线 |
| 3 | Python 是行为基准 | ✅ PASS | 设计将本变更记录为协议扩展，并要求差分/trace 关注事件顺序不变 |
| 4 | 先定义契约 | ✅ PASS | `contracts/event-protocol.md` 定义数据与序列化契约 |
| 5 | 不允许伪兼容 | ✅ PASS | None 与 Some("") 明确区分，取消/错误路径不伪装完整成功输出 |
| 6 | 测试驱动兼容性 | ✅ PASS | quickstart 覆盖 event serde、agent streaming、non-streaming 和 regression |
| 7 | Trace 是核心验收 | ✅ PASS | SC/contract 要求 trace 可用 EndEvent 重建块级最终输出 |
| 8 | Rust 原生设计 | ✅ PASS | 使用 Option<String>、HashMap lifecycle state 和 typed events |
| 9 | 安全 Rust 优先 | ✅ PASS | 设计不需要 unsafe |
| 10 | 结构化并发 | ✅ PASS | 不新增后台任务；保持 cancellation 路径 |
| 11 | 分层与依赖方向 | ✅ PASS | event 协议层与 agent 生产层边界清晰 |
| 12 | 稳定数据协议 | ✅ PASS | 合同规定旧 JSON 缺失字段兼容 |
| 13 | 稳定错误模型 | ✅ PASS | 错误/取消状态保持现有 typed state |
| 14 | 可观测性 | ✅ PASS | EndEvent 内容纳入 trace |
| 15 | 性能不能牺牲正确性 | ✅ PASS | 不改变事件发布顺序与取消检查点 |
| 16 | 小步交付 | ✅ PASS | 范围限定为事件协议增强 |
| 17 | 完成的定义 | ✅ PASS | quickstart 定义 tests/fmt/clippy 验证路径 |
| 18 | 兼容性分级 | ✅ PASS | 目标 L1 + L2 明确 |
| 19 | 变更治理 | ✅ PASS | 无违规 |

**Post-design gate result**: ALL 19 PASS — 可进入 `/speckit-tasks`。
