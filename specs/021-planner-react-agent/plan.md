# Implementation Plan: Planner + ReActAgent Compatibility

**Branch**: `021-planner-react-agent` | **Date**: 2026-08-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/021-planner-react-agent/spec.md`

## Summary

实现 AgentScope Rust 的 Planner + ReActAgent compatibility layer：开发者可以提交一个多步骤目标，系统先生成显式 plan，再按 plan step 通过既有 ReActAgent reasoning→acting 能力执行，支持 step 状态追踪、失败后显式 replanning、非流式与流式进度事件、取消/超时/unsupported 终态，以及与 Python AgentScope reference 的 deterministic trace-level 兼容性验证。

技术方案采用 `agent_scope_agent` 内的 additive planner orchestration layer，复用现有 `Agent` trait、`ReActAgent`、`AgentEvent`、`Msg`、`ToolKit`、`AgentState`、middleware、permission、session、memory、workspace、sandbox 与 SubAgent trace/error 模式。首期不引入分布式调度、外部 durable queue、复杂 DAG/并行计划执行或 provider-specific natural-language planner parity；这些能力必须以 compatibility matrix deferred/unsupported 状态诚实暴露。

## Technical Context

**Language/Version**: Rust 2024 edition（workspace `Cargo.toml` 使用 `edition = "2024"`）

**Primary Dependencies**:
- `agent_scope_agent` — `Agent` trait、`ReActAgent`、SubAgent collaboration、streaming/cancellation/error/middleware 基础；本 feature 的主要实现位置
- `agent_scope_model` — planner 与 ReAct step 复用 `ChatModel` 抽象；deterministic tests 使用 scripted/mock model
- `agent_scope_message` — `Msg`、`Role`、content blocks；planned task 与 ReAct loop 的 message context 载体
- `agent_scope_event` — 既有 reply/model/tool/session lifecycle events；新增或复用 planning lifecycle boundary events 必须保持稳定顺序
- `agent_scope_state` — `AgentState`、context、permission/tool/middleware contexts；planned execution 不得破坏已有状态隔离
- `agent_scope_tool` — plan step 中工具执行仍通过现有 Tool/ToolKit/permission lifecycle
- `agent_scope_memory` / `agent_scope_workspace` / `agent_scope_sandbox` — planned tasks 可复用现有 capability boundaries；默认不新增隐式权限
- `agent_scope_types` — typed errors、finish reason、shared stable types
- `tokio` / `tokio-util` — async execution、timeout、cancellation token propagation
- `futures` / `async-trait` / `async-stream` — trait object 与 stream API
- `serde` / `serde_json` / `schemars` — Plan、PlanStep、PlanningTrace、contracts、golden trace serialization
- `uuid` / `chrono` — correlation IDs 与 timestamps；tests 必须 normalize 或固定
- `thiserror` — planner typed error model

**Storage**: 默认 in-memory planned task state、plan revisions 与 planning trace；可通过现有 session/memory/workspace 记录副作用。首期不新增外部 database、durable queue 或 distributed state store。

**Testing**: `cargo test`（unit + integration tests）、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --check`；planner compatibility tests 使用 deterministic scripted/mock models、fixed tools、fixed IDs/clocks 或 trace normalization，不依赖 live LLM natural-language output。

**Target Platform**: Rust library workspace，Linux/macOS 优先；Windows 需保持库级数据结构和 deterministic tests 可编译。平台差异能力通过 compatibility/deviation 记录。

**Project Type**: Rust library workspace；主要修改 `crates/agent_scope_agent`，必要时 additive 修改 `agent_scope_event`、root re-export、docs/examples、compatibility matrix。

**Performance Goals**:
- 生成和验证 20-step 以内 plan 的 framework overhead < 50ms（不含模型调用耗时）
- 单个 plan step orchestration overhead < 25ms（不含模型/tool 自身耗时）
- Planning trace append 为 bounded operation，不随 raw conversation content 无限制复制
- 20 个独立 planned tasks 使用不同 agent instances 时无可观察状态泄漏
- Streaming progress event delivery 保持 backpressure，不使用 unbounded channel

