# Implementation Plan: AgentScope Foundation Layer

**Branch**: `002-foundation-layer` | **Date**: 2026-07-28 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-foundation-layer/spec.md`

## Summary

实现 AgentScope Rust 框架的 Foundation 层——Message、Event、State、Types 四大核心数据协议模块。这四模块构成整个框架的最底层，按 `types → message → event → state` 拓扑顺序，不依赖上层 Model/Tool/Agent 模块。共 58 条功能需求、10 条成功标准，目标兼容等级为 L1（协议兼容）。实现方式为多 crate workspace 结构——每个 Foundation 模块一个独立 crate（`agent_scope_types`、`agent_scope_message`、`agent_scope_event`、`agent_scope_state`），通过 Cargo.toml 的 `[dependencies]` 显式声明模块间依赖方向，使用 serde 进行 JSON 序列化/反序列化以保证与 Python 参考实现的输出一致性。

## Technical Context

**Language/Version**: Rust stable 1.85+

**Primary Dependencies**: serde (1.x, features: derive), serde_json (1.x), uuid (1.x, v4 + hex encoding), chrono (ISO 8601 timestamps), schemars (optional, JSON Schema generation)

**Storage**: N/A — 本 Feature 仅定义数据结构，不涉及持久化

**Testing**: cargo test (单元测试), cargo clippy, JSON 黄金快照差分测试（Python vs Rust）

**Target Platform**: Linux (主要), macOS (开发), WASM 可选（serde 兼容）

**Project Type**: 多 crate workspace（`agent_scope_types` → `agent_scope_message` → `agent_scope_event` → `agent_scope_state`，每模块一个独立 crate）

**Performance Goals**: 单个 Msg 的 JSON 序列化/反序列化 < 1ms；append_event 单事件处理 < 100μs；无特别吞吐量要求

**Constraints**: Foundation 层 crate 不依赖上层模块（model/tool/agent/workspace/middleware）；序列化输出必须与 Python 参考实现一致（经归一化规则处理后）

**Scale/Scope**: ~58 个功能需求，4 个大模块，~30 个公开类型（struct/enum/trait），~15 个枚举，~27 个事件类型

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | 条款 | 状态 | 说明 |
|---|------|------|------|
| 1 | **兼容性优先** | ✅ PASS | Foundation 层数据结构定义从 Python 源码直接提取，JSON 序列化格式与上游一致 |
| 2 | **锁定上游版本** | ✅ PASS | 兼容目标已在 Feature 001 中锁定（v2.0.4） |
| 3 | **Python 是行为基准** | ✅ PASS | 本层为数据结构定义，序列化行为通过差分测试验证 |
| 4 | **先定义契约** | ✅ PASS | spec.md 通过了 quality checklist，58 条 FR 均有明确验收场景 |
| 5 | **不允许伪兼容** | ✅ PASS | 本层不涉及"暂不支持"的功能，所有定义均有 Python 源码对应 |
| 6 | **测试驱动兼容性** | ✅ PASS | 序列化往返测试、单元测试、黄金快照测试均适用 |
| 7 | **Trace 是核心验收产物** | ✅ PASS | 序列化输出作为差分测试的核心验收产物 |
| 8 | **Rust 原生设计** | ✅ PASS | 使用 enum/trait/Result<T,E>/struct 替代 Python 继承/反射；trait object 用于动态扩展点 |
| 9 | **安全 Rust 优先** | ✅ PASS | 本层为纯数据结构，不需要 unsafe；unwrap 仅用于测试 |
| 10 | **结构化并发** | ✅ N/A | 本层为纯数据结构，无异步任务 |
| 11 | **分层与依赖方向** | ✅ PASS | Foundation 层为最底层，types→message→event→state 拓扑，不依赖上层 |
| 12 | **稳定的数据协议** | ✅ PASS | 使用 `#[serde(other)]` 处理未知枚举变体；未知字段通过 `#[serde(deny_unknown_fields)]` 在反向兼容时选择性放宽 |
| 13 | **稳定错误模型** | ✅ PASS | FR-014 明确使用 `Result<T, ValidationError>`；未来扩展需对齐宪法类型分类 |
| 14 | **可观测性** | ✅ ASSESSED | 数据结构层不产生日志/trace；上层使用时由 Agent/Middleware 层负责 tracing |
| 15 | **性能不能牺牲正确性** | ✅ PASS | 正确性优先；序列化兼容性优先于性能 |
| 16 | **小步交付** | ✅ PASS | Foundation 层作为独立 feature 交付，不涉及 Agent/Model 实现 |
| 17 | **完成的定义** | ⏳ TARGET | 完成时需满足 Done Definition 全部条件 |
| 18 | **兼容性分级** | ✅ PASS | 本层目标为 L1（协议兼容）——数据结构定义、序列化格式兼容 |
| 19 | **变更治理** | ✅ PASS | 无违反宪法条款的设计决策 |

