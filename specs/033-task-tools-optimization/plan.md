# Implementation Plan: 任务工具输出质量优化（Task Tools Output Optimization）

**Branch**: `033-task-tools-optimization` | **Date**: 2026-08-17 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/033-task-tools-optimization/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

优化 `agent_scope_agent` 内置任务工具（TaskCreate / TaskList / TaskGet / TaskUpdate）的输出质量。Feature 024 交付时输出文本**逐字对齐** Python AgentScope `9d1026fa`；`plan-react-agent` 实测暴露三类缺陷：① 工具结果文本无换行终止，同一轮多次工具调用（或紧随的模型推理）输出拼接成一串；② TaskUpdate 输出 `Update task (id=1) status.` 不报告实际变更值（同时掩盖 `add_blocked_by` 依赖变更），模型无法核实；③ TaskGet 原样倾倒超长描述，膨胀上下文并与后续推理粘连。

本特性为**已批准的有意偏差**（用户经 `/speckit-specify` 确认：允许打破 Python 逐字对齐）：工具结果文本统一换行终止 + TaskUpdate 报告实际字段值 + TaskGet 截断超长描述；流式展示层对所有完整工具结果防御性补换行；示例渲染微调。工具名、输入 Schema、状态/依赖/错误语义与数据模型**零变更**。

核心设计决策（详见 [research.md](research.md)）：
1. 偏差路径：按宪法第一条例外 + 第十九条治理，用户在 spec 阶段已人工批准；兼容矩阵 `tool-task-*` 条目登记偏差（仿 Feature 029 ResetTools 命名偏差记录）
2. 换行终止：`task_tools::text_chunk` 统一追加尾随 `\n`（任务工具自身文本）；`streaming_reactor` / `react_loop` 对任意完整工具结果若未以 `\n` 结尾则补 `\n`（通用规则，覆盖非任务工具）
3. TaskUpdate 输出改为 `Updated task (id={id}): status=in_progress, add_blocked_by=[4]`，逐字段报实际值
4. TaskGet 对超过 200 字符的 description 截断：前缀 + `… (truncated, {len} chars total)`
5. 旧精确文本断言（`task_tools_tests.rs` 等）迁移到新协议，并新增新行/报值/截断断言

## Technical Context

**Language/Version**: Rust（stable toolchain，workspace edition 2021）

**Primary Dependencies**: 无新增第三方依赖；内部 crate 沿用：`agent_scope_agent`（task_tools、streaming_reactor、react_loop）、`agent_scope_state`（Task/TaskContext，零改动）、`agent_scope_tool`（ToolExecOutput/ToolResultBlock）、`agent_scope_event`（AgentEvent::ToolResultTextDelta）

**Storage**: N/A——任务数据模型与持久化布局不变；改动全部在工具输出文本与流式展示层

**Testing**: `cargo test`（workspace）；`task_tools_tests.rs` 输出断言迁移 + 新增（尾随换行 / 报值格式 / 截断提示）；流式事件断言覆盖通用换行规则；`plan-react-agent` 手工验证

**Target Platform**: 跨平台库（Linux / macOS / Windows）

**Project Type**: library（多 crate Cargo workspace）

**Performance Goals**: 无独立性能目标——任务操作为内存 O(n)；TaskGet 截断反而降低上下文体积（正面收益）

**Constraints**: `#![deny(unsafe_code)]`；库代码禁 unwrap/expect/panic（锁中毒按 crate 既有模式）；无新后台任务（宪法第十条）；无循环依赖（宪法第十一条）；**输出文本不再逐字对齐 Python（已批准偏差，需在兼容矩阵登记，宪法第一/三/十八条）**

