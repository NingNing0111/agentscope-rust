# Implementation Plan: SubAgent Collaboration

**Branch**: `020-subagent` | **Date**: 2026-08-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/020-subagent/spec.md`

## Summary

实现 AgentScope Rust 的 SubAgent Collaboration：允许 primary agent 注册 reusable SubAgent templates 和 concrete SubAgents，将 bounded tasks 委派给目标 SubAgent，接收 attributable collaboration result，保留 multi-agent speaker identity，并通过稳定 trace 暴露 invocation、completion、failure、timeout、cancellation、scope denial 等生命周期结果。

技术方案采用 `agent_scope_agent` 中的 in-process collaboration layer，复用现有 `Agent` trait、`Msg`、`AgentEvent`、`AgentState`、middleware、permission、memory、session、workspace、sandbox 等抽象。首期不引入分布式 runtime、远程 worker、durable queue、完整 Python app service 或 provider-specific multi-agent formatter parity；这些模式必须以 `UnsupportedFeature` 或 compatibility matrix deferred 状态诚实暴露。

## Technical Context

**Language/Version**: Rust 2024 edition（workspace `Cargo.toml` 使用 `edition = "2024"`）

**Primary Dependencies**:
- `agent_scope_agent` — `Agent` trait、`ReActAgent`、streaming/cancellation/error/middleware 基础；本 feature 的主要实现位置
- `agent_scope_message` — `Msg` 与 `Role`，其中 `Msg.name` 用作 speaker identity
- `agent_scope_event` — 现有 `AgentEvent` reply/model/tool/session 生命周期事件，SubAgent trace 需关联或补充 delegation boundary
- `agent_scope_state` — `AgentState`、context、permission/tool/middleware contexts
- `agent_scope_tool` / `agent_scope_memory` / `agent_scope_workspace` / `agent_scope_sandbox` — capability scope 与 context sharing policy 的现有能力边界
- `agent_scope_types` — 稳定错误信息、finish reason、共享基础类型
- `tokio` / `tokio-util` — async、cancellation、timeout 相关行为
- `futures` / `async-trait` — trait object 与 stream API
- `serde` / `serde_json` — SubAgent templates、requests、results、trace、metadata 序列化
- `uuid` / `chrono` — correlation IDs 与 timestamps（测试中需可 normalize 或固定）
- `thiserror` — typed SubAgent error model

**Storage**: 默认内存 registry、in-memory delegation trace、现有 AgentState/context；可通过现有 session/memory/workspace 能力记录或持久化 side effects。首期不新增外部 durable queue/storage。

**Testing**: `cargo test`（unit + integration tests），`cargo check --workspace`，`cargo clippy --workspace --all-targets -- -D warnings`，`cargo fmt --check`；SubAgent compatibility tests 使用 deterministic scripted/mock agents，不依赖 live model 输出。

**Target Platform**: Rust library workspace，Linux/macOS 优先；Windows 仅需保持库级数据结构和 deterministic tests 可编译，平台差异能力通过 compatibility/deviation 记录。

**Project Type**: Rust library workspace；主要修改 `crates/agent_scope_agent`，新增 SubAgent collaboration modules 与测试；必要时更新 root re-export、docs/examples、compatibility matrix。

**Performance Goals**:
- 单次 successful in-process delegation 的 framework overhead < 50ms（不含模型/tool 自身耗时）
- 注册与 lookup 100 个 SubAgents 的常规操作 < 10ms
- Delegation trace 追加为 bounded in-memory operation，不随未共享 parent context 无限增长
- 20 个独立 parent tasks 使用不同 SubAgent instances 时无可观察状态泄漏

**Constraints**:
- `#![deny(unsafe_code)]`；不得新增 unsafe
- 不允许伪兼容：distributed/app-service/message-bus/provider-formatter 等 deferred 能力必须显式 unsupported/deferred
- Existing single-agent behavior 在未配置 SubAgents 时必须保持不变
- SubAgent 默认 least-privilege context sharing，不隐式共享完整 parent context、tools、memory、workspace、sandbox 权限
- 所有 terminal outcomes 必须 typed 且可观察：success、failure、timeout、cancellation、permission denied、unsupported feature
- Trace/error 默认不得泄露 API keys、credentials、raw secrets 或不必要的敏感 conversation content
- 并发模式必须尊重现有 `ReActAgent` single reply/stream guard（如 `AlreadyStreaming`）

