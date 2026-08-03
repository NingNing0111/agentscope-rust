# Implementation Plan: Agent 任务规划重构（内置任务规划工具）

**Branch**: `024-agent-task-planning` | **Date**: 2026-08-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/024-agent-task-planning/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

参考 Python AgentScope（本地 checkout `agentscope/`，commit `9d1026fa`，v2.0.5 基线）重构 Rust 版 Agent 的任务规划能力：**移除独立的 Planner 组件**（Feature 021 产物），**改为在 ReActAgent 构造期自动注册 4 个内置任务工具**（TaskCreate / TaskList / TaskGet / TaskUpdate）。任务工具通过共享 `Arc<RwLock<AgentState>>` 操作 `tasks_context`（数据模型已存在，扩展顺序 id、删除、双向依赖方法）；当未完成任务的任务痕迹被上下文压缩移除时，向对话上下文注入 HintBlock 任务提醒（对齐 Python `_inject_runtime_state` 任务维度）。工具命名、输出文本、错误语义逐字对齐 Python 参考实现。

核心设计决策（详见 [research.md](research.md)）：
1. 任务工具放 `agent_scope_agent` crate，构造期注册，共享 state 句柄，`Tool` trait 零侵入
2. `AgentConfig.task_tools_enabled`（默认 true）单一开关控制工具注册 + 提醒注入
3. 复用 state crate 已有 `Task`/`TaskContext`，仅扩展方法不改字段（serde 布局不变）
4. 提醒注入复用既有 HintBlock，在 batch/streaming 两条循环的每轮迭代前评估
5. PermissionEngine 内置放行名单保证任务工具始终 Allow
6. Planner 完整移除（源码 5 文件 + 测试 11 文件 + 文档章节），不留兼容层

## Technical Context

**Language/Version**: Rust（stable toolchain，workspace edition 2021）

**Primary Dependencies**: tokio、serde/serde_json、async-trait、futures、thiserror、chrono、uuid；内部 crate：`agent_scope_message`（HintBlock）、`agent_scope_state`（AgentState/TaskContext）、`agent_scope_tool`（Tool/ToolKit）、`agent_scope_event`（AgentEvent）、`agent_scope_model`（ChatModel）

**Storage**: 会话状态 JSON 序列化（`agent_scope_state` SessionStore，Feature 010）；任务集合作为 AgentState 字段随会话持久化，无独立存储

**Testing**: `cargo test`（workspace）；Scripted/Mock Model 驱动的 ReAct 循环集成测试；serde 往返测试；契约文本逐字断言（对齐 Python 输出协议）

**Target Platform**: 跨平台库（Linux / macOS / Windows）

**Project Type**: library（多 crate Cargo workspace）

**Performance Goals**: 无独立性能目标——任务操作为内存 Vec 上 O(n)（n = 任务数，典型 <100）；框架开销非瓶颈（LLM I/O bound，宪法第十五条）

**Constraints**: `#![deny(unsafe_code)]`；库代码禁 unwrap/expect/panic（锁中毒按 crate 既有模式处理）；无新后台任务（宪法第十条）；无循环依赖（宪法第十一条）