**Constraints**:
- `#![deny(unsafe_code)]`；不得新增 unsafe
- 不允许伪兼容：unsupported Python Planner 能力、distributed runtime、external scheduler、parallel DAG execution 必须显式 unsupported/deferred
- 未启用 planning 的既有 ReActAgent 行为必须保持完全 backward compatible
- Planning 默认不扩大 tool/memory/workspace/sandbox 权限；step execution 仍走既有 permission/capability boundaries
- 所有 terminal outcomes 必须 typed 且可观察：completed、partially_completed、cancelled、failed、unsupported
- Trace/error/event 默认不得泄露 API keys、credentials、raw secrets 或不必要的敏感 conversation content
- 并发模式必须尊重现有 `ReActAgent` reply/stream guard 与 structured cancellation semantics
- 事件顺序、tool call lifecycle、failure propagation 不得为性能优化而改变

**Scale/Scope**: 主要 1 个 crate（`agent_scope_agent`）+ 可选 event/root/docs/example/compatibility matrix 更新；预计 1200–2500 LOC production code + 1200–2200 LOC tests；覆盖 4 个 user stories、20 个 FR、8 个 SC。

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Pre-Design Check (Phase 0)

| # | 宪法条款 | 评估 | 备注 |
|---|----------|------|------|
| 1 | 兼容性优先 | ✅ PASS | 以 Python AgentScope Planner/ReAct 可观察行为为基准；unsupported 能力显式记录 |
| 2 | 锁定上游版本 | ✅ PASS | 继承项目既有 Python AgentScope 兼容基线；本 feature 不改变 upstream version |
| 3 | Python 行为基准 | ✅ PASS | research/quickstart 要求 deterministic trace-level Python vs Rust scenarios |
| 4 | 先定义契约 | ✅ PASS | spec.md 已定义 US/FR/SC；本 plan 生成 contracts/data-model/quickstart |
| 5 | 不允许伪兼容 | ✅ PASS | distributed/parallel DAG/provider natural-language parity 等均 deferred/unsupported |
| 6 | 测试驱动兼容性 | ✅ PASS | 使用 scripted/mock model、fixed tools、trace normalization，不依赖 live LLM |
| 7 | Trace 是核心验收产物 | ✅ PASS | PlanningTrace 与 lifecycle events 是验收核心 |
| 8 | Rust 原生设计 | ✅ PASS | 使用 struct/enum/trait object/Result，不机械复制 Python runtime |
| 9 | 安全 Rust 优先 | ✅ PASS | 无 unsafe；typed errors 替代 panic/no-op |
| 10 | 结构化并发 | ✅ PASS | planned task owner、timeout、cancellation、bounded streams 均明确 |
| 11 | 分层与依赖方向 | ✅ PASS | 主要位于 agent layer，复用现有 abstraction，不引入 provider/core 反向依赖 |
| 12 | 稳定数据协议 | ✅ PASS | Plan/Step/Trace/Outcome 均定义序列化与扩展策略 |
| 13 | 稳定错误模型 | ✅ PASS | PlannerError/PlannerErrorCategory 提供 stable machine-readable categories |
| 14 | 可观测性 | ✅ PASS | Trace redaction 与 structured progress events 纳入 contracts |
| 15 | 性能不能牺牲正确性 | ✅ PASS | 性能目标不允许改变 event order、error semantics 或 capability boundaries |
| 16 | 小步交付 | ✅ PASS | 聚焦 Planner + ReAct integration，不实现 distributed runtime |
| 17 | 完成的定义 | ✅ PASS | quickstart 定义 tests/check/clippy/fmt/docs/examples/matrix gates |
| 18 | 兼容性分级 | ✅ PASS | 目标 L2 core behavior + L3 public API semantics；L4 仅覆盖 documented examples |
| 19 | 变更治理 | ✅ PASS | 当前设计无宪法违反 |

**Gate Result**: ✅ ALL PASS — 无违反，可进入 Phase 0

### Post-Design Check (Phase 1)