**Scale/Scope**: 主要 1 个 crate（`agent_scope_agent`）+ root/docs/example/compatibility matrix 更新；预计 1200–2200 LOC production code + 1000–1800 LOC tests；覆盖 4 个 user stories、28 个 FR、8 个 SC。

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Pre-Design Check (Phase 0)

| # | 宪法条款 | 评估 | 备注 |
|---|----------|------|------|
| 1 | 兼容性优先 | ✅ PASS | 基于 Python AgentScope v2.0.5 baseline 中的 SubAgentTemplate/app/multi-agent formatter 语义，无法覆盖项显式 deferred/unsupported |
| 2 | 锁定上游版本 | ✅ PASS | 继承项目既有兼容基线；本 feature 不改变 upstream version |
| 3 | Python 行为基准 | ✅ PASS | research.md 引用 compatibility inventory；quickstart 要求 deterministic trace 与 known deviations |
| 4 | 先定义契约 | ✅ PASS | spec.md 已定义 US/FR/SC；本 plan 生成 data-model/contracts/quickstart |
| 5 | 不允许伪兼容 | ✅ PASS | 明确禁止把 distributed/app-service/message-bus 伪装成本地成功 |
| 6 | 测试驱动兼容性 | ✅ PASS | quickstart 定义 6 类 deterministic core traces，不依赖 live LLM |
| 7 | Trace 是核心验收产物 | ✅ PASS | DelegationTrace/DelegationEvent 是核心验收合同 |
| 8 | Rust 原生设计 | ✅ PASS | 使用 trait object、struct/enum、Result、Arc<dyn Agent>，不机械复制 Python runtime |
| 9 | 安全 Rust 优先 | ✅ PASS | 无 unsafe；typed errors 替代 panic/no-op |
| 10 | 结构化并发 | ✅ PASS | Parent owns SubAgent work；timeout/cancellation/terminal outcome 必须可观察 |
| 11 | 分层与依赖方向 | ✅ PASS | 实现在 agent 层，复用 message/event/state/tool/memory/workspace/sandbox abstractions，不污染 provider/core |
| 12 | 稳定数据协议 | ✅ PASS | data-model 定义 template/request/result/trace 可序列化稳定结构与 metadata 扩展 |
| 13 | 稳定错误模型 | ✅ PASS | SubAgentErrorCategory 覆盖 invalid/missing/timeout/cancel/permission/unsupported 等稳定类别 |
| 14 | 可观测性 | ✅ PASS | Trace redaction rules 明确禁止泄露 secrets/raw sensitive content |
| 15 | 性能不能牺牲正确性 | ✅ PASS | 性能目标不允许改变事件顺序、错误语义或 isolation policy |
| 16 | 小步交付 | ✅ PASS | 聚焦 in-process SubAgent，不包含 Distributed runtime |
| 17 | 完成的定义 | ✅ PASS | quickstart 定义 test/check/clippy/fmt + compatibility matrix gate |
| 18 | 兼容性分级 | ✅ PASS | 目标 L2 lifecycle/trace；可表达处 L3 API semantics；其他 deferred |
| 19 | 变更治理 | ✅ PASS | 当前设计无宪法违反 |

**Gate Result**: ✅ ALL PASS — 无违反，可进入 Phase 0

### Post-Design Check (Phase 1)

