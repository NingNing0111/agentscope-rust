# Tasks: 任务工具输出质量优化（Task Tools Output Optimization）

**Input**: Design documents from `/specs/033-task-tools-optimization/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/task-tools-output.md, quickstart.md

**Tests**: 本特性**显式要求**测试迁移与新增（FR-008 / SC-004 / plan Decision 6）。每个用户故事阶段先写测试（预期 FAIL），再实现。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

**核心约束**（宪法与契约）：
- 工具名、输入 Schema、状态/依赖/错误语义与数据模型（`agent_scope_state`）**零变更**（FR-005 / SC-006）——只改输出文本与展示层
- 输出文本协议逐字以 `contracts/task-tools-output.md` 为准（Rust 优化版，已批准偏差）
- 错误文本同样以 `\n` 结尾；中断/取消路径（`ToolResultState::Interrupted`）不补换行
- 库代码禁 unwrap/expect/panic（锁中毒按既有模式）；`#![deny(unsafe_code)]`

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)

---

## Phase 1: Setup

**Purpose**: 确认基线并快照当前输出协议，为断言迁移提供对照

- [X] T001 [P] Verify green baseline and snapshot current task-tool output protocol — run `rtk cargo test -p agent_scope_agent` to confirm baseline green (task_tools_tests.rs / task_tools_e2e_tests.rs pass); record the current exact output strings (TaskCreate success, TaskList empty/non-empty, TaskGet success/not-found, TaskUpdate not-found/delete/no-updates/update) as the migration baseline; target protocol is `specs/033-task-tools-optimization/contracts/task-tools-output.md`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 换行终止的**工具层**统一机制（FR-001）——所有任务工具完整结果文本经 `text_chunk` 生成，必须先加尾随 `\n`，否则 US1/US2/US3 的输出断言全部失败

**⚠️ CRITICAL**: 本阶段完成前，任何用户故事的输出文本断言都无法通过

- [X] T002 Implement trailing-`\n` termination in `text_chunk` in `crates/agent_scope_agent/src/task_tools.rs` (fn `text_chunk` ~line 29): append `\n` to the output `text` if it does not already end with `\n` (idempotent). This makes all 4 task tools' complete result texts (Success / Error / delete / no-updates paths) end with `\n` per `contracts/task-tools-output.md` §0.1
- [X] T003 Migrate base exact-text assertions in `crates/agent_scope_agent/tests/task_tools_tests.rs` to the trailing-`\n` protocol — ONLY the outputs NOT reformatted by US2/US3: TaskCreate success (~line 101, ~line 108), TaskList empty (~line 144) / non-empty (~line 153), TaskGet not-found (~line 178), TaskUpdate not-found (~line 317), TaskUpdate delete (~line 301), TaskUpdate no-updates (~line 224); append `\n` to expected strings (or assert `.ends_with('\n')`); keep state / dependency / metadata semantic assertions unchanged

**Checkpoint**: `rtk cargo test -p agent_scope_agent task_tools` — 任务工具文本已换行终止，基础断言迁移完成

---

## Phase 3: User Story 1 - 多工具调用的结果清晰可读 (Priority: P1) 🎯 MVP

**Goal**: FR-002 展示层对任意"完整结束"的工具结果文本统一补换行（幂等），使同一轮连续多个工具结果独立成行、与后续模型推理以换行分隔（含非任务工具如 Bash/Grep）

**Independent Test**: Scripted Model 在一轮中连续产生 3 次 TaskCreate，驱动 ReAct 循环；断言每个完整 `ToolResultTextDelta` 文本以 `\n` 结尾、连续结果不拼接；构造一个输出无尾随换行的非任务工具，断言其完整结果文本被补 `\n`（事件 delta 与上下文存储文本一致）；中断/取消路径不产生残留换行

### Tests for User Story 1（先写、先 FAIL）⚠️