**Scale/Scope**: 主改 1 个 crate（`agent_scope_agent`：task_tools.rs 输出协议、streaming_reactor.rs + react_loop.rs 换行补全）；小改 1 个示例（plan-react-agent main.rs 渲染微调）；迁移 `task_tools_tests.rs` / `task_tools_e2e_tests.rs` 输出断言；更新 `capability-matrix.json`（4 条 `tool-task-*` 偏差登记）与 024 契约引用说明；文档（docs/zh/en agent.md）按需提及新输出协议

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 符合性 | 说明 |
|------|--------|------|
| 第一条 兼容性优先 | ✅（含已登记偏差） | 工具名、输入 schema、状态/依赖/错误语义与数据模型保持 Python 对齐；成功输出文本为**已批准偏差**（spec 阶段用户确认），按第一条例外路径在兼容矩阵 `tool-task-*` 条目登记并说明原因 |
| 第二条 锁定上游版本 | ✅ | 兼容基线不变（Python `9d1026fa`）；本特性不升级上游，仅 Rust 侧输出文本优化 |
| 第三条 Python 是行为基准 | ✅（记录偏差） | 输出文本不再逐字对齐；该偏差为显式、已批准、已登记的例外（非凭文档/类型推断的默认行为），契约 `contracts/task-tools-output.md` 逐字定义新协议 |
| 第四条 先契约后实现 | ✅ | spec（033 已批准）→ research → data-model → contracts（输出协议）先行 |
| 第五条 不允许伪兼容 | ✅ | 输出改动为真实质量提升，无 stub/no-op/静默忽略；任务工具能力完整保留 |
| 第六条 测试驱动兼容性 | ✅ | 旧逐字断言迁移为新协议断言，新增三类断言（换行/报值/截断）；其它对齐面（schema、错误、状态机）diff 测试不变 |
| 第七条 Trace 是核心验收产物 | ✅ | 工具调用仍发射完整 ToolResult 事件序列；quickstart 场景经事件流验证换行终止 |
| 第八条 Rust 原生设计 | ✅ | 改动为局部文本与展示层，无新增模拟 Python 的抽象 |
| 第九条 安全 Rust 优先 | ✅ | 无 unsafe；无新 panic 路径 |
| 第十条 结构化并发 | ✅ | 零新 spawn、零新 channel；换行补全为发射点的同步字符串处理 |
| 第十一条 分层与依赖方向 | ✅ | 改动局限 `agent_scope_agent` 内部与示例，无新 crate 依赖边 |
| 第十二条 稳定数据协议 | ✅ | `Task`/`TaskContext` 字段与 serde 布局零变更；输出文本不属于数据协议 |
| 第十三条 稳定错误模型 | ✅ | 错误语义不变（TaskNotFoundError / Task not found / InvalidInput）；错误文本仅追加尾随换行 |
| 第十四条 可观测性 | ✅ | 事件类型与顺序不变；结果文本更可读，利于 trace 检查 |
| 第十五条 性能不牺牲正确性 | ✅ | 无性能优化诉求；TaskGet 截断降低上下文体积 |
| 第十六条 小步交付 | ✅ | 单一能力（任务工具输出优化），前置依赖（006 工具、007 Agent、024 任务工具）均已交付 |
| 第十七条 完成的定义 | ✅ | quickstart.md 场景 5 定义完整验收命令（test/clippy/fmt/兼容矩阵更新） |
| 第十八条 兼容性分级 | ✅（偏差登记） | 目标等级维持 **L2**（核心行为）+ **L3**（公开 API 语义）：工具 schema/语义/错误仍对齐；输出文本偏差在兼容矩阵 `notes` 登记（同 Feature 029 ResetTools 命名偏差做法），不降级 |
| 第十九条 变更治理 | ✅ | 偏差已获用户人工批准（`/speckit-specify` 两问确认）；按流程记录：违反条款（第一条输出文本对齐）→ 原因（可读性与模型可用性）→ 替代方案（保持对齐仅修流式换行，被用户否决）→ 风险（跨语言输出文本不一致）→ 批准记录（本 plan 与兼容矩阵 notes） |

**Gate 结果（Phase 0 前）**: 通过——唯一偏差（成功输出文本）为已批准、已记录、有治理流程支撑的例外，非未论证违规。

**Gate 结果（Phase 1 后复审）**: 通过——设计产物（research/data-model/contracts/quickstart）未引入新的宪法冲突；偏差范围（仅成功输出文本 + 展示层）在 data-model 与 contracts 中显式限定。

## Project Structure

### Documentation (this feature)

```text
specs/033-task-tools-optimization/
├── plan.md                        # This file (/speckit-plan command output)
├── research.md                    # Phase 0 output (/speckit-plan command)
├── data-model.md                  # Phase 1 output (/speckit-plan command)
├── quickstart.md                  # Phase 1 output (/speckit-plan command)
├── contracts/
│   └── task-tools-output.md       # 新输出文本协议（取代 024 契约的输出文本部分）
├── checklists/
│   └── requirements.md            # /speckit-specify 阶段产物
└── tasks.md                       # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/
├── agent_scope_agent/
│   ├── src/
│   │   ├── task_tools.rs              # text_chunk 统一追加尾随 \n；TaskUpdate 报实际值；TaskGet 截断
│   │   ├── streaming_reactor.rs       # emit_tool_result_and_collect / emit_denied_tool_result 完整结果补 \n
│   │   └── react_loop.rs              # 批处理路径工具结果发射点补 \n（与 streaming 一致）
│   └── tests/
│       ├── task_tools_tests.rs        # 输出断言迁移到新协议 + 新增三类断言
│       └── task_tools_e2e_tests.rs    # 若断言输出文本则同步迁移
├── agent_scope_state/                 # 零源码变更（Task/TaskContext 数据模型不变）
examples/
└── plan-react-agent/src/main.rs       # 渲染微调：工具结果/文本增量事件换行分隔（主要由协议解决）
specs/001-compatibility-baseline/
└── capability-matrix.json             # tool-task-create/get/list/update 4 条 notes 登记输出文本偏差
docs/
├── zh/modules/agent.md                # 任务工具章节按需提及新输出协议
└── en/modules/agent.md                # 同上
```

**Structure Decision**: 沿用既有多 crate workspace。输出协议改动集中在 `agent_scope_agent`（任务工具归属层，符合宪法第十一条）；数据模型层零改动（偏差不触及持久化）；兼容矩阵登记是宪法第一/十八条的强制性要求，归 `specs/001-compatibility-baseline/`。

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| 任务工具成功输出文本偏离 Python 逐字对齐（第一条） | 逐字对齐的文本 `Update task (id=1) status.` 不报告实际变更值，导致模型无法核实依赖/状态变更，实测降低规划质量；拼接输出不可读 | 保持对齐、仅在流式层补换行：能修拼接但 TaskUpdate 仍不报值，模型的核实需求未解决；用户已在 spec 阶段否决该替代（选"全面优化输出"） |

治理记录：本偏差于 `/speckit-specify` 阶段经用户人工确认，按宪法第十九条流程记录（原因、替代方案、风险、批准），并在 `capability-matrix.json` 登记。
