---

description: "Task list for Agent 运行时状态注入系统"

---

# Tasks: Agent 运行时状态注入系统

**Input**: Design documents from `/specs/026-runtime-state-injection/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: 测试任务包含在内（spec 以 Python `agent_injection_test.py` 为行为基准，research R10 明确 13 个测试场景；宪法第六条要求测试驱动兼容性）。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Rust 多 crate 库**: `crates/agent_scope_agent/src/`, `crates/agent_scope_agent/tests/`
- 依赖通过 workspace `Cargo.toml` 管理

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 新增依赖、配置类型与测试夹具等共享基础设施

- [X] T001 在 `Cargo.toml`（workspace root）的 `[workspace.dependencies]` 新增 `chrono-tz = "0.10"`（IANA 时区解析，对齐 Python `ZoneInfo`）
- [X] T002 [P] 在 `crates/agent_scope_agent/Cargo.toml` 的 `[dependencies]` 引入 `chrono-tz.workspace = true`
- [X] T003 [P] 在 `crates/agent_scope_agent/tests/mocks.rs` 扩展/确认 MockModel 支持确定 `count_tokens`（对齐 Python `MockModel.count_tokens = AsyncMock(return_value=...)`）与 `context_size`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: `InjectionConfig` 类型与校验，以及统一注入管线核心——所有用户故事的阻断前提

**⚠️ CRITICAL**: 无用户故事工作可在本阶段完成前开始

- [X] T004 在 `crates/agent_scope_agent/src/config.rs` 新增 `InjectionConfig` 结构体，字段与默认值对齐 Python `_config.py::InjectionConfig`（`inject_runtime_state=true`、`timezone="UTC"`、`time_format="%Y-%m-%dT%H:%M:%S"`、`time_interval=0.5`、`context_buffer_ratio=0.2`、`template` 含 `{runtime_state}` 占位符、`injection_source`、`task_tool_names` 四任务工具名、`extra_fields`、`emit_hint_event=true`）
- [X] T005 在 `crates/agent_scope_agent/src/config.rs` 实现 `InjectionConfig::validate()`：模板缺 `{runtime_state}` 拒绝、`time_format` 不可往返拒绝、`time_interval < 0` 拒绝、`context_buffer_ratio` 超出 `[0,1]` 拒绝、`context_buffer_ratio >= ContextConfig.trigger_ratio` 拒绝；均返回 `AgentError::InvalidConfig { field, message }`
- [X] T006 在 `crates/agent_scope_agent/src/config.rs` 为 `AgentConfig` 新增 `injection_config: InjectionConfig` 字段（默认 `InjectionConfig::default()`），`AgentConfigBuilder` 新增 `.injection_config(...)` 方法，`build()` 调用 `injection_config.validate()`；`AgentConfigBuilder::default()` 初始化该字段
- [X] T007 在 `crates/agent_scope_agent/src/config.rs` 新增 `InjectionConfig` 的单元测试（`#[cfg(test)]`）：默认值断言、模板缺占位符拒绝、格式不可往返拒绝、缓冲比例越界拒绝、缓冲比例 >= trigger 拒绝
- [X] T008 [P] 在 `crates/agent_scope_agent/src/runtime_injection.rs` 新建统一注入管线模块：定义 `maybe_inject_runtime_state(state, agent_name, config, now, cur_iter, input_tokens, task_tools_enabled) -> Option<HintBlockEvent>`，内部完成三维评估 + 单条 HintBlock 组装 + 追加到 `state.context`（写锁临界区）
- [X] T009 [P] 在 `crates/agent_scope_agent/src/runtime_injection.rs` 实现时间维度评估：`now` 按 `config.timezone` 解析（失败回退 UTC）、按 `time_format` 格式化；扫描上下文反向 assistant 消息找最新含 `<current-time>` 的注入、解析记录时间、按 `<timezone>` 标注恢复时区；判定注入条件（无记录/解析失败/超间隔/时钟回拨 elapsed<0）
- [X] T010 [P] 在 `crates/agent_scope_agent/src/runtime_injection.rs` 实现任务维度评估：统计 `tasks_context` 中 pending/in_progress 数量；扫描上下文检测感知（含 `<tasks>` 的同源 HintBlock 或 `name ∈ task_tool_names` 的 ToolCallBlock）；注入条件为 有未完成任务 + 不感知 + `inject_runtime_state && task_tools_enabled`；任务字段文本与 024 逐字一致
- [X] T011 [P] 在 `crates/agent_scope_agent/src/runtime_injection.rs` 实现上下文用量维度评估：仅当 `cur_iter == 1` 时，`input_tokens > max(0, trigger_ratio - context_buffer_ratio) * context_size` 则注入；`input_tokens` 与 `context_size` 由调用点传入
- [X] T012 在 `crates/agent_scope_agent/src/runtime_injection.rs` 实现三维字段组装：按序（current-time → timezone → tasks → context-length → extra_fields）`\n` 连接为 `joined_fields`，用 `template.replace("{runtime_state}", joined_fields)` 渲染（保留模板其他花括号）；单次调用至多产出一条 HintBlock 与一个 `HintBlockEvent`
- [X] T013 在 `crates/agent_scope_agent/src/lib.rs` 导出 `runtime_injection` 模块（及 `maybe_inject_runtime_state`），并在 `config.rs` 导出 `InjectionConfig`
- [X] T014 在 `crates/agent_scope_agent/src/task_reminder.rs` **保留 Feature 024 任务提醒原逻辑**（非薄封装）：实现中发现薄封装委托统一管线会在"已感知任务"场景注入时间字段、破坏 024 测试，故按宪法第一条保留 024 独立实现以保兼容基线；统一管线用于调用点（react_loop/streaming_reactor）
- [X] T015 在 `crates/agent_scope_agent/tests/task_reminder_tests.rs` 运行既有测试确认经薄封装后仍全部通过（024 兼容基线不回归）

