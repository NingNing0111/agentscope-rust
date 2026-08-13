---

description: "Task list for Feature 032 - event-driven HITL confirmation aligned with Python"
---

# Tasks: 事件驱动 HITL 确认机制与 Python 对齐

**Input**: Design documents from `/specs/032-hitl-event-driven/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: 测试任务是本 feature 的核心（宪法第六条测试驱动），黄金快照测试在实现前先写 FAIL。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Workspace**: `crates/agent_scope_agent/`, `crates/agent_scope_event/`, `examples/human-in-the-loop/`
- 事件类型在 `agent_scope_event`（不改定义，只改消费语义）
- 核心引擎改动在 `agent_scope_agent/src/`
- 黄金快照测试在 `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 建立测试基础设施（Mock Model 驱动黄金快照）与事件输入类型基础

- [ ] T001 新增黄金快照测试文件骨架 `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`，引入 Mock Model 辅助（参考 `crates/agent_scope_agent/tests/support/` 现有 Mock 模式）
- [ ] T002 在 `agent_scope_agent` 定义 `EventInput` 事件联合枚举（`Confirm(UserConfirmResultEvent)` / `Interrupt(UserInterruptEvent)` / `ExternalResult(ExternalExecutionResultEvent)`），放入 `crates/agent_scope_agent/src/event_input.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 引擎 Ask 暂停语义 + 可变权限引擎 + `reply_stream_event` 入口，所有 user story 的基础

**⚠️ CRITICAL**: 无此阶段，任何 user story 都无法工作

- [ ] T003 修改 `crates/agent_scope_agent/src/streaming_reactor.rs`：Ask 分支改为 emit `RequireUserConfirmEvent`（携带 `state=asking` + `suggested_rules`，多工具并发）后**结束当前流**（返回暂停标记），不再 `continue` 喂 denied
- [ ] T004 修改 `crates/agent_scope_agent/src/react_loop.rs`：batch 路径 Ask 分支同样改为 emit 确认事件后结束（对齐 streaming 路径）
- [ ] T005 修改 `crates/agent_scope_agent/src/react_agent.rs`：`AgentInner` 新增 `Arc<RwLock<PermissionEngine>>` 可变权限引擎（替代从 `config.permission_context` 克隆），构造时初始化
- [ ] T006 修改 `crates/agent_scope_agent/src/config.rs`：构造 `AgentInner` 时用传入 `PermissionContext` 构建 `PermissionEngine`
- [ ] T007 修改 `crates/agent_scope_agent/src/agent_trait.rs`：`Agent` trait 新增 `reply_stream_event(&self, input: EventInput)` 方法（保留 `reply_stream` 原签名，18 处调用点不动）
- [ ] T008 修改 `crates/agent_scope_agent/src/react_agent.rs`：实现 `reply_stream_event`，dispatch 事件类型到恢复逻辑（`_reply_impl` 语义等价共享底层）
- [ ] T009 在 `crates/agent_scope_agent/src/react_agent.rs` / `agent_state.rs` 实现 `get_awaiting_tool_calls` 辅助：从 `state.context` 末尾 assistant 消息扫描 `state==asking` / `state==submitted`（且无匹配 tool_result）的 tool_call（对齐 Python `_state.py:312-339`）

**Checkpoint**: Foundation ready - Ask 暂停语义 + 事件入口 + awaiting 判定可用，user story 实现可开始

---

## Phase 3: User Story 1 - 宿主以事件恢复暂停的回复 (Priority: P1) 🎯 MVP

**Goal**: 宿主以 `UserConfirmResultEvent` 恢复暂停的同一 agent，按 tool_call_id 精确匹配执行/拒绝

**Independent Test**: mock 工具返回 `PermissionBehavior::Ask`，宿主确认 true 后工具执行、agent 从暂停点继续

### Tests for User Story 1 (TDD - write FIRST, FAIL) ⚠️

- [ ] T010 [P] [US1] 黄金快照测试：单工具 Ask → `RequireUserConfirmEvent`（state=asking + suggested_rules）→ 流结束（无 denied、无 ReplyEnd）in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`
- [ ] T011 [P] [US1] 黄金快照测试：注入 `UserConfirmResultEvent{confirmed:true}` 恢复 → 工具执行 → tool_result → ReplyEnd(completed)，事件顺序对齐 Python in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`

### Implementation for User Story 1

- [ ] T012 [US1] 实现 `_handle_incoming_event` 的确认分支：按 tool_call_id 匹配 awaiting asking 工具，`confirmed=true` → 执行工具（对齐 Python `_agent.py:1580-1625`）in `crates/agent_scope_agent/src/react_agent.rs`（或新增 `hitl_resume.rs`）
- [ ] T013 [US1] 恢复后从暂停点继续 reasoning-acting 循环，补充 tool_result 到 context

**Checkpoint**: 暂停-确认-恢复全链路可用，单工具场景独立可测

---

## Phase 4: User Story 2 - 拒绝未等待确认的恢复请求 (Priority: P2)

**Goal**: 非法恢复（未等待/ id 不匹配）返回明确错误

**Independent Test**: agent 无等待确认时注入确认事件 → 报错

### Tests for User Story 2 (TDD - write FIRST, FAIL) ⚠️

- [ ] T014 [P] [US2] 黄金快照测试：agent 无 awaiting 时注入 `UserConfirmResultEvent` → 返回明确错误（"not waiting for user confirmation"）in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`
- [ ] T015 [P] [US2] 黄金快照测试：注入 id 与 awaiting 不匹配的确认结果 → 报错指出额外 id in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`
- [ ] T016 [P] [US2] 黄金快照测试：reply_id 不匹配 → 报错 in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`

