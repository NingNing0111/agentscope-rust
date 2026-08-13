# Implementation Plan: 事件驱动 HITL 确认机制与 Python 对齐

**Branch**: `032-hitl-event-driven` | **Date**: 2026-08-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/032-hitl-event-driven/spec.md`

## Summary

Rust 引擎的 human-in-the-loop 确认闭环从"Ask → continue 喂 denied + 宿主重建重放"改造为 Python 式"暂停 → 事件恢复同一 agent"。核心改动：`reply_stream` 接受三类 HITL 事件输入（`UserConfirmResultEvent`/`UserInterruptEvent`/`ExternalExecutionResultEvent`）、Ask 时暂停不喂 denied、按 tool_call_id 精确匹配恢复、多工具并发确认、事件带 `suggested_rules`、拒绝生成 `state=DENIED` tool_result。`examples/human-in-the-loop` 改为暂停-确认-恢复交互。

## Technical Context

**Language/Version**: Rust（workspace edition 2021，工具链见 rust-toolchain.toml）

**Primary Dependencies**: 
- `agent_scope_agent`（核心引擎：`react_loop.rs`、`streaming_reactor.rs`、`react_agent.rs`）
- `agent_scope_event`（事件类型已定义，需消费语义）
- `agent_scope_state`（`AgentState.context` awaiting 判定）
- `agent_scope_message`（`Msg`、`ToolCallBlock`、`ContentBlock`）
- `tokio`、`futures`（异步流）

**Storage**: N/A（会话持久化不涉及；state 内存中）

**Testing**: `cargo test`（Mock Model 驱动黄金快照）、`cargo clippy`、`cargo fmt`

**Target Platform**: 跨平台（darwin/linux/windows），库 + 示例

**Project Type**: 多 crate workspace（library）+ examples

**Performance Goals**: 暂停/恢复路径零额外模型调用（对比现状的"重建重放"减少重复推理）

**Constraints**: 
- 不改事件类型定义（`agent_scope_event` 已定义 `UserConfirmResultEvent` 等）
- **增量 API**：保留 `reply_stream(Some(vec![msg]))` 现有签名（18 处调用点不动），新增 `reply_stream_event(EventInput)` 对齐 Python 事件输入
- 事件顺序对齐 Python 黄金快照（宪法第七条）
- Mock Model 驱动测试（宪法第六条）

**Scale/Scope**: 核心引擎（`agent_scope_agent`）+ 事件消费 + 1 示例改造 + 测试

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 宪法条款 | 检查 | 结论 |
|---------|------|------|
| 第一条（兼容性优先） | Ask 暂停语义、事件载荷、拒绝 DENIED、并发确认均对齐 Python | ✅ 通过 |
| 第二条（锁定上游版本） | 对齐 Python 1.0.21（仓库 `agentscope/` 目录）`get_awaiting_tool_calls` 等行为 | ✅ 通过 |
| 第三条（Python 行为基准） | 从 context 提取 awaiting、事件载荷带 suggested_rules，均以 Python 源码/测试为基准 | ✅ 通过 |
| 第五条（伪兼容禁止） | 无等待时注入事件显式报错，不静默 | ✅ 通过 |
| 第六条（测试驱动） | Mock Model + 黄金快照；不依赖真实 LLM | ✅ 通过 |
| 第七条（Trace 验收） | 比较完整事件序列（含顺序、tool state、DENIED） | ✅ 通过 |
| 第八条（Rust 原生设计） | 用 `enum AgentInput` 表达事件联合，`Arc<RwLock<PermissionEngine>>` 共享可变状态 | ✅ 通过 |
| 第十条（结构化并发） | 暂停=流结束（StreamHandle Drop 清理），恢复=新流，无游离任务 | ✅ 通过 |
| 第十一条（分层） | 引擎改动限 `agent_scope_agent`；`agent_scope_event` 类型不动 | ✅ 通过 |

## Project Structure

### Documentation (this feature)

```text
specs/032-hitl-event-driven/
├── plan.md              # 本文件
├── research.md          # Phase 0 输出（10 项研究结论）
├── data-model.md        # Phase 1 输出（实体/状态机）
├── quickstart.md        # Phase 1 输出（验证指南）
├── contracts/
│   └── hitl-events.md   # Phase 1 输出（接口契约）
└── tasks.md             # Phase 2 输出（/speckit-tasks - NOT created by /speckit-plan）
```

### Source Code (repository root)

```text
crates/agent_scope_agent/src/
├── agent_trait.rs        # 修改: 新增 reply_stream_event(EventInput) 方法（保留 reply_stream）
├── react_agent.rs        # 修改: do_reply_stream_event 处理事件输入、AgentInner 可变权限引擎
├── react_loop.rs         # 修改: batch 路径 Ask 暂停、事件恢复
├── streaming_reactor.rs  # 修改: streaming 路径 Ask 暂停、事件恢复
├── permission.rs         # 修改: (如需要) 权限引擎 add_rule 接口
└── config.rs             # 修改: AgentInner 可变 PermissionEngine

crates/agent_scope_agent/tests/
└── hitl_event_driven_test.rs  # 新增: 黄金快照测试（暂停/恢复/拒绝/并发/外部执行/中断）

examples/human-in-the-loop/
└── src/main.rs           # 修改: 暂停-确认-恢复交互（消费 reply_stream_event）
```

**Structure Decision**: 引擎改动集中在 `agent_scope_agent` crate 的三个循环/入口文件；事件类型复用 `agent_scope_event` 既有定义（不改）；测试按宪法第六条用 Mock Model。示例改造为消费新事件输入。

## Complexity Tracking

> 无宪法违规，无需复杂度说明。