**Scale/Scope**: 主改 1 个 crate（`agent_scope_agent`：+2 模块，-5 模块）；小改 3 个 crate（`agent_scope_state` TaskContext 扩展、`agent_scope_agent` PermissionEngine 放行名单、`agent_scope_event`/`agent_scope_message` 测试 fixture 清理）；删除 11 个 planner 测试文件；更新 2 个文档文件（zh/en agent.md）

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 符合性 | 说明 |
|------|--------|------|
| 第一条 兼容性优先 | ✅ | 工具名、输入 schema、输出文本、错误语义、提醒注入逻辑逐字对齐 Python `9d1026fa`；契约见 `contracts/` |
| 第二条 锁定上游版本 | ✅ | 基线锁定：commit `9d1026fad17e6a985873c0981bb8d4aeacf98cf9`（v2.0.5-9），记录于 research.md 与本文档 |
| 第三条 Python 是行为基准 | ✅ | 已阅读上游源码（`tool/_task/`、`state/_task.py`、`_agent.py::_inject_runtime_state`、`_config.py`），非凭文档推测；输出文本协议逐字摘录进契约 |
| 第四条 先契约后实现 | ✅ | spec（024 已批准）+ research + data-model + contracts 先行 |
| 第五条 不允许伪兼容 | ✅ | Planner 完全移除而非 stub；无静默忽略（add_blocks 引用不存在 id 的忽略行为是 Python 基准行为本身，已在契约中显式记录） |
| 第六条 测试驱动兼容性 | ✅ | Scripted/Mock Model 为核心测试手段（quickstart 场景 1-6）；不依赖真实 LLM 判定 |
| 第七条 Trace 是核心验收产物 | ✅ | 任务工具复用现有工具事件 trace（ToolCallStart/End、ToolResult 序列）；提醒注入通过后续 model request 上下文可观测；不新增事件类型故无需 trace 规范变更 |
| 第八条 Rust 原生设计 | ✅ | `Arc<RwLock<AgentState>>` 共享所有权、`enum` 表达状态、trait object 工具（`Arc<dyn Tool>` 经 ToolKit）；不模拟 Python 的 `is_state_injected` 动态注入 |
| 第九条 安全 Rust 优先 | ✅ | 无 unsafe；锁操作遵循 crate 既有模式；无新 panic 路径 |
| 第十条 结构化并发 | ✅ | 零新 spawn、零新 channel；任务工具为原子短锁内存操作，中断一致性见 research.md Decision 8 |
| 第十一条 分层与依赖方向 | ✅ | 任务工具置 `agent_scope_agent`（依赖 tool+state+message）；tool crate 不反向依赖；无新 crate 间依赖边 |
| 第十二条 稳定数据协议 | ✅ | `Task`/`TaskContext` 字段零变更，仅新增方法；会话存档向后兼容；`deleted` 不进入持久化枚举 |
| 第十三条 稳定错误模型 | ✅ | 用户级错误走 `ToolResultState::Error`（模型可自愈）；参数错误走既有 `ToolError::InvalidInput`；无字符串匹配判错 |
| 第十四条 可观测性 | ✅ | 复用现有工具调用 span/事件；任务工具参数不含敏感信息 |
| 第十五条 性能不牺牲正确性 | ✅ | 无性能优化诉求；O(n) 内存操作 |
| 第十六条 小步交付 | ✅ | 单一能力（任务规划重构），前置依赖（006 工具、007 Agent、008 流式、010 会话）均已交付 |
| 第十七条 完成的定义 | ✅ | quickstart.md 场景 7 定义完整验收命令（test/clippy/fmt/文档） |
| 第十八条 兼容性分级 | ✅ | 目标等级：**L2**（核心行为兼容：工具生命周期、输出文本、提醒注入）+ **L3**（公开 API 语义兼容：`task_tools_enabled` 配置、TaskContext 公开方法）；L4 不适用（本特性无对应上游示例迁移要求） |
| 第十九条 变更治理 | ✅ | 无宪法违反项 |

**Gate 结果（Phase 0 前）**: 通过，无违规需论证。
**Gate 结果（Phase 1 后复审）**: 通过——设计产物（research/data-model/contracts/quickstart）未引入新的宪法冲突；Task 模型复用与 Planner 移除决策均有 spec Assumptions 与 research 记录支撑。

## Project Structure

### Documentation (this feature)

```text
specs/024-agent-task-planning/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/
│   ├── task-tools.md    # 4 个任务工具的 schema/行为/输出/错误契约
│   └── task-reminder.md # 任务提醒注入契约
├── checklists/
│   └── requirements.md  # /speckit-specify 阶段产物
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/
├── agent_scope_state/
│   └── src/
│       └── task.rs                  # 扩展: next_sequential_id / delete_task / update_block_relation
├── agent_scope_agent/
│   ├── src/
│   │   ├── lib.rs                   # 移除 planner 相关 mod/export；新增 task_tools（pub）、task_reminder（crate 内）
│   │   ├── config.rs                # AgentConfig 新增 task_tools_enabled（默认 true）+ builder 方法
│   │   ├── react_agent.rs           # AgentInner.state → Arc<RwLock<AgentState>>；构造期注册任务工具
│   │   ├── react_loop.rs            # 每轮迭代前调用 task_reminder 注入评估（batch 路径）
│   │   ├── streaming_reactor.rs     # 同上（streaming 路径）
│   │   ├── permission.rs            # PermissionEngine 内置任务工具放行名单
│   │   ├── task_tools.rs            # 新增: TaskCreate/TaskList/TaskGet/TaskUpdate 实现
│   │   ├── task_reminder.rs         # 新增: 任务提醒注入（HintBlock 追加 + 感知检测）
│   │   ├── plan.rs                  # 删除
│   │   ├── planner.rs               # 删除
│   │   ├── planner_error.rs         # 删除
│   │   ├── planner_stream.rs        # 删除
│   │   └── planning_trace.rs        # 删除
│   └── tests/
│       ├── task_tools_tests.rs      # 新增: 工具行为契约测试（quickstart 场景 1-3）
│       ├── task_reminder_tests.rs   # 新增: 提醒注入测试（场景 5-6）
│       └── planner_*.rs             # 删除（11 个文件）
├── agent_scope_event/
│   └── tests/event_serde_tests.rs   # planner.lifecycle fixture 清理/改名
├── agent_scope_message/
│   └── tests/append_event_tests.rs  # source="planner" fixture 改名（无语义耦合）
docs/
├── zh/modules/agent.md              # planner 章节 → 任务工具章节
└── en/modules/agent.md              # 同上
```

**Structure Decision**: 单 workspace 多 crate 布局（既有结构）。任务工具归属 `agent_scope_agent`（唯一同时依赖 tool/state/message 的层，符合宪法第十一条）；数据模型留在 `agent_scope_state`（序列化所有权）；消息模型与事件 crate 零源码变更（仅测试 fixture 清理）。

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

无违规项，本表不适用。