- [X] T004 [P] [US1] Add event-level newline assertions in `crates/agent_scope_agent/tests/streaming_tests.rs` (or a focused new test using Scripted Model): a single reply with 3 consecutive TaskCreate calls — assert each complete `ToolResultTextDelta` `delta.ends_with('\n')` and consecutive results don't concatenate; a non-task tool whose text has no trailing `\n` — assert its complete result delta and `add_tool_result_to_context`-stored text both end with `\n` (FR-002); an interrupted/cancelled tool result — assert no newline residue

### Implementation for User Story 1

- [X] T005 [US1] Apply newline completion to complete-result emission points in `crates/agent_scope_agent/src/streaming_reactor.rs`: in `emit_tool_result_and_collect` — Complete path (~line 1657-1698, normalize `text` before the `ToolResultTextDelta` emit at ~line 1683) and Stream completion path (~line 1774-1786, ensure final delta + returned/collected text end with `\n`, appending only if missing); and in `emit_denied_tool_result` (~line 1224) apply the same idempotent append; `ToolResultState::Interrupted` returns must NOT be modified (contract §0.3)
- [X] T006 [P] [US1] Apply the same idempotent newline completion to the batch path in `crates/agent_scope_agent/src/react_loop.rs`: Stream branch (~line 640-733), Complete branch (~line 734-797), Err branch (~line 798-849) — normalize the emitted `ToolResultTextDelta` text and the `ToolOutput::Text(...)` persisted into `state.context` to end with `\n` if missing (different file from T005 → parallel)

**Checkpoint**: 事件级换行断言通过；US1 可独立验证（连续工具结果独立成行、非任务工具补换行）

---

## Phase 4: User Story 2 - 更新结果报告实际变更 (Priority: P1)

**Goal**: FR-003 TaskUpdate 输出报告实际应用的字段值——状态类报新值、依赖类报实际新增 id 列表、多项变更全部列出

**Independent Test**: 直接调用 TaskUpdate（不经模型）：仅更新状态、同时更新状态与依赖、无实际变更、删除、任务不存在五种场景，输出均准确反映实际变化（对照 `contracts/task-tools-output.md` §4）

### Tests for User Story 2（先写、先 FAIL）⚠️

- [X] T007 [US2] Add TaskUpdate value-reporting assertions in `crates/agent_scope_agent/tests/task_tools_tests.rs` — write BEFORE implementation, expect FAIL: status-only → `Updated task (id=2): status=in_progress`; status+dependency → `Updated task (id=1): status=in_progress; add_blocked_by=[4]`; multi-field order per contract table (subject→description→add_blocks→add_blocked_by→status→owner→metadata); completed → `Updated task (id=3): status=completed` + blank line + `Task completed. Call TaskList now to find your next available task or see if your work unblocked others.`; all with trailing `\n`

### Implementation for User Story 2

- [X] T008 [US2] Rework TaskUpdate output building in `crates/agent_scope_agent/src/task_tools.rs` (~line 440-566): replace the name-only `updated_fields: Vec<String>` with a `Vec<(String, String)>` of field→actual-value pairs; collect actual added ids for `add_blocks` / `add_blocked_by` (skip self-references / already-present / non-existent ids, matching the existing `added_any` guards at ~line 453-497), the new `status`/`owner`/`subject`/`description` values, and affected `metadata` keys; output `Updated task (id={id}): {field}={value}; ...` (contract §4); keep delete (`Task (id={id}) has been deleted.`), no-updates, and not-found messages unchanged (all gain trailing `\n` via `text_chunk`); keep the completed guide append `\n\nTask completed. Call TaskList ...`
- [X] T009 [US2] Migrate old TaskUpdate exact-text assertions in `crates/agent_scope_agent/tests/task_tools_tests.rs` (~line 211, ~line 236-241, ~line 254, ~line 283) from `Update task (id=X) {fields}.` to the new value-reporting format + trailing `\n`

**Checkpoint**: `rtk cargo test -p agent_scope_agent task_tools` — US2 报值断言通过；US2 可独立验证

---

## Phase 5: User Story 3 - 任务详情保持紧凑 (Priority: P2)

**Goal**: FR-004 TaskGet 对超过阈值（默认 200 字符）的 description 截断，输出前缀 + 省略提示 + 完整长度