| 条款 | 设计决策 | 状态 |
|------|----------|------|
| §1/§3 | `PlanningTrace`、contracts 与 quickstart 均要求 Python vs Rust normalized trace comparison | ✅ |
| §4 | `planner-api.md`、`planning-trace.md`、`data-model.md` 覆盖输入输出、生命周期、状态机、事件、错误、取消 | ✅ |
| §5 | `UnsupportedFeature`/`PlannerErrorCategory::UnsupportedCapability` 用于 deferred Python Planner capabilities | ✅ |
| §6/§7 | quickstart 定义 5+ deterministic scenarios：success、tool step、replan、cancellation、unsupported、regression | ✅ |
| §8 | 数据结构用 `struct/enum`，agent/planner 扩展点用 trait object，内部可与 Python 不同但外部行为对齐 | ✅ |
| §9/§13 | 所有 fallible public APIs 返回 typed planner/agent errors，不引入 unsafe 或 panic-driven control flow | ✅ |
| §10 | Planned task owns execution lifecycle；streaming 使用 bounded delivery/backpressure；cancellation 传播到 planning/replanning/step execution | ✅ |
| §11 | 主要实现位于 `agent_scope_agent`，仅 additive 依赖现有 abstractions，不污染 core/provider | ✅ |
| §12 | data-model 定义 stable IDs、status enum、outcome enum、metadata extension 与 unknown-field strategy | ✅ |
| §14 | contracts 要求 redacted summaries，不默认输出 secrets/raw sensitive content | ✅ |
| §15 | performance goals 不覆盖 correctness gates；event ordering 与 state transitions 是硬约束 | ✅ |
| §16/§18 | 明确不实现 distributed runtime/parallel DAG；compatibility matrix 记录 L2/L3/deferred | ✅ |

**Post-Design Gate Result**: ✅ ALL PASS — 设计无违反宪法

## Project Structure

### Documentation (this feature)

```text
specs/021-planner-react-agent/
├── plan.md                         # This file
├── research.md                     # Phase 0 output
├── data-model.md                   # Phase 1 output
├── quickstart.md                   # Phase 1 output
├── contracts/                      # Phase 1 output
│   ├── planner-api.md              # Public planner API behavior contract
│   └── planning-trace.md           # Trace/event ordering/redaction contract
└── tasks.md                        # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/agent_scope_agent/
├── Cargo.toml
├── src/
│   ├── lib.rs                       # Re-export public Planner types
│   ├── agent_trait.rs               # Existing Agent trait reused; additive docs only if needed
│   ├── react_agent.rs               # Existing ReActAgent remains backward compatible
│   ├── react_loop.rs                # Existing reasoning→acting loop reused by plan steps
│   ├── agent_error.rs               # Existing AgentError interop / wrapping
│   ├── planner.rs                   # Planner, PlannerConfig, planned task entry points
│   ├── plan.rs                      # Plan, PlanStep, PlanRevision, statuses/outcomes
│   ├── planning_trace.rs            # PlanningTrace, PlanningEvent, redaction helpers
│   ├── planner_error.rs             # Typed PlannerError and stable categories
│   └── planner_stream.rs            # Non-streaming/streaming orchestration helpers if separated
└── tests/
    ├── planner_plan_tests.rs         # Plan validation, malformed/empty plans, limits
    ├── planner_execution_tests.rs    # Successful sequential execution and final outcome
    ├── planner_replan_tests.rs       # Recoverable failure and explicit revisions
    ├── planner_stream_tests.rs       # Chronological planning + ReAct lifecycle events
    ├── planner_cancel_tests.rs       # Cancellation during planning/step/replanning
    ├── planner_error_tests.rs        # typed errors, unsupported capability, redaction
    └── planner_regression_tests.rs   # Existing ReActAgent behavior remains unchanged

crates/agent_scope_event/
└── src/lib.rs                        # Additive planning lifecycle events only if existing generic events cannot express them

specs/001-compatibility-baseline/
└── capability-matrix.json            # Planner + ReActAgent supported/deferred/deviation entries

docs/
├── en/modules/agent.md               # Document Planner + ReActAgent usage after implementation
└── zh/modules/agent.md               # 中文文档同步

examples/agent-demo/
└── README.md or main.rs              # Optional follow-up: documented Planner scenario once implemented
```

**Structure Decision**: 将 Planner 作为 `agent_scope_agent` 的 additive orchestration capability，而不是新建独立 runtime crate。原因：Planner 必须复用 ReActAgent、middleware、state、tool permission、streaming/cancellation/error semantics；放在 agent layer 可以保持 dependency direction 合规，并避免在 Feature 021 中引入 distributed runtime 或外部 scheduling/storage。

## Complexity Tracking

> 无违反宪法的情况，不需要填写此表。
