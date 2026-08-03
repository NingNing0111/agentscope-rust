---

description: "Task list for Feature 024 implementation"
---

# Tasks: Agent 任务规划重构（内置任务规划工具）

**Input**: Design documents from `/specs/024-agent-task-planning/`

**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅, quickstart.md ✅

**Tests**: 本特性包含测试任务（宪法第六条要求：Scripted/Mock Model 为核心测试手段；契约文本逐字断言）。测试任务先于实现任务，初始运行为 RED（编译失败即预期失败状态），实现后转 GREEN。

**Organization**: 按用户故事分组，每个故事可独立实现与验证。

**上游基准**: Python AgentScope `9d1026fa`（本地 `agentscope/` checkout）。工具描述与输出文本逐字摘录自 `agentscope/src/agentscope/tool/_task/`。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行（不同文件，无未完成依赖）
- **[Story]**: 用户故事标签（US1/US2/US3，对应 spec.md 优先级 P1/P2/P3）

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 基线确认与上游契约素材准备

- [X] T001 确认基线全绿：运行 `rtk cargo test --workspace`（既有 706+ 测试通过），并依据 plan.md "Project Structure" 核对 planner 移除清单（5 个源码文件 + 11 个测试文件）
- [X] T002 [P] 从 `agentscope/src/agentscope/tool/_task/` 摘录 4 个工具的 description 全文与输出文本模板（`_create_task.py`、`_list_task.py`、`_get_task.py`、`_update_task.py`），作为 `contracts/task-tools.md` 的实现素材备查

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 所有用户故事共享的基础设施——任务数据模型扩展、共享状态句柄、配置开关

**⚠️ CRITICAL**: 本阶段完成前不得开始任何用户故事

- [X] T003 扩展 `crates/agent_scope_state/src/task.rs`：为 `TaskContext` 新增 `next_sequential_id()`（现存数值 id 最大值 +1，忽略非数值 id，空集合返回 `"1"`）、`delete_task(id) -> bool`（移除任务并清理所有 blocks/blocked_by 引用）、`update_block_relation(block_id, blocked_by_id)`（双向去重写入，忽略不存在的 id）；在 `#[cfg(test)] mod tests` 中补充三个方法的单元测试（含空集合、非数值 id、悬空引用、删除清理用例）
- [X] T004 [P] 将 `crates/agent_scope_agent/src/react_agent.rs` 的 `AgentInner.state` 从 `RwLock<AgentState>` 改为 `Arc<RwLock<AgentState>>`，并修复所有访问点（`react_agent.rs`、`react_loop.rs`、`streaming_reactor.rs` 及引用 `inner.state` 的其他位置），保证既有行为与测试不变
- [X] T005 [P] 在 `crates/agent_scope_agent/src/config.rs` 为 `AgentConfig` 新增 `task_tools_enabled: bool`（默认 `true`）字段与 `AgentConfigBuilder::task_tools_enabled(bool)` 方法；补充 builder 默认值与显式禁用的单元测试

**Checkpoint**: Foundation ready——数据模型方法就绪、状态可共享、开关可用，用户故事实现可以开始

---

## Phase 3: User Story 1 - Agent 通过内置任务工具自主规划复杂任务 (Priority: P1) 🎯 MVP

**Goal**: 默认配置的 ReActAgent 自动注册 TaskCreate/TaskList/TaskGet/TaskUpdate 四个内置工具，模型可在 ReAct 循环中创建任务清单、管理依赖、推进状态，输出文本与 Python 逐字一致

**Independent Test**: `rtk cargo test -p agent_scope_agent task_tools` 全过（quickstart 场景 1-3）；默认构造的 agent toolkit schema 含 4 个任务工具

### Tests for User Story 1 ⚠️

> **NOTE: 先写测试，确认 RED（编译失败/断言失败）后再实现**