### Implementation for User Story 2

- [ ] T017 [US2] 实现 `_check_incoming_event` 校验逻辑：`extra_ids = confirm_ids - awaiting_ids`，空 awaiting 或 extra 非空 → 明确错误（对齐 Python `_agent.py:1469-1550`）in `crates/agent_scope_agent/src/hitl_resume.rs`
- [ ] T018 [US2] 校验 reply_id 匹配暂停回复（FR-010）

**Checkpoint**: 错误契约完整，状态机不会错乱

---

## Phase 5: User Story 3 - 确认结果可携带放行规则 (Priority: P3)

**Goal**: `ConfirmResult.rules` 采纳进引擎，后续同类调用不再询问

**Independent Test**: 确认结果带 `rules:[allow(...)]` → 恢复后同工具不再触发确认

### Tests for User Story 3 (TDD - write FIRST, FAIL) ⚠️

- [ ] T019 [P] [US3] 黄金快照测试：确认结果携带 `rules:[allow(tool)]` → 恢复执行后同工具再次调用直接放行不再 Ask in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`

### Implementation for User Story 3

- [ ] T020 [US3] 确认分支中采纳 `ConfirmResult.rules`：`engine.add_rule(rule)`（对齐 Python `_agent.py:1607-1609`），用 T005 的可变 `PermissionEngine` in `crates/agent_scope_agent/src/hitl_resume.rs`

**Checkpoint**: 确认采纳规则能力可用（对应示例 `a`=总是允许）

---

## Phase 6: User Story 4 - 外部执行结果以事件注入 (Priority: P2)

**Goal**: `ExternalExecutionResultEvent` 恢复外部执行暂停，结果追加 context 并更新工具状态

**Independent Test**: mock 工具触发 `RequireExternalExecutionEvent`，注入结果后继续

### Tests for User Story 4 (TDD - write FIRST, FAIL) ⚠️

- [ ] T021 [P] [US4] 黄金快照测试：工具触发 `RequireExternalExecutionEvent`（携带 tool_calls）→ 流结束暂停 in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`
- [ ] T022 [P] [US4] 黄金快照测试：注入 `ExternalExecutionResultEvent` → 结果追加 context、工具状态 finished、agent 继续 in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`
- [ ] T023 [P] [US4] 黄金快照测试：外部执行结果 id 不匹配 → 报错 in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`

### Implementation for User Story 4

- [ ] T024 [US4] 引擎支持 `RequireExternalExecutionEvent` 暂停（FR-013）in `crates/agent_scope_agent/src/streaming_reactor.rs`
- [ ] T025 [US4] 实现 `ExternalExecutionResultEvent` 恢复分支：结果追加 context、更新工具状态 finished（对齐 Python `_agent.py:1627-1649`）in `crates/agent_scope_agent/src/hitl_resume.rs`
- [ ] T026 [US4] 校验外部执行结果类型/ id 匹配（FR-015）

**Checkpoint**: 外部执行暂停-恢复可用，与确认机制共享 `_check_incoming_event` 校验

---

## Phase 7: User Story 5 - 用户中断事件 (Priority: P3)

**Goal**: `UserInterruptEvent` 打断回复，以 INTERRUPTED 结束；无 awaiting 时 no-op

**Independent Test**: 注入中断 → 以 `ReplyEnd(INTERRUPTED)` 结束

