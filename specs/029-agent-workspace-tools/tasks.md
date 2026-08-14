---

description: "Task list for Feature 029 - Agent Workspace Built-in Tools"
---

# Tasks: Agent Workspace Built-in Tools

**Input**: Design documents from `/specs/029-agent-workspace-tools/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: 本 feature spec 的 acceptance scenarios 与成功标准（SC-001~008）要求测试验证；宪法 Art.6 要求测试驱动兼容。包含测试任务（golden snapshot + diff test 对齐 vendored Python 参考实现 `agentscope/src/agentscope/tool/_builtin/`）。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Crate workspace**: `crates/agent_scope_tool/src/builtin/`（工具实现）、`crates/agent_scope_agent/src/`（注入）、`crates/agent_scope_state/src/`（激活状态）
- **Tests**: `crates/agent_scope_tool/tests/`
- 路径依 plan.md Project Structure（见 `specs/029-agent-workspace-tools/plan.md`）

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 项目初始化与基础结构——`agent_scope_tool` 的 builtin 模块骨架、错误模型扩展、依赖确认。

- [X] T001 确认 `agent_scope_tool` 依赖 `agent_scope_workspace`（Cargo.toml 已有）与 `agent_scope_state`（如需），在 crates/agent_scope_tool/Cargo.toml 添加缺失依赖
- [X] T002 [P] 创建 `crates/agent_scope_tool/src/builtin/mod.rs` 模块骨架并加入 lib.rs re-export
- [X] T003 [P] 在 `crates/agent_scope_tool/src/tool_trait.rs` 的 `ToolError` 新增 `UnsupportedCapability` variant（对齐宪法 Art.13 与 research.md Decision 5）
- [X] T004 创建测试辅助 `crates/agent_scope_tool/tests/common/mod.rs`（临时 workspace + 文件 fixture 构造）

**Checkpoint**: Setup 完成，builtin 模块骨架就绪。

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 所有 user story 共同依赖的基础——`WorkspaceToolSession`（read-state + 激活组视图）、工具注册注入机制。

**⚠️ CRITICAL**: 无此阶段完成，任何 user story 都无法实现。

- [X] T005 实现 `WorkspaceToolSession` 在 `crates/agent_scope_tool/src/builtin/session.rs`：`read_files: BTreeSet<String>`、`active_tool_groups: BTreeSet<String>`、`record_read/is_read/clear_reads/record_groups/list_groups`（对齐 contracts/workspace-tool-session.md）
- [X] T006 实现 `BuiltInToolInfo` 契约类型（`name/description/input_schema/availability/read_only/concurrency_safe`）在 `crates/agent_scope_tool/src/builtin/mod.rs`（对齐 data-model.md ToolInfo 扩展决策）
- [X] T007 [P] 在 `crates/agent_scope_tool/src/toolkit.rs` 为 `ToolGroup` 增加激活状态支持：`active: bool` 或 `ToolKit` 内维护激活组集合，使 `get_tool_schemas()` 按激活组过滤（research.md Decision + ToolGroup 缺口）
- [X] T008 在 `crates/agent_scope_agent/src/config.rs` 的 `AgentConfig`/`AgentConfigBuilder` 新增 workspace 装配钩子（`workspace_tools_enabled` 或 workspace 引用），不新增对 `agent_scope_workspace` 的硬依赖（用 trait object 或配置回调解耦）
- [X] T009 在 `crates/agent_scope_agent/src/react_agent.rs` 实现构造期合并 workspace 内置工具的逻辑（仿 Feature 024 任务工具 `register_builtin_task_tool` + `try_register` fail-closed 模式）
- [X] T010 测试：`WorkspaceToolSession` 单元测试在 `crates/agent_scope_tool/tests/session_tests.rs`（read-state 归一化、激活组 final-state、workspace 隔离）

**Checkpoint**: Foundation 就绪——`WorkspaceToolSession` 可用、`ToolError::UnsupportedCapability` 就位、`ToolKit` 支持激活组过滤、agent 构造期注入点打通。

---

## Phase 3: User Story 1 - 使用 workspace 后自动获得基础文件与命令工具 (Priority: P1) 🎯 MVP

**Goal**: agent 绑定 workspace 后自动拥有一组基础工具（Bash/Read/Edit/Write/Grep/Glob/ResetTools/Skill [+PowerShell]），无需调用方手动注册；未配置 workspace 的 agent 不暴露。

**Independent Test**: 创建启用 workspace 的 agent，检查可用工具列表含全部必需工具且名称/描述/参数契约完整；创建未启用 workspace 的 agent，确认无文件/命令工具。

### Tests for User Story 1

> **NOTE**: 写测试 FIRST，确保实现前 FAIL（对齐 spec acceptance scenario 1 & SC-001/SC-002）

- [X] T011 [P] [US1] 契约测试：注入工具集合的 schema（名称/描述/参数契约）在 `crates/agent_scope_tool/tests/builtin_injection_tests.rs`
- [X] T012 [P] [US1] 集成测试：未配置 workspace 的 agent 工具列表不含文件/命令工具（FR-002, SC-002）在 `crates/agent_scope_agent/tests/workspace_tools_injection_tests.rs`

### Implementation for User Story 1

- [X] T013 [P] [US1] 实现 `Bash` 工具在 `crates/agent_scope_tool/src/builtin/bash.rs`：`command`/`description`/`timeout`（默认 120000ms 最大 600000ms），经 `WorkspaceBackend::exec_shell`，输出截断 30000 字符，超时 kill（对齐 contracts/bash.md 与 vendored `_bash.py`）
- [X] T014 [P] [US1] 实现 `Read` 工具在 `crates/agent_scope_tool/src/builtin/read.rs`：`file_path`/`offset`/`limit`，`cat -n` 输出格式，成功读取记录到 `WorkspaceToolSession.read_files`（对齐 contracts/read.md 与 `_read.py`）
- [X] T015 [P] [US1] 实现 `Grep` 工具在 `crates/agent_scope_tool/src/builtin/grep.rs`：`pattern`/`path`/`output_mode`/`glob`/`type`/context/i/multiline/`head_limit`/`offset`，native Rust 搜索，结果有界（对齐 contracts/grep.md 与 `_grep.py`）
- [X] T016 [P] [US1] 实现 `Glob` 工具在 `crates/agent_scope_tool/src/builtin/glob.rs`：`pattern`/`path`，globset 匹配，workspace 限定，结果有界（对齐 contracts/glob.md 与 `_glob.py`）
- [X] T017 [P] [US1] 实现 `Edit` 工具骨架在 `crates/agent_scope_tool/src/builtin/edit.rs`：`file_path`/`old_string`/`new_string`/`replace_all`，唯一匹配校验（守卫逻辑归 US2）
- [X] T018 [P] [US1] 实现 `Write` 工具骨架在 `crates/agent_scope_tool/src/builtin/write.rs`：`file_path`/`content`，父目录自动创建（守卫逻辑归 US2）
- [X] T019 [P] [US1] 实现 `Skill` 工具（SkillViewer）在 `crates/agent_scope_tool/src/builtin/skill.rs`：`skill` 参数精确查找，`SkillNotFoundError`（复用/对齐既有 `skill_viewer.rs` 契约，contracts/skill.md）
- [X] T020 [US1] 实现 `PowerShell` 工具在 `crates/agent_scope_tool/src/builtin/powershell.rs`：Windows 环境启用，`pwsh`→`powershell.exe` 探测，`-EncodedCommand`，非 Windows 返回 `UnsupportedCapability`（对齐 contracts/bash.md §PowerShell 与 `_powershell.py`）
- [X] T021 [US1] 实现 `ResetTools` 工具在 `crates/agent_scope_tool/src/builtin/reset_tools.rs`：动态 schema（非 basic 组布尔字段）、final-state 激活语义、写入 `AgentState.tool_context.activated_groups`（对齐 contracts/reset-tools.md 与 `_meta.py`）
- [X] T022 [US1] 实现 `WorkspaceToolSession` 注入：内置工具通过共享 session 绑定 workspace backend 与 read-state（`crates/agent_scope_tool/src/builtin/session.rs` 装配）
- [X] T023 [US1] 集成注入：在 `crates/agent_scope_agent/src/react_agent.rs` 将全部内置工具合并入启用 workspace 的 agent Toolkit（FR-001/FR-002），fail-closed 冲突处理

**Checkpoint**: US1 完成——workspace 启用的 agent 自动获得全部内置工具，schema 契约完整，未启用者无文件/命令工具。

---

## Phase 4: User Story 2 - 安全、可审计地修改 workspace 文件 (Priority: P2)

**Goal**: agent 通过 Read 后 Edit/Write 的约束修改文件，减少误改/覆盖未知内容/不可追踪变更。

**Independent Test**: 让 agent 在已读/未读文件两状态尝试 Edit/Write，验证只有满足前置条件的修改被接受。

### Tests for User Story 2

> **NOTE**: 写测试 FIRST（对齐 spec acceptance scenarios 2 & SC-003/SC-004）

- [X] T024 [P] [US2] 契约测试：未读先 Edit/Write 拒绝（`read_before_modify_required`）在 `crates/agent_scope_tool/tests/builtin_edit_write_tests.rs`
- [X] T025 [P] [US2] 契约测试：Edit 唯一/非唯一 `old_string` 行为（`ambiguous_edit`、`pattern_not_found`）在 `crates/agent_scope_tool/tests/builtin_edit_write_tests.rs`
- [X] T026 [P] [US2] 集成测试：read-before-modify 全流程（read→edit 成功、read→write 覆盖成功、未读拒绝）在 `crates/agent_scope_tool/tests/builtin_edit_write_tests.rs`

### Implementation for User Story 2

- [X] T027 [US2] 实现 Edit 读-改守卫：`Edit.call` 要求目标文件在 `WorkspaceToolSession.read_files`，否则拒绝（FR-008）在 `crates/agent_scope_tool/src/builtin/edit.rs`
- [X] T028 [US2] 实现 Write 覆盖守卫：已存在文件覆盖要求先读，否则拒绝（FR-012）在 `crates/agent_scope_tool/src/builtin/write.rs`
- [X] T029 [US2] 实现 Edit 唯一性校验：`old_string` 未找到 → `pattern_not_found`，多次出现且无 replace_all → `ambiguous_edit`（FR-009）在 `crates/agent_scope_tool/src/builtin/edit.rs`
- [ ] T030 [US2] 实现 Edit/Write 原子写入（临时文件 + rename）与 unified diff 元数据（可追踪，FR-026）在 `crates/agent_scope_tool/src/builtin/edit.rs` / `write.rs`
- [X] T031 [US2] 错误类别映射：Edit/Write 全部拒绝路径返回 typed 错误类别（validation/permission，对齐 data-model.md Art.13 映射，SC-004）在 `crates/agent_scope_tool/src/builtin/edit.rs` / `write.rs`

**Checkpoint**: US2 完成——read-before-modify 守卫全流程可验证，唯一替换成功、非唯一拒绝、新文件创建。

---

## Phase 5: User Story 3 - 高效搜索与技能查看 (Priority: P3)

**Goal**: agent 优先使用专用搜索（Grep/Glob）与技能查看（Skill）工具，获得稳定、结构化、低噪声结果；ResetTools 切换工具组。

**Independent Test**: 向 agent 提供文件搜索、内容搜索、技能查看任务，验证对应工具可完成任务。

### Tests for User Story 3

> **NOTE**: 写测试 FIRST（对齐 spec acceptance scenarios 3 & SC-005/SC-007）

- [X] T032 [P] [US3] 契约测试：Glob 结果限定 workspace 范围（FR-015/FR-016, SC-007）在 `crates/agent_scope_tool/tests/builtin_search_tests.rs`
- [X] T033 [P] [US3] 契约测试：Grep 输出模式/上下文/结果上限（FR-013/FR-014, SC-007）在 `crates/agent_scope_tool/tests/builtin_search_tests.rs`
- [X] T034 [P] [US3] 契约测试：Skill 精确名称命中/未找到错误（FR-020/FR-021）在 `crates/agent_scope_tool/tests/builtin_search_tests.rs`
- [X] T035 [P] [US3] 契约测试：ResetTools 激活/越权拒绝（FR-019）在 `crates/agent_scope_tool/tests/builtin_reset_tools_tests.rs`

### Implementation for User Story 3

- [X] T036 [US3] 完善 Grep 搜索结果界限：文件字节上限、扫描条目上限、结果硬上限（对齐 pi-rust `grep_tool` 的 `MAX_GREP_RESULTS` 等，SC-007）在 `crates/agent_scope_tool/src/builtin/grep.rs`
- [X] T037 [US3] 完善 Glob 搜索结果界限：扫描上限、结果上限、mtime/确定性排序（SC-007）在 `crates/agent_scope_tool/src/builtin/glob.rs`
- [X] T038 [US3] 完善 Skill 激活组交互：`get_skills_method(activated_groups)` 来自 `AgentState.tool_context.activated_groups`（对齐 `_skill.py:112`）在 `crates/agent_scope_tool/src/builtin/skill.rs`
- [X] T039 [US3] 完善 ResetTools 授权边界：仅激活授权范围内组、越权拒绝、不创建新权限（FR-019）在 `crates/agent_scope_tool/src/builtin/reset_tools.rs`
- [X] T040 [US3] Trace 集成：全部内置工具调用出现于 agent trace（工具名/参数概要/错误类别/顺序，FR-025, SC-006）在 `crates/agent_scope_agent/src/` + 各 builtin 工具

**Checkpoint**: US3 完成——搜索/技能/工具组切换可独立验证，trace 可观察。

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: 跨 user story 的收尾——兼容性验证、文档、示例、格式化。

- [X] T041 [P] Python diff 测试：内置工具结果对齐 vendored 参考实现（宪法 Art.1/Art.3/Art.6）在 `tests/compatibility/` 新增 fixture
- [X] T042 [P] 更新兼容性矩阵：ResetTools 命名偏差（Rust `ResetTools` vs Python `reset_tools`）登记（宪法 Art.1 例外流程）在 `specs/001-compatibility-baseline/`
- [ ] T043 [P] 示例验证：`examples/pi-rust/` workspace 启用时自动获得内置工具（验证 SC-005 流程）
- [X] T044 运行 quickstart.md 全部验证场景（`cargo test -p agent_scope_tool` + `cargo test -p agent_scope_agent`）
- [X] T045 [P] 文档更新：`docs/` 与 README 补充 workspace 内置工具说明
- [X] T046 收尾验证：`cargo fmt`、`cargo clippy --all-targets`（零 unsafe、无警告）、`cargo test` 全绿（宪法 Art.9/Art.17）

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖，可立即开始
- **Foundational (Phase 2)**: 依赖 Setup 完成——BLOCKS 所有 user story
- **User Stories (Phase 3+)**: 依赖 Foundational 完成；按 P1→P2→P3 顺序或并行
- **Polish (Phase 6)**: 依赖所有目标 user story 完成

### User Story Dependencies

- **US1 (P1)**: Foundational 后即可开始——无其他 story 依赖（T013-T023 工具实现是 US2/US3 的前提，故 US1 最先）
- **US2 (P2)**: 依赖 US1 的 Read/Edit/Write 工具骨架（T014/T017/T018）——实现守卫需工具已存在
- **US3 (P3)**: 依赖 US1 的 Grep/Glob/Skill/ResetTools 工具骨架（T015/T016/T019/T021）——完善搜索界限与激活交互

### Within Each User Story

- Tests（含）MUST 先写并 FAIL 后实现
- 工具骨架 → 守卫/界限/激活完善 → Trace 集成
- Story 完成后进入下一优先级

### Parallel Opportunities

- Setup 阶段 T002/T003 [P] 可并行
- Foundational T007/T008 [P] 可并行
- US1 各工具实现 T013-T020 [P] 全部可并行（不同文件，无相互依赖）
- US2 测试 T024-T026 [P] 可并行
- US3 测试 T032-T035 [P] 可并行
- Polish T041-T043/T045 [P] 可并行

---

## Parallel Example: User Story 1

```bash
# 并行实现所有内置工具（不同文件，互不依赖）:
Task: "实现 Bash 在 crates/agent_scope_tool/src/builtin/bash.rs"        # T013
Task: "实现 Read 在 crates/agent_scope_tool/src/builtin/read.rs"        # T014
Task: "实现 Grep 在 crates/agent_scope_tool/src/builtin/grep.rs"        # T015
Task: "实现 Glob 在 crates/agent_scope_tool/src/builtin/glob.rs"        # T016
Task: "实现 Edit 骨架在 crates/agent_scope_tool/src/builtin/edit.rs"    # T017
Task: "实现 Write 骨架在 crates/agent_scope_tool/src/builtin/write.rs"  # T018
Task: "实现 Skill 在 crates/agent_scope_tool/src/builtin/skill.rs"      # T019
Task: "实现 PowerShell 在 crates/agent_scope_tool/src/builtin/powershell.rs" # T020
```

## Parallel Example: User Story 2 Tests

```bash
# 并行启动 US2 所有测试（先写先 fail）:
Task: "未读先 Edit/Write 拒绝测试在 crates/agent_scope_tool/tests/builtin_edit_write_tests.rs"  # T024
Task: "Edit 唯一/非唯一测试在 crates/agent_scope_tool/tests/builtin_edit_write_tests.rs"        # T025
Task: "read-before-modify 全流程集成测试在 crates/agent_scope_tool/tests/builtin_edit_write_tests.rs" # T026
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 Setup → Phase 2 Foundational（阻塞所有 story）
2. Phase 3: US1——完成全部内置工具骨架 + 注入
3. **STOP and VALIDATE**: US1 独立测试（注入集合 schema、未启用者无工具）
4. 可 demo：workspace 启用的 agent 自动获得全部基础工具

### Incremental Delivery

1. Setup + Foundational → Foundation ready（`WorkspaceToolSession`、`ToolError::UnsupportedCapability`、激活组过滤、注入点）
2. +US1 → 注入全部内置工具（MVP）
3. +US2 → read-before-modify 守卫（安全文件修改）
4. +US3 → 搜索界限/激活交互/trace（高效搜索）
5. 每 story 独立可测，不破坏前序 story

### Parallel Team Strategy

多开发者时：
1. 团队完成 Setup + Foundational
2. Foundational 后：开发者 A 做 US1 各工具、开发者 B 待 US1 骨架就绪做 US2、开发者 C 做 US3
3. story 独立完成并整合

---

## Notes

- [P] tasks = 不同文件、无依赖
- [Story] label 映射到具体 user story（traceability）
- 每 user story 独立可完成、可测试
- 测试先写先 fail，再实现
- 每任务或逻辑组提交
- 任意 checkpoint 可停下独立验证 story
- 避免：模糊任务、同文件冲突、跨 story 依赖破坏独立性
- 对齐 vendored Python 参考实现 `agentscope/src/agentscope/tool/_builtin/`（上游 `9d1026fa`）作为兼容基准（宪法 Art.1/Art.3）