| 条款 | 设计决策 | 状态 |
|------|----------|------|
| §I/§III | `SubAgentTemplate`、`DelegationRequest`、`CollaborationResult`、`DelegationTrace` 捕获可观察 SubAgent 行为 | ✅ |
| §V | Distributed/app-service/message-bus/provider formatter 完整兼容均显式 deferred/unsupported，不返回 no-op success | ✅ |
| §VI/§VII | quickstart 要求 6 类 deterministic traces，trace fields 支持 event order、speaker identity、terminal outcome 验证 | ✅ |
| §VIII | 使用 `Arc<dyn Agent>`、struct/enum、typed `Result`，复用现有 `Agent::reply/reply_stream/observe` | ✅ |
| §IX | 新增错误模型必须 typed，不引入 unsafe/panic 驱动控制流 | ✅ |
| §X | Parent owns delegation lifecycle；timeout/cancellation propagation 和 terminal outcome 必须可观察 | ✅ |
| §XI | 主要实现位于 `agent_scope_agent`，不增加 provider/core 反向依赖 | ✅ |
| §XII/§XIII | data-model.md 定义稳定序列化实体与错误类别；contracts 定义 API/trace 稳定行为 | ✅ |
| §XIV | delegation trace 默认 redacted；safe summaries 替代 raw secrets/content | ✅ |
| §XV | 并发和性能不得破坏 `AlreadyStreaming` guard、event ordering 或 capability scope | ✅ |
| §XVI | 不实现 distributed runtime；仅为 roadmap 下一步 multi-agent collaboration 提供 in-process MVP | ✅ |
| §XVIII | plan/spec 明确目标兼容等级和 deferred parity 范围 | ✅ |

**Post-Design Gate Result**: ✅ ALL PASS — 设计无违反宪法

## Project Structure

### Documentation (this feature)

```text
specs/020-subagent/
├── plan.md                         # This file
├── research.md                     # Phase 0 output
├── data-model.md                   # Phase 1 output
├── quickstart.md                   # Phase 1 output
├── contracts/                      # Phase 1 output
│   ├── subagent-api.md             # Public API behavior contract
│   └── delegation-trace.md         # Trace/event ordering/redaction contract
└── tasks.md                        # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/agent_scope_agent/
├── Cargo.toml
├── src/
│   ├── lib.rs                      # Re-export public SubAgent types
│   ├── agent_trait.rs              # Existing Agent trait reused unchanged unless contract requires additive docs
│   ├── agent_error.rs              # Existing AgentError interop / wrapping
│   ├── subagent.rs                 # SubAgent, SubAgentTemplate, registry public surface
│   ├── delegation.rs               # DelegationRequest, CollaborationResult, lifecycle orchestration
│   ├── context_policy.rs           # ContextSharingPolicy, SharedContext, capability scope
│   ├── delegation_trace.rs         # DelegationTrace, DelegationEvent, redaction helpers
│   └── subagent_error.rs           # Typed SubAgentError and stable categories
└── tests/
    ├── subagent_template_tests.rs   # Template validation and registry errors
    ├── subagent_delegation_tests.rs # Successful single delegation
    ├── multi_subagent_tests.rs      # Multiple collaborators and speaker identity
    ├── subagent_error_tests.rs      # failure/timeout/cancellation/unsupported
    ├── subagent_scope_tests.rs      # capability/context policy denial
    └── subagent_trace_tests.rs      # deterministic trace ordering/redaction

crates/agent_scope_message/
└── src/msg.rs                       # No breaking changes; `Msg.name` remains speaker identity

crates/agent_scope_event/
└── src/lib.rs                       # Additive event variants only if needed for delegation boundary

docs/
├── en/modules/agent.md              # Document SubAgent collaboration after implementation
└── zh/modules/agent.md              # 中文文档同步

examples/agent-demo/
└── main.rs                          # Optional follow-up: demonstrate SubAgent once implemented

specs/001-compatibility-baseline/
└── capability-matrix.json           # Record supported/deferred/unsupported SubAgent-related capabilities
```

**Structure Decision**: 将 SubAgent collaboration 作为 `agent_scope_agent` 的 agent-layer 能力实现，并保持 `Msg`、`AgentEvent`、state/tool/memory/workspace/sandbox 作为被复用的边界。这样可以最大限度复用现有单 Agent 运行时，同时避免把 service runtime、provider formatter 或 distributed scheduling 过早引入核心设计。

## Complexity Tracking

> 无违反宪法的情况，不需要填写此表。