**Checkpoint**: Foundation ready - 统一管线核心与配置类型可用，用户故事实现可并行开始

---

## Phase 3: User Story 1 - 时间注入 (Priority: P1) 🎯 MVP

**Goal**: 统一管线在首轮/超间隔/压缩后/时钟回拨时注入当前时间与时区，近间隔不重复注入

**Independent Test**: 构造空上下文 + 固定 `now`，调用 `maybe_inject_runtime_state`，断言 hint 含 `<current-time>` 与 `<timezone>` 且被 `<system-reminder>` 模板包裹、`source` 正确；构造近间隔上下文断言零注入

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T016 [P] [US1] 首轮触发时间注入测试（空上下文 + `now=2026-07-01T12:00:00Z` → hint 含 `<current-time>2026-07-01T12:00:00</current-time>\n<timezone>UTC</timezone>`）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T017 [P] [US1] 长间隔/近间隔测试（6 小时前注入 → 重注入；10 分钟前 → 零注入）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T018 [P] [US1] 压缩后重注入测试（上下文清空后再次调用 → 重新注入）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T019 [P] [US1] 记录时区生效测试（`Asia/Shanghai` 记录 10 分钟前墙钟时间 → elapsed 正确不注入；同一墙钟读作 UTC → 负 elapsed 触发注入）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T020 [P] [US1] 无效时区回退 UTC 测试（`Mars/Olympus_Mons` 不报错，墙钟按 UTC 计算，`<timezone>` 注入原始配置值）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`

### Implementation for User Story 1

- [X] T021 [P] [US1] 在 `crates/agent_scope_agent/src/react_loop.rs` 调用点接入统一管线：循环顶部以默认 `InjectionConfig` 调用 `maybe_inject_runtime_state`，传入 `now`（`Utc::now()`）、局部 `cur_iter`、首轮 token 计数、`task_tools_enabled`；注入发生时发送 `AgentEvent::HintBlock`（`emit_hint_event` 由 config 控制）
- [X] T022 [P] [US1] 在 `crates/agent_scope_agent/src/streaming_reactor.rs` 调用点接入统一管线（与 react_loop 相同逻辑）；注入事件经 `event_tx` 发送
- [X] T023 [US1] 在 `crates/agent_scope_agent/src/config.rs` 确认 `AgentConfig::build()` 构造默认 `InjectionConfig` 并校验通过（时间维度默认启用）

**Checkpoint**: User Story 1 完全可用——首次回复 Agent 能感知当前时间；测试 T016-T020 通过

---

## Phase 4: User Story 2 - 任务提醒纳入统一管线 (Priority: P2)

**Goal**: 024 任务提醒升级到统一管线，文本/来源/感知行为逐字兼容不回归

**Independent Test**: 复用 024 测试场景（未完成任务+无痕迹 → 注入；已感知/无未完成 → 不注入），经统一管线后全部通过且文本逐字一致

### Tests for User Story 2 ⚠️

- [X] T024 [P] [US2] 待办任务触发注入测试（1 个 pending 任务 + 无任务痕迹 → hint 含 `<tasks>You have 0 in-progress tasks and 1 pending tasks. Use \`TaskList\` to view them if you don't know.</tasks>`）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T025 [P] [US2] 已感知任务不重复注入测试（任务工具调用痕迹 或 先前 `<tasks>` 提醒 在上下文中 → 零注入）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T026 [P] [US2] 双开关测试（`task_tools_enabled=false` 抑制任务注入；`inject_runtime_state=false` 抑制全部三维）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T027 [P] [US2] 兼容回归测试——既有 `crates/agent_scope_agent/tests/task_reminder_tests.rs` 全部通过（含 `test_disabled_flag_via_loop_does_not_inject`）