### Tests for User Story 5 (TDD - write FIRST, FAIL) ⚠️

- [ ] T027 [P] [US5] 黄金快照测试：有 awaiting 时注入 `UserInterruptEvent` → `ReplyEnd(INTERRUPTED)` in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`
- [ ] T028 [P] [US5] 黄金快照测试：无 awaiting 时注入中断 → 静默 no-op in `crates/agent_scope_agent/tests/hitl_event_driven_test.rs`

### Implementation for User Story 5

- [ ] T029 [US5] 实现 `UserInterruptEvent` 分支：`has_awaiting_tool_calls` 时 `ReplyEnd(INTERRUPTED)`，否则 no-op（对齐 Python `_agent.py:807-814`）in `crates/agent_scope_agent/src/hitl_resume.rs`

**Checkpoint**: 三类事件输入全对齐（Q2=B）

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: 示例改造 + 回归 + 文档

- [ ] T030 改造 `examples/human-in-the-loop/src/main.rs`：从"截断历史重建重放"改为"暂停-确认-恢复"（消费 `reply_stream_event`，y/n/a 语义不变：y=confirm true、n=confirm false、a=rules 采纳）
- [ ] T031 更新 `examples/human-in-the-loop/README.md`：描述暂停-确认-恢复交互（去除"重建 agent 重放"说明）
- [ ] T032 回归：全仓 `cargo test --workspace`（原依赖"denied 喂回"行为的测试按 Python 语义更新）
- [ ] T033 [P] 回归：`cargo clippy --workspace --all-targets` + `cargo fmt --check`
- [ ] T034 运行 `specs/032-hitl-event-driven/quickstart.md` 全部验证场景
- [ ] T035 更新兼容性矩阵：记录"Ask 暂停语义"与 Python 对齐、"重建重放"为已知偏差移除

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup - BLOCKS all user stories
- **User Stories (Phase 3-7)**: All depend on Foundational
  - US1 (P1) → US2/P3 顺序（US2 校验依赖 US1 的 awaiting 判定，US3 rules 依赖 US1 恢复路径）
  - US4 (P2)、US5 (P3) 与 US1/2 共享 `_check_incoming_event` / `_handle_incoming_event` 基础设施，可在 US1 后并行
- **Polish (Phase 8)**: Depends on all desired user stories

### User Story Dependencies

- **US1 (P1)**: After Foundational - no deps on other stories (MVP)
- **US2 (P2)**: After Foundational - reuses US1 awaiting 判定
- **US3 (P3)**: After US1 - reuses 恢复路径 + T005 可变引擎
- **US4 (P2)**: After Foundational - shares 校验基础设施
- **US5 (P3)**: After Foundational - shares 状态判定

### Within Each User Story

- 黄金快照测试 MUST 先写并 FAIL，再实现
- 事件消费逻辑 → 校验 → 集成

### Parallel Opportunities

- Setup 任务 T001/T002 可并行
- 各 story 的黄金快照测试（T010/011、T014-16、T019、T021-23、T027-28）可并行
- US4、US5 可在 US1+US2 完成后并行（共享校验基础设施）
- Polish 中 T032/T033 可并行

---

## Parallel Example: 黄金快照测试

```bash
# 并行写测试（各测独立行为）：
Task: "T010 单工具 Ask 暂停测试"
Task: "T014 非法恢复报错测试"
Task: "T019 rules 采纳测试"
Task: "T021 外部执行暂停测试"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. 完成 Phase 1: Setup（EventInput + 测试骨架）
2. 完成 Phase 2: Foundational（Ask 暂停 + 可变引擎 + reply_stream_event + awaiting 判定）
3. 完成 Phase 3: User Story 1（确认恢复）
4. **STOP and VALIDATE**: T010/T011 测试通过，暂停-确认-恢复全链路可用
5. 这是最小可用闭环（对齐 Python 核心语义）

### Incremental Delivery

1. Setup + Foundational → 引擎支持暂停语义
2. 加 US1 → 确认恢复（MVP，对齐 Python HITL 核心）
3. 加 US2 → 错误契约
4. 加 US3 → rules 采纳
5. 加 US4 → 外部执行
6. 加 US5 → 中断
7. 每个 story 独立可测，不破坏前序

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- 每个 user story 独立可完成可测试
- 黄金快照测试先 FAIL 再实现
- Commit after each task or logical group
- 引擎改动集中在 `agent_scope_agent`，`agent_scope_event` 类型定义不动
- `reply_stream` 原签名保留（18 处调用点不动），新增 `reply_stream_event`