- [X] T006 [P] [US1] 编写工具单元契约测试 `crates/agent_scope_agent/tests/task_tools_tests.rs`：覆盖 4 个工具的 name/description/input_schema 契约、TaskCreate 顺序 id 分配、TaskList 空/非空输出格式、TaskGet 全字段/部分字段输出、TaskUpdate 各字段更新与 `Update task (id=X) ...` 输出、completed 追加提示、deleted 输出与引用清理、TaskNotFoundError/Task not found 错误输出、add_blocks/add_blocked_by 双向同步与无效引用忽略、metadata 合并与 null 删除、空 subject 不更新——输出文本逐字对齐 `contracts/task-tools.md`
- [X] T007 [P] [US1] 编写 Scripted Model 端到端测试 `crates/agent_scope_agent/tests/task_tools_e2e_tests.rs`：覆盖 quickstart 场景 1（TaskCreate×2 → TaskUpdate 建依赖 → TaskList → 状态推进 → 最终答复，验证事件管线 ToolCallStart/End + ToolResult 序列与 `try_state().tasks_context` 终态）、场景 2（不存在 id 更新/查询 + 非法 status 值，循环不中断）、场景 3（删除中间任务，依赖引用清理）

### Implementation for User Story 1

- [X] T008 [US1] 实现 `crates/agent_scope_agent/src/task_tools.rs`：定义共享状态类型（`Arc<RwLock<AgentState>>`）与 `TaskCreate`、`TaskList` 两个工具（含逐字摘录的 description、输入 schema、`Tool::call` 实现、TaskCreate 的顺序 id 赋值逻辑）；工具错误以 `ToolResultState::Error` 的工具结果返回
- [X] T009 [US1] 在 `crates/agent_scope_agent/src/task_tools.rs` 继续实现 `TaskGet` 与 `TaskUpdate`（字段处理顺序严格对齐契约：subject → description → add_blocks → add_blocked_by → status → owner → metadata；deleted 立即移除并清理引用；TaskUpdateStatusInput 输入枚举含 deleted 变体但不进入 TaskState）
- [X] T010 [US1] 在 `crates/agent_scope_agent/src/react_agent.rs` 的 `ReActAgent::new` 中实现构造期注册：`task_tools_enabled` 为 true 时，用共享 `Arc<RwLock<AgentState>>` 构造 4 个任务工具并注册进 toolkit（`None` 时新建默认 ToolKit）；在 `crates/agent_scope_agent/src/lib.rs` 导出 `task_tools` 模块与 4 个工具类型
- [X] T011 [P] [US1] 在 `crates/agent_scope_agent/src/permission.rs` 的 `PermissionEngine::check_decision` 规则评估前新增内置放行名单（`TaskCreate`/`TaskList`/`TaskGet`/`TaskUpdate`），命中返回 `PermissionDecision::Allow`（message 为 `"{tool_name} is always allowed to be called."`）；补充放行名单单测（含 Ask 模式下不触发审批）

**Checkpoint**: US1 完成——`rtk cargo test -p agent_scope_agent task_tools` 全绿；默认 agent 可端到端完成任务清单规划（MVP 可交付）

---

## Phase 4: User Story 2 - 任务状态随会话持久化并在压缩后保持感知 (Priority: P2)

**Goal**: 任务集合随 AgentState 会话保存/加载完整往返；未完成任务的工具痕迹被压缩移除时，向对话上下文注入 HintBlock 任务提醒；已感知或无未完成任务时不注入

**Independent Test**: `rtk cargo test -p agent_scope_agent task_reminder` 全过 + state 序列化测试全过（quickstart 场景 4-6）

### Tests for User Story 2 ⚠️

> **NOTE: 先写测试，确认 RED 后再实现**