**Independent Test**: 构造描述长度分别 >200 / ==200 / <200 / 空 的 4 个任务，调用 TaskGet 验证截断/完整/边界/空描述行为（对照 `contracts/task-tools-output.md` §3）

### Tests for User Story 3（先写、先 FAIL）⚠️

- [X] T010 [US3] Add TaskGet description-truncation assertions in `crates/agent_scope_agent/tests/task_tools_tests.rs` — write BEFORE implementation, expect FAIL: `len>200` → `Description: {前 200 字符}… (truncated, {len} chars total)`; `len==200` → full description (no truncation); `len<200` → full description; empty description → `Description: ` (empty line, no error); not-found keeps `Task not found` (Error state)

### Implementation for User Story 3

- [X] T011 [US3] Add `TASK_DESCRIPTION_MAX_CHARS: usize = 200` constant and apply description truncation in `crates/agent_scope_agent/src/task_tools.rs` TaskGet (`format!("Description: {}", task.description)` at ~line 292): if `len > 200` output `{前 200 字符}… (truncated, {len} chars total)`; `len <= 200` output full; empty outputs empty line; truncation only affects the output text — `Task.description` storage and the data model are unchanged (contract §3, data-model.md §输出文本层)

**Checkpoint**: 截断断言通过；US3 可独立验证（紧凑详情 + 完整长度提示）

---

## Phase 6: User Story 4 - 示例渲染同步改进 (Priority: P2)

**Goal**: FR-009 `plan-react-agent` 示例流式渲染输出分段清晰、工具输入与结果可对应，不再放大拼接问题

**Independent Test**: 运行 `examples/plan-react-agent`（Scripted/Mock 或真实 key），目视检查一轮多工具调用与最终答复的输出分段清晰、工具输入↔结果可对应

### Implementation for User Story 4

- [X] T012 [US4] Improve event rendering in `examples/plan-react-agent/src/main.rs` (~line 79-91): the protocol now supplies trailing `\n` so `print!` for `ToolResultTextDelta` / `TextBlockDelta` (~line 89-90) separates naturally; add only minimal visual separation between event groups (e.g. newline after the `ToolCallEnd` input print at ~line 84-87) so each tool call's input↔result pair is visually paired — minimal change per research Decision 5

**Checkpoint**: 示例输出分段清晰；US4 可独立验证

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 契约/兼容矩阵/文档收尾 + 全量验收（FR-006 / FR-007 / FR-008 完整性 / 宪法第十七条）

- [X] T013 [P] Update `specs/024-agent-task-planning/contracts/task-tools.md` output-text section to declare the task-tool success output is now Rust-optimized (newline-terminated; TaskUpdate reports actual field values; TaskGet truncates descriptions >200 chars) and no longer verbatim-aligned with Python; reference `specs/033-task-tools-optimization/contracts/task-tools-output.md` (FR-006); tool names / input schema / behavior semantics remain per the 024 contract
- [X] T014 [P] Register the output-text deviation in `specs/001-compatibility-baseline/capability-matrix.json` for `tool-task-create` / `tool-task-list` / `tool-task-get` / `tool-task-update` (4 entries, currently status=NOT_ANALYZED, notes empty): add `notes` with the Feature 033 deviation text from `contracts/task-tools-output.md` §6 (newline-terminated; TaskUpdate reports actual values; TaskGet truncates >200 chars; tool names / schemas / semantics / data model unchanged; contract reference), following the `tool-reset-tools` ResetTools naming-deviation notes style (FR-007)
- [X] T015 [P] Check `crates/agent_scope_agent/tests/task_tools_e2e_tests.rs` for any exact-output-text assertions (currently only `contains("All done")` on the reply text) — verify none assert task-tool output verbatim; if any found, migrate them to the new protocol (FR-008 completeness)
- [X] T016 [P] Update `docs/rust/zh/building-blocks/agent/configure-agent.md` to mention the new task-tool output protocol (newline-terminated results, value-reporting TaskUpdate, truncated TaskGet) as needed; if the en counterpart `docs/rust/en/building-blocks/agent/` contains the file, sync it
- [X] T017 Run the full acceptance gate from `specs/033-task-tools-optimization/quickstart.md` scenario 5/6: `rtk cargo test --workspace` (all tests incl. migrated task-tool assertions pass), `rtk cargo clippy --workspace --all-targets -- -D warnings`, `rtk cargo fmt --check`, `examples/plan-react-agent` compiles/runs; verify the compatibility-matrix snippet reports all 4 `tool-task-*` `notes` contain "Feature 033"

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS all user stories (工具层换行是协议基础)
- **User Stories (Phase 3+)**: All depend on Foundational completion
  - US1 → US2 → US3 → US4 can proceed sequentially (priority order)
  - US2 / US3 are file-local edits to `task_tools.rs` output builders and do not depend on US1's display-layer changes (they inherit trailing `\n` from T002); they can start after Foundational