### Implementation for User Story 2

- [X] T028 [US2] 确认 `task_reminder.rs` 保留 024 任务提醒原逻辑，`task_reminder_tests.rs` 断言文本逐字一致（`<tasks>` 字段、`SOURCE`、`<system-reminder>` 包裹）且全部通过

**Checkpoint**: User Story 1 AND 2 均独立工作——任务提醒行为经统一管线不回归

---

## Phase 5: User Story 3 - 上下文用量预警 (Priority: P3)

**Goal**: 首轮 token 落入预警窗口时注入上下文用量字段，独立于其他维度

**Independent Test**: 构造首轮 + `count_tokens=700`/`context_size=1000`/`trigger=0.8`/`buffer=0.2` → hint 含 `<context-length>`；token 远离阈值或非首轮 → 零注入

### Tests for User Story 3 ⚠️

- [X] T029 [P] [US3] 上下文用量触发测试（首轮 + 700 tokens → hint 含 `<context-length>Your current context contains 700 tokens. When reaching 800 tokens, your context will be compressed.</context-length>`）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T030 [P] [US3] 上下文用量独立共存测试（首轮同时满足时间+上下文 → 同一条 hint 含 `<current-time>` 与 `<context-length>`）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T031 [P] [US3] 非首轮不评估上下文测试（`cur_iter != 1` 即使 token 超阈值 → 零上下文注入）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`

### Implementation for User Story 3

- [X] T032 [US3] 在 `crates/agent_scope_agent/src/runtime_injection.rs` 确认上下文维度取 `ContextConfig.trigger_ratio` 与调用点传入的 `input_tokens`/`context_size` 计算阈值；调用点（react_loop/streaming_reactor）在首轮提供 token 计数（复用压缩检查处的 `count_tokens` 或独立调用）

**Checkpoint**: 全部用户故事独立可用——三维注入管线完整

---

## Phase 6: User Story 4 - 注入配置化与事件集成 (Priority: P4)

**Goal**: 注入行为可配置、注入时发射 `HintBlockEvent`、非法配置结构化拒绝

**Independent Test**: 逐项改配置验证行为变化；订阅事件流验证 `AgentEvent::HintBlock` 发射

### Tests for User Story 4 ⚠️

- [X] T033 [P] [US4] 总开关关闭测试（`inject_runtime_state=false` → 零注入、零事件、上下文无追加）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T034 [P] [US4] 附加字段附着/不触发测试（`extra_fields` 附着于触发注入的同一 hint；仅配置附加字段无维度命中 → 零注入）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T035 [P] [US4] 模板花括号保留测试（自定义模板 `{"reminder": "{runtime_state}"}` → hint 保留花括号）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T036 [P] [US4] 事件发射测试（`emit_hint_event=true` 注入时发出 `AgentEvent::HintBlock(HintBlockEvent)` 携带 `reply_id`/`block_id`/`source`/`hint`；`false` 时零发射）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`
- [X] T037 [P] [US4] 自定义任务工具名列表测试（`task_tool_names` 自定义后，仅匹配自定义名视为已感知任务）在 `crates/agent_scope_agent/tests/runtime_injection_tests.rs`

### Implementation for User Story 4