- [X] T012 [P] [US2] 编写提醒注入测试 `crates/agent_scope_agent/tests/task_reminder_tests.rs`：覆盖 quickstart 场景 5（无任务工具痕迹 + 未完成任务 → 注入含 `<tasks>You have N in-progress tasks and M pending tasks...` 的 HintBlock，source 为 `{"label": "System", "sublabel": "Runtime State"}`，宿主消息 role 为 assistant；提醒已存在 → 不重复注入；上下文含任务工具调用 → 不注入；全部任务完成 → 不注入）与场景 6 禁用部分（`task_tools_enabled(false)` → 不注入）；覆盖 batch（react_loop）与 streaming（streaming_reactor）两条路径
- [X] T013 [P] [US2] 扩展 `crates/agent_scope_state/src/agent_state.rs` 的序列化测试：构造含任务（含 owner、metadata、双向 blocks/blocked_by 引用、顺序 id）的 AgentState，验证 JSON 保存/加载往返后任务全字段 100% 保留（quickstart 场景 4，SC-002）

### Implementation for User Story 2

- [X] T014 [US2] 实现 `crates/agent_scope_agent/src/task_reminder.rs`：`maybe_inject_task_reminder` 逻辑（契约见 `contracts/task-reminder.md`）——统计 pending/in_progress 数量、反向扫描 assistant 消息检测任务工具调用痕迹与先前提醒（source 匹配 + `<tasks>` 文本）、单写锁临界区内完成评估与 HintBlock 追加；定义任务工具名常量与来源标识常量
- [X] T015 [P] [US2] 在 `crates/agent_scope_agent/src/react_loop.rs` 的每轮推理迭代开始前接入 `maybe_inject_task_reminder`（batch 路径，仅在 `task_tools_enabled` 时调用）
- [X] T016 [P] [US2] 在 `crates/agent_scope_agent/src/streaming_reactor.rs` 的每轮推理迭代开始前接入 `maybe_inject_task_reminder`（streaming 路径，行为与 batch 一致）

**Checkpoint**: US2 完成——场景 4-6 全绿；压缩后任务感知恢复，持久化往返无损

---

## Phase 5: User Story 3 - 移除独立规划器，统一规划能力入口 (Priority: P3)

**Goal**: 从公开 API 完整移除 Planner 组件及其配置/错误/事件/追踪类型；既有引用迁移或清理；文档重写为任务工具说明

**Independent Test**: `rtk cargo test --workspace` 全绿（quickstart 场景 7）；`grep -rn "pub use.*[Pp]lanner\|pub mod plan" crates/agent_scope_agent/src/lib.rs` 无结果

### Implementation for User Story 3

- [X] T017 [US3] 删除 `crates/agent_scope_agent/src/` 下的 `plan.rs`、`planner.rs`、`planner_error.rs`、`planner_stream.rs`、`planning_trace.rs`，并清理 `crates/agent_scope_agent/src/lib.rs` 中对应的 `pub mod` 声明与全部 planner 相关 re-export（Plan/PlanStep/PlanStatus/Planner/PlannerConfig/PlannerError/PlanningTrace 等），修复 crate 内残留引用直至编译通过
- [X] T018 [US3] 删除 `crates/agent_scope_agent/tests/` 下 11 个 planner 测试文件（`planner_*.rs` 含 `planner_mocks.rs`），并检查修复 `subagent_*_tests.rs` 等其他测试文件中的 planner 引用
- [X] T019 [P] [US3] 清理事件与消息 crate 的 planner fixture：`crates/agent_scope_event/tests/event_serde_tests.rs` 的 `planner.lifecycle` 自定义事件用例改名或删除（保留 AgentEvent::Custom 本身）；`crates/agent_scope_message/tests/append_event_tests.rs` 中 `source: "planner"` 的用例改名为中性来源（该用例仅测 HintBlock source 透传）
- [X] T020 [P] [US3] 重写 `docs/zh/modules/agent.md` 与 `docs/en/modules/agent.md`：移除 planner 章节，新增内置任务工具说明（4 个工具、任务模型、`task_tools_enabled` 配置、任务提醒注入行为），与 `contracts/task-tools.md`、`contracts/task-reminder.md` 保持一致

**Checkpoint**: US3 完成——公开 API 零 planner 类型；全 workspace 测试通过；文档与代码一致

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: 全量质量门与收尾