- **Polish (Phase 7)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: After Foundational (T002/T003) — no dependency on other stories
- **User Story 2 (P1)**: After Foundational — independently testable (direct TaskUpdate calls)
- **User Story 3 (P2)**: After Foundational — independently testable (direct TaskGet calls)
- **User Story 4 (P2)**: Depends on US1 (protocol-driven readability is the main fix; rendering tweak is minimal)

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Implementation → test migration → checkpoint

### Parallel Opportunities

- T001 Setup is parallel
- T004 (US1 test) is parallel with T005/T006 (different files)
- T005 (streaming_reactor.rs) and T006 (react_loop.rs) are parallel
- T013/T014/T015/T016 (Polish, different files) are parallel
- US2 and US3 both only touch `task_tools.rs` — NOT parallel with each other (same file)

---

## Parallel Example: User Story 1

```bash
# Test-first (must fail before impl):
Task: "T004 Add event-level newline assertions in tests/streaming_tests.rs"

# Implementation (two files in parallel):
Task: "T005 Apply newline completion in src/streaming_reactor.rs"
Task: "T006 Apply newline completion in src/react_loop.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Complete Phase 1: Setup (baseline snapshot)
2. Complete Phase 2: Foundational (T002 text_chunk 换行 + T003 基础断言迁移 — CRITICAL, blocks all stories)
3. Complete Phase 3: User Story 1 (展示层补换行 + 事件级断言)
4. **STOP and VALIDATE**: `rtk cargo test -p agent_scope_agent` — 连续工具结果独立成行（拼接缺陷已修复，核心用户价值达成）
5. 因 US2 同为 P1 且与 US1 文件不冲突，推荐 MVP 交付 US1+US2（拼接 + 报值）一起验证后再进入 US3/US4

### Incremental Delivery

1. Complete Setup + Foundational → 工具层换行协议就绪
2. Add US1 → 展示层可读性达成 → 验证（MVP!）
3. Add US2 → TaskUpdate 报实际值 → 验证（P1 双目标完成）
4. Add US3 → TaskGet 截断 → 验证
5. Add US4 → 示例渲染分段 → 验证
6. Polish → 契约/兼容矩阵/文档/全量验收

### Parallel Team Strategy

With multiple developers:

1. Developer A: US1 (streaming_reactor.rs + react_loop.rs)
2. Developer B (after Foundational): US2 (task_tools.rs TaskUpdate)
3. Developer C (after US2 done, same file): US3 (task_tools.rs TaskGet)
4. Polish (T013-T016) parallel once stories complete; T017 final gate

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- 输出文本协议以 `specs/033-task-tools-optimization/contracts/task-tools-output.md` 为唯一基准（取代 024 契约的输出文本部分）
- 数据模型（`agent_scope_state`）零变更 —— 禁止触碰 `Task` / `TaskContext` / serde 布局
- 中断/取消路径（`ToolResultState::Interrupted`）不补换行（契约 §0.3）
- 提交粒度：每个 checkpoint 一个 commit（先 T001-T003 提交，再每用户故事一个提交，Polish 收尾提交）
- Avoid: 修改工具名/输入 Schema/状态/依赖/错误语义；在 `agent_scope_state` 加字段；unwrap/expect/panic（测试代码除外）