- [X] T038 [US4] 在 `crates/agent_scope_agent/src/config.rs` 确认 `AgentConfigBuilder::injection_config(...)` 支持自定义 `InjectionConfig`（含 `template`/`injection_source`/`task_tool_names`/`extra_fields`/`emit_hint_event`/`timezone` 等），构建时校验
- [X] T039 [US4] 在 `crates/agent_scope_agent/src/react_loop.rs` 与 `streaming_reactor.rs` 确认调用点将 `config.injection_config` 传入统一管线，事件发射遵循 `emit_hint_event`

**Checkpoint**: 全部 4 个用户故事独立可用，配置化与事件集成完成

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 文档、质量门、quickstart 验证与收尾

- [X] T040 [P] 在 `specs/026-runtime-state-injection/quickstart.md` 按 8 个验证场景逐一运行确认端到端可用
- [X] T041 [P] 运行 `rtk cargo test -p agent_scope_agent` 全量通过（新测试 + 024 兼容基线）
- [X] T042 [P] 运行 `rtk cargo clippy -p agent_scope_agent` 与 `rtk cargo fmt --check -p agent_scope_agent` 无告警
- [X] T043 [P] 更新 `docs/` 或 `README.md` 中的 Agent 配置文档，说明 `injection_config` 用法与默认值
- [X] T044 更新兼容性矩阵/CHANGELOG：记录上游 commit `9d1026fa`、`chrono-tz` 依赖、`InjectionConfig` 新增、任务维度注入从 024 独立实现迁移到统一管线
- [X] T045 全量回归：`rtk cargo test`（工作区）、确认无未登记的 `UnsupportedFeature`、无静默降级

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories then proceed sequentially in priority order (P1 → P2 → P3 → P4)；因四者共享统一管线核心，串行推进更高效
- **Polish (Final Phase)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: 时间维度——依赖统一管线骨架（T008）与调用点接入；无其他故事依赖
- **User Story 2 (P2)**: 任务维度——依赖统一管线骨架与薄封装（T014）；兼容 024 基线
- **User Story 3 (P3)**: 上下文维度——依赖统一管线骨架；独立评估
- **User Story 4 (P4)**: 配置化与事件——依赖三维接入调用点（T021/T022）

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- 统一管线骨架（T008-T012）先于调用点接入
- 调用点（react_loop/streaming_reactor）双路径一致
- Story complete before moving to next priority

### Parallel Opportunities

- Phase 1 的 T002/T003 可并行
- Phase 2 的 T008-T011（管线各维度模块）在 T004-T006 配置就绪后部分并行
- 各用户故事的测试任务（T016-T020/T024-T027/T029-T031/T033-T037）标记 [P] 可并行
- Phase 7 的 T040-T043 可并行

---

## Parallel Example: 统一管线核心

```bash
# 配置就绪后，三维评估模块可并行开发（同一文件内独立函数）：
Task: "实现时间维度评估 in runtime_injection.rs"
Task: "实现任务维度评估 in runtime_injection.rs"
Task: "实现上下文用量维度评估 in runtime_injection.rs"

# 各用户故事测试可并行编写（FAIL 先行）：
Task: "首轮触发时间注入测试 in runtime_injection_tests.rs"
Task: "待办任务触发注入测试 in runtime_injection_tests.rs"
Task: "上下文用量触发测试 in runtime_injection_tests.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup（`chrono-tz` 依赖）
2. Complete Phase 2: Foundational（`InjectionConfig` + 统一管线骨架）— CRITICAL
3. Complete Phase 3: User Story 1（时间维度 + 调用点接入）
4. **STOP and VALIDATE**: 测试 T016-T020 通过，Agent 首次回复感知时间
5. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → 统一管线核心可用
2. User Story 1 → 时间注入（MVP）
3. User Story 2 → 任务提醒不回归迁移
4. User Story 3 → 上下文用量预警
5. User Story 4 → 配置化 + 事件集成
6. 每步独立测试，不破坏前序故事

### Parallel Team Strategy

- 统一管线骨架与 `InjectionConfig` 为核心依赖，建议单线程先完成 Phase 1-2
- Phase 3 完成后，后续维度（US2/US3/US4）测试可并行编写

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- 每个用户故事独立可完成、可测试
- 验证测试先失败再实现
- 提交按逻辑分组；在 checkpoint 处停止验证故事独立性
- 避免：模糊任务、同文件冲突、破坏独立性的跨故事依赖