- [X] T021 [P] 运行 `rtk cargo clippy --workspace --all-targets -- -D warnings` 与 `rtk cargo fmt`，修复全部警告与格式问题
- [X] T022 执行 quickstart.md 全场景验证（场景 1-7），确认 `rtk cargo test --workspace` 全绿、planner 移除彻底、文档无 planner 残留
- [X] T023 [P] 更新兼容性追踪文档（docs/ 下的兼容性矩阵或模块状态记录）：标注 planner 能力移除（破坏性变更）、内置任务工具达到 L2（核心行为兼容）+ L3（公开 API 语义兼容）等级，记录上游基线 commit `9d1026fa`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖，可立即开始
- **Foundational (Phase 2)**: 依赖 Phase 1——**阻断所有用户故事**
- **User Stories (Phase 3-5)**: 均依赖 Foundational 完成
  - US1 → US2 顺序执行（US2 的提醒注入引用 US1 注册的工具名与任务状态）
  - US3 在 US1/US2 之后执行（lib.rs 导出清理与新导出存在同文件依赖）
- **Polish (Phase 6)**: 依赖全部用户故事完成

### User Story Dependencies

- **User Story 1 (P1)**: Foundational 完成后即可开始，无其他故事依赖——**MVP 范围**
- **User Story 2 (P2)**: 依赖 US1（任务工具存在是提醒注入语义的前提）
- **User Story 3 (P3)**: 依赖 US1/US2 完成（先具备等效能力再移除旧组件，spec 故事排序）

### Within Each User Story

- 测试任务先于实现任务（RED → GREEN）
- T003（TaskContext 方法）是 T008/T009 的工具实现前置
- T004（Arc 状态句柄）是 T010（构造期注册）的前置
- T014（task_reminder 模块）是 T015/T016（两条循环接入）的前置
- T017（源码删除）是 T018（测试删除与修复）的前置

### Parallel Opportunities

- Phase 1: T001 与 T002 并行
- Phase 2: T003、T004、T005 全部可并行（不同 crate/文件）
- US1: T006 与 T007 并行（不同测试文件）；T011 与 T008/T009 并行（不同文件）
- US2: T012 与 T013 并行（不同 crate）；T015 与 T016 并行（不同文件，均依赖 T014）
- US3: T019 与 T020 并行（不同文件，均依赖 T017/T018）
- Polish: T021 与 T023 并行

---

## Parallel Example: User Story 1

```bash
# 测试任务并行启动（不同文件）:
Task: "编写工具单元契约测试 crates/agent_scope_agent/tests/task_tools_tests.rs"
Task: "编写 Scripted Model 端到端测试 crates/agent_scope_agent/tests/task_tools_e2e_tests.rs"

# 工具实现与权限放行并行（不同文件）:
Task: "实现 task_tools.rs 的 TaskCreate/TaskList"
Task: "PermissionEngine 内置放行名单（permission.rs）"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. 完成 Phase 1: Setup（基线确认）
2. 完成 Phase 2: Foundational（T003-T005）
3. 完成 Phase 3: User Story 1（T006-T011）
4. **STOP and VALIDATE**: `rtk cargo test -p agent_scope_agent task_tools` 全绿，默认 agent 可自主规划——**MVP 可交付**
5. 后续故事按优先级递增交付

### Incremental Delivery

1. Setup + Foundational → 基础设施就绪
2. + US1 → 内置任务工具可用（MVP）
3. + US2 → 持久化与压缩后感知完备
4. + US3 → Planner 移除，API 表面收敛
5. + Polish → 质量门全过，达到宪法第十七条"完成"定义

---

## Notes

- [P] 任务 = 不同文件、无未完成依赖
- [US1]/[US2]/[US3] 标签对应 spec.md 用户故事（可追溯）
- 每个故事完成后在 Checkpoint 处独立验证
- 工具输出文本必须与 `contracts/task-tools.md` 逐字一致（差分测试基础，宪法第三条）
- 遇到锁操作遵循 crate 既有模式；库代码禁止新增 unwrap/expect/panic（宪法第九条）
- 每个逻辑任务组完成后提交；破坏性变更（US3）独立提交