### Gate Result

**ALL GATES PASSED** — 无宪法违规项。可进入 Phase 0 研究阶段。

### Post-Design Re-Evaluation (Phase 1 Complete)

*Re-checked after research.md, data-model.md, contracts/*, quickstart.md completion.*

| # | 条款 | 状态 | 设计验证说明 |
|---|------|------|-------------|
| 1 | **兼容性优先** | ✅ PASS | `#[serde(tag = "type")]` tagged enum 与 Pydantic `Literal["text"]` 序列化输出结构一致；`rename_all = "snake_case"`/`"lowercase"` 精确匹配 Python StrEnum 值 |
| 2 | **锁定上游版本** | ✅ PASS | 已在 Feature 001 锁定 v2.0.4；黄金快照 fixture 路径已规划 |
| 3 | **Python 是行为基准** | ✅ PASS | research.md §10 定义了黄金快照差分测试基础设施；normalization rules 继承自 Feature 001 |
| 4 | **先定义契约** | ✅ PASS | 4 个 API 合约文件（types-api.md, message-api.md, event-api.md, state-api.md）完整定义公开接口、序列化契约、错误类型、依赖边界 |
| 5 | **不允许伪兼容** | ✅ PASS | 无未实现功能；PermissionContext/PermissionRule 明确定义为占位类型，后续由 permission 模块替换 |
| 6 | **测试驱动兼容性** | ✅ PASS | quickstart.md §3-4 定义序列化往返测试、差分测试；contracts 中包含 JSON 样本 |
| 7 | **Trace 是核心验收产物** | ✅ PASS | 序列化 JSON 输出作为核心验收产物；27 种事件类型均有明确的 JSON 序列化契约 |
| 8 | **Rust 原生设计** | ✅ PASS | ContentBlock/AgentEvent 用 `enum`（非继承）；HintContent/ToolOutput 用 untagged enum；`Result<T, ValidationError>` 替代 exception；`#[serde(flatten)]` kwargs 替代 `ConfigDict(extra="allow")` |
| 9 | **安全 Rust 优先** | ✅ PASS | 纯数据结构层，无需 `unsafe`；`unwrap()` 仅限构造时固定值 |
| 10 | **结构化并发** | ✅ N/A | 数据层无异步任务 |
| 11 | **分层与依赖方向** | ✅ PASS | 各 crate Cargo.toml 已验证：types(0)→message(1)→event(2)/state(2) 拓扑正确，无 cycle |
| 12 | **稳定的数据协议** | ✅ PASS | ThinkingBlock 使用 `#[serde(flatten)] extras: HashMap<String, Value>` 处理 provider 透传字段；ContentBlock enum 标记 `#[serde(other)]` 处理未知类型；timestamp/id 归一化规则可处理跨语言差异 |
| 13 | **稳定错误模型** | ✅ PASS | 定义了 `ValidationError`、`AppendEventError`、`AppendContextError`、`TaskError` 四个类型化错误枚举；错误语义可编程区分 |
| 14 | **可观测性** | ✅ ASSESSED | 数据层不产生日志/trace（由上层 Agent/Middleware 负责） |
| 15 | **性能不能牺牲正确性** | ✅ PASS | 正确性优先于性能；JSON 兼容性优先于零拷贝优化 |
| 16 | **小步交付** | ✅ PASS | Foundation 层作为独立 Feature，仅包含 4 个 crate；tasks.md 中按 User Story 分组，支持增量交付验证 |
| 17 | **完成的定义** | ⏳ TARGET | 完成后需满足全部 14 条 Done Definition 条件 |
| 18 | **兼容性分级** | ✅ PASS | 本层目标 L1（协议兼容）— contracts 中 JSON 序列化样本已对齐 |
| 19 | **变更治理** | ✅ PASS | 无违规项 |

**Post-Design Gate Result: ALL GATES PASSED** ✅

设计阶段未发现任何宪法违规项。所有技术决策（serde tagged enum、`#[serde(flatten)]` extras、LRU 缓存、base64 拼接、黄金快照测试）均有 research.md 中的替代方案评估和理由说明。

## Project Structure

### Documentation (this feature)

```text
specs/002-foundation-layer/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── message-api.md   # Message 模块公共 API 契约
│   ├── event-api.md     # Event 模块公共 API 契约
│   ├── state-api.md     # State 模块公共 API 契约
│   └── types-api.md     # Types 模块公共 API 契约
├── spec.md              # Feature specification
├── checklists/
│   └── requirements.md  # Spec quality checklist
└── tasks.md             # Phase 2 output (NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
# Workspace root
Cargo.toml                # [workspace] — members = ["crates/*"]

crates/
├── agent_scope_types/          # types crate — 无 agentscope 内部依赖
│   ├── Cargo.toml              # [package] name = "agent_scope_types"
│   │                           #   [dependencies]: serde, serde_json, uuid, chrono
│   └── src/
│       ├── lib.rs              # pub mod reply; pub mod error; pub mod hook;
│       ├── reply.rs            # ReplyFinishedReason
│       ├── error.rs            # ErrorType, ErrorInfo
│       ├── json.rs             # JSON 类型别名
│       └── hook.rs             # AgentHookTypes, ReActAgentHookTypes
│
├── agent_scope_message/        # message crate — 依赖 agent_scope_types
│   ├── Cargo.toml              # [dependencies] agent_scope_types = { path = "../agent_scope_types" }
│   └── src/
│       ├── lib.rs              # pub mod block; pub mod msg; pub mod factory;
│       ├── msg.rs              # Msg, Usage, Role
│       ├── block.rs            # ContentBlock enum, TextBlock, ThinkingBlock, HintBlock, DataBlock
│       ├── source.rs           # Base64Source, URLSource
│       ├── state.rs            # ToolCallState, ToolResultState
│       └── factory.rs          # user_msg(), assistant_msg(), system_msg()
│
├── agent_scope_event/          # event crate — 依赖 agent_scope_message, agent_scope_types
│   ├── Cargo.toml              # [dependencies] agent_scope_message = { path = "../agent_scope_message" }
│   │                           #                 agent_scope_types = { path = "../agent_scope_types" }
│   └── src/
│       ├── lib.rs              # pub mod base; pub mod event_type; pub mod events;
│       ├── base.rs             # EventBase
│       ├── event_type.rs       # EventType enum (27 variants)
│       ├── reply_events.rs     # ReplyStartEvent, ReplyEndEvent
│       ├── model_events.rs     # ModelCallStartEvent, ModelCallEndEvent
│       ├── block_events.rs     # Text/Data/Thinking/Hint block events
│       ├── tool_events.rs      # Tool call/result events
│       ├── control_events.rs   # User confirm/interrupt/external execution events
│       └── custom.rs           # CustomEvent, AgentEvent enum
│
├── agent_scope_state/          # state crate — 依赖 agent_scope_message, agent_scope_types
│   ├── Cargo.toml              # [dependencies] agent_scope_message = { path = "../agent_scope_message" }
│   │                           #                 agent_scope_types = { path = "../agent_scope_types" }
│   └── src/
│       ├── lib.rs              # pub mod agent_state; pub mod tool_context; pub mod task; pub mod permission;
│       ├── agent_state.rs      # AgentState, ReplyContext
│       ├── tool_context.rs     # ToolContext, ReadCacheEntry
│       ├── task.rs             # Task, TaskContext
│       └── permission.rs       # PermissionContext, PermissionRule 占位类型
│
└── agent_scope_utils/          # utils crate（内部工具，不公开）
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        └── id.rs               # generate_id(), generate_timestamp()

tests/
├── types/
│   ├── reply_tests.rs
│   ├── error_tests.rs
│   └── hook_tests.rs
├── message/
│   ├── msg_tests.rs
│   ├── block_tests.rs
│   ├── factory_tests.rs
│   └── append_event_tests.rs
├── event/
│   ├── event_type_tests.rs
│   └── event_serde_tests.rs
├── state/
│   ├── agent_state_tests.rs
│   ├── task_tests.rs
│   └── migration_tests.rs
└── compatibility/
    ├── fixtures/               # Python 黄金快照 JSON 文件
    └── diff_tests.rs           # Rust vs Python 差分测试
```

**Structure Decision**: 采用多 crate workspace 结构（与宪法第十一条"分层与依赖方向"一致）。每个 Foundation 层模块为独立 crate，通过 Cargo.toml 显式声明依赖方向：

```
agent_scope_types  ← (0 deps)
     ↑
agent_scope_message  ← (types のみ)
     ↑
agent_scope_event  ← (message + types)
     ↑
agent_scope_state  ← (message + types)
```

**多 crate 优势**:
- **编译隔离**: 修改 event crate 不会触发 types/message 的重新编译
- **依赖强制**: Cargo.toml 显式阻止非法依赖方向（如 event 依赖 state），在编译期即可检测
- **独立版本**: 各 crate 可独立发版（如 agent_scope_types 稳定后可 1.0 发布而不影响其他）
- **下游可选**: 外部用户可按需引入（仅需 types 时无需编译 event/state）
- **宪法合规**: 直接体现第十一条的分层与依赖方向要求

测试文件保持在顶层 `tests/` 目录（integration tests），可跨 crate 测试序列化兼容性和差分测试。各 crate 内部使用 `#[cfg(test)] mod tests` 维护单元测试。

## Complexity Tracking

> 无违反宪法条款的设计决策，此节留空。
