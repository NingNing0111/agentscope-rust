---

description: "Agent 状态持久化（内置 JSON 文件存储 + 可插拔存储后端）实现任务清单"
---

# Tasks: Agent 状态持久化（内置 JSON 文件存储 + 可插拔存储后端）

**Input**: Design documents from `/specs/025-agent-state-persistence/`

**Prerequisites**: plan.md（必需）、spec.md（必需，3 个 user story）、research.md、data-model.md、contracts/

**Tests**: 本特性通过 quickstart.md 场景驱动（场景 1-8），测试任务为验收核心手段（宪法第六条），**先写测试、确认 FAIL，再实现**。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Path Conventions

- 单 workspace 多 crate：`crates/agent_scope_state/`（存储抽象与数据模型）、`crates/agent_scope_agent/`（ReActAgent 接入）
- 文档：`docs/zh/modules/agent.md`、`docs/en/modules/agent.md`
- 示例：`examples/pi-rust/`

---

## Phase 1: Setup（共享基础设施）

**Purpose**: 确认依赖与测试骨架就绪，无需项目初始化（workspace 已存在）

- [X] T001 确认 `crates/agent_scope_state/Cargo.toml` 已具备 serde/serde_json/chrono/uuid/tokio/async-trait 运行时依赖；为该 crate 添加 `tempfile = "3"` 到 `[dev-dependencies]`（对齐 `agent_scope_agent` 等 crate 先例，测试用临时目录）
- [X] T002 [P] 创建集成测试骨架：`crates/agent_scope_state/tests/json_file_store_tests.rs`（模块头注释 + 空 `#[tokio::test]` 占位），确认可编译
- [X] T003 [P] 创建集成测试骨架：`crates/agent_scope_agent/tests/agent_persistence_tests.rs`（模块头注释 + 复用 `tests/mocks.rs` 的 MockModel/ScriptedModel 的占位测试），确认可编译

**Checkpoint**: 两个测试骨架编译通过，后续任务可并行填充。

---

## Phase 2: Foundational（阻塞性前置）

**Purpose**: 存储文件格式外壳、标识校验、原子写入——US1/US2/US3 全部依赖的核心基础设施

**⚠️ CRITICAL**: 所有 user story 的存储实现都依赖本阶段，必须先完成

- [X] T004 在 `crates/agent_scope_state/src/json_file_store.rs` 定义 `SessionRecordFile` 序列化结构：`session_id`/`status: SessionStatus`/`message_count: usize`/`created_at: DateTime<Utc>`/`last_active: DateTime<Utc>`/`state: AgentState`，对齐 data-model.md §2 与 contracts/json-file-format.md；全字段 `#[serde(default)]`（宪法第十二条，旧版本文件缺字段兼容加载）
- [X] T005 在 `crates/agent_scope_state/src/json_file_store.rs` 实现 `validate_session_id(id: &str) -> Result<(), SessionError>`：拒绝路径分隔符（`/`、`\`）、`.`、空串等非法文件名，返回 `SessionError::StorageError { reason }`（防路径穿越，spec Edge Case / contracts/json-file-format.md §会话标识校验）
- [X] T006 在 `crates/agent_scope_state/src/json_file_store.rs` 实现 `atomic_write(dir: &Path, id: &str, contents: &[u8]) -> Result<(), SessionError>`：写 `{id}.json.tmp` → fsync → rename 为 `{id}.json` → 成功后清理临时文件（崩溃不留半写文件，spec FR-004；I/O 失败映射 `SessionError::StorageError` 保留根因）
- [X] T007 [P] 在 `crates/agent_scope_state/src/lib.rs` 注册 `pub mod json_file_store;` 并 re-export `JsonFileSessionStore`（待 US1 实现后生效，先注册模块）

**Checkpoint**: 文件格式外壳与安全工具就绪——US1 的 JsonFileSessionStore 实现可开始。

---

## Phase 3: User Story 1 - Agent 自动持久化与会话恢复 (Priority: P1) 🎯 MVP

**Goal**: 内置 `JsonFileSessionStore` 实现 + ReActAgent 通过 `AgentConfig` 接入——每次 reply 结束自动落盘，按 `session_id` 跨进程恢复完整历史，行为对开发者透明。

**Independent Test**: 用默认配置构建 Agent 完成多轮对话，确认每次回复后状态已落盘；用同一 `session_id` 重新构建 Agent，验证完整历史恢复且可基于历史继续作答（quickstart 场景 1/2/3/4/6）。

### Tests for User Story 1（quickstart 场景 1-4、6，先写后实现）⚠️

> **NOTE**: 这些测试在实现前编写并确认 FAIL

- [X] T008 [P] [US1] `agent_scope_state` 集成测试：`JsonFileSessionStore` 保存/加载往返无损（含 context/summary/tasks/middle_context 全字段），断言 `{dir}/{session_id}.json` 存在，文件结构符合 json-file-format.md——quickstart 场景 1，在 `crates/agent_scope_state/tests/json_file_store_tests.rs`
- [X] T009 [P] [US1] `agent_scope_state` 集成测试：原子写入（tmp 文件不残留）、损坏 JSON 文件返回 `SessionError::SerializationError`、非法标识（含 `/`、`.`）被拒且未越界建文件——quickstart 场景 6，在 `crates/agent_scope_state/tests/json_file_store_tests.rs`
- [X] T010 [P] [US1] `agent_scope_agent` 集成测试：跨进程重启恢复——ScriptedModel 构建 Agent（`session_id="s-1"`、默认 store）完成一轮 reply，丢弃 Agent 实例后同一 `session_id` 重建，断言 `try_state().context` 含历史且第二轮基于完整历史作答——quickstart 场景 2，在 `crates/agent_scope_agent/tests/agent_persistence_tests.rs`
- [X] T011 [P] [US1] `agent_scope_agent` 集成测试：自动落盘——reply 结束后 `store.load(session_id)` 返回含该轮上下文与工具结果的状态——quickstart 场景 3，在 `crates/agent_scope_agent/tests/agent_persistence_tests.rs`
- [X] T012 [P] [US1] `agent_scope_agent` 集成测试：中断/取消时保存——对齐 Feature 008 既有取消测试模式，取消后 `store.load` 返回中断前已产生的最新消息且状态一致——quickstart 场景 4，在 `crates/agent_scope_agent/tests/agent_persistence_tests.rs`

### Implementation for User Story 1

- [X] T013 [US1] 实现 `JsonFileSessionStore` 于 `crates/agent_scope_state/src/json_file_store.rs`：字段 `dir: PathBuf`；`new(dir)` 指定目录、`Default`/`with_default_dir()` 用工作区 `sessions/`；构造时创建目录（失败返回 `StorageError`）；`save` 用 `validate_session_id` + `atomic_write` 写 `SessionRecordFile`（依赖 T004/T005/T006）；`load` 读文件→反序列化→组装 `SessionImpl`，缺失返回 `NotFound`，解析失败返回 `SerializationError`，`state.session_id` 以文件名为准
- [X] T014 [US1] 在 `crates/agent_scope_agent/src/config.rs` 为 `AgentConfig` 新增三个可选字段：`session_store: Option<Arc<dyn SessionStore>>`、`session_id: Option<String>`、`auto_persist: bool`（默认 `true`）；`AgentConfigBuilder` 新增 `session_store()`/`session_id()`/`auto_persist()` 方法；既有字段与调用完全不变（向后兼容，spec Assumption）；`build()` 校验不变
- [X] T015 [US1] 在 `crates/agent_scope_agent/src/react_agent.rs` 的 `ReActAgent::new` 增加构建期恢复逻辑：解析 `session_id`（`Some` → `store.load(id)`：`Ok` 恢复既有 `AgentState` / `Err(NotFound)` 用该 id 新建 / `Err(其他)` 返回 `AgentError` 构建失败）；`store` 为 `None` 时内部创建默认 `JsonFileSessionStore::with_default_dir()`；`None` session_id 用 `AgentState::new()` 生成新 ID（spec FR-005 / contracts/agent-config.md）
- [X] T016 [US1] 在 `crates/agent_scope_agent/src/react_loop.rs` 为 batch 路径 `do_reply` 增加回复结束后保存点：reply 正常结束时 `session_store.save(&session)`；`auto_persist=false` 跳过（零写入）；保存失败经 `AgentError` 上报但不破坏已完成的回复结果（spec FR-006 / contracts/agent-config.md）
- [X] T017 [US1] 在 `crates/agent_scope_agent/src/streaming_reactor.rs` 为 streaming 路径 `reply_stream` 增加结束/中断时保存点：正常结束与被中断/取消时保存中断时刻最新状态；`auto_persist=false` 跳过；保存失败上报不阻断事件流（spec FR-006 / quickstart 场景 4）
- [X] T018 [US1] 构建期加载失败（损坏/IO）时的 `AgentError` 错误映射：确认 `AgentError` 有承载 `SessionError` 根因的变体（复用 `ValidationError` 或新增 `SessionError { source }`，对齐宪法第十三条 typed 错误），不静默降级

**Checkpoint**: US1 完整可用——默认持久化开箱即用，会话可自动落盘并跨进程恢复。此即 MVP。

---

## Phase 4: User Story 2 - 自定义存储后端 (Priority: P2)

**Goal**: `SessionStore` trait 作为唯一扩展点，开发者实现该接口即可接入自有后端（SQLite/MySQL/Redis），行为与内置 JSON 文件后端一致，零框架改动。

**Independent Test**: 实现最小自定义 `SessionStore` 后端并用它构建 Agent 完成保存/恢复往返，验证与内置后端行为一致；文档给出 SQLite/MySQL 实现契约说明（quickstart 场景 7）。

### Tests for User Story 2（quickstart 场景 7，先写后实现）⚠️

- [X] T019 [P] [US2] `agent_scope_agent` 集成测试：内联最小自定义 `SessionStore` 实现（基于内存 HashMap + 简单路径，模拟 SQLite 语义），`AgentConfig::builder().session_store(Arc::new(custom)).session_id("c-1")` 构建 Agent 完成保存/恢复往返，断言与内置后端行为一致（保存/加载/删除/列表）、加载不存在会话返回明确"未找到"——quickstart 场景 7，在 `crates/agent_scope_agent/tests/agent_persistence_tests.rs`

### Implementation for User Story 2

- [X] T020 [US2] 编写"实现自定义后端"文档章节于 `docs/zh/modules/agent.md`：`SessionStore` trait 语义契约（upsert/NotFound/幂等删除/轻量 list_meta）+ 以 SQLite（`INSERT ... ON CONFLICT DO UPDATE`）、MySQL（`INSERT ... ON DUPLICATE KEY UPDATE`）为例的实现说明，无需反读框架源码（spec FR-014 / US2 Independent Test）

**Checkpoint**: 自定义后端可无缝接入，扩展点语义有文档背书。

---

## Phase 5: User Story 3 - 会话管理 (Priority: P3)

**Goal**: 对持久化会话执行列表、查询、删除，元数据轻量（不加载完整状态）、删除幂等，语义与 Python 参考实现一致。

**Independent Test**: 对多个已持久化会话执行 list/delete，验证元数据正确、列表不反序列化完整状态、删除不存在会话不报错（quickstart 场景 8）。

### Tests for User Story 3（quickstart 场景 5、8，先写后实现）⚠️

- [X] T021 [P] [US3] `agent_scope_state` 集成测试：`list_ids` 返回全部 ID、`list_meta` 按 `last_active` 降序且不加载完整状态、`delete` 幂等、删除后 `load` 返回 `NotFound`——quickstart 场景 8，在 `crates/agent_scope_state/tests/json_file_store_tests.rs`
- [X] T022 [P] [US3] `agent_scope_agent` 集成测试：`auto_persist=false` 时完成多轮 reply 后目录内 0 个会话文件（零磁盘写入）——quickstart 场景 5，在 `crates/agent_scope_agent/tests/agent_persistence_tests.rs`

### Implementation for User Story 3

- [X] T023 [US3] 在 `crates/agent_scope_state/src/json_file_store.rs` 实现 `list_meta` 轻量读取：只解析每个文件的 `SessionRecordFile` 外层元数据字段（不反序列化完整 `AgentState`），按 `last_active` 降序返回 `Vec<SessionMeta>`（spec FR-010 / contracts/session-store.md §4）；`delete` 幂等（文件不存在返回 `Ok`）；`list_ids` 扫描目录 `*.json` 文件名

**Checkpoint**: 会话管理语义完整——列表轻量、删除幂等、关闭持久化零写入。

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: 文档、示例、兼容性矩阵与完整验收（影响多个 user story）

- [X] T024 [P] 编写"开箱即用与恢复"文档章节于 `docs/zh/modules/agent.md`：默认 JSON 文件存储用法、按 `session_id` 恢复、`auto_persist` 开关、保存失败上报语义（spec FR-014）
- [X] T025 [P] 同步编写同章节于 `docs/en/modules/agent.md`（与 zh 保持一致）
- [X] T026 [P] 更新兼容性矩阵（`docs/zh/modules/agent.md` 及 en 对应位置）：存储模块标记 **L2**（核心行为兼容——会话保存/恢复语义对齐 Python `StorageBase`）+ **L3**（公开 API 语义兼容——`JsonFileSessionStore`、`SessionStore` 扩展点、`AgentConfig` 新增配置）；L1 字节级协议不在范围（plan.md Constitution Check 第十八条）
- [X] T027 示例接入 `examples/pi-rust`：展示持久化/恢复用法（可选，若接入则复用默认 `sessions/` 目录或显式 `auto_persist(false)` 说明落盘副作用）
- [X] T028 运行 quickstart.md 完整验收（宪法第十七条）：`cargo test --workspace`、`cargo clippy --workspace -- -D warnings`、`cargo fmt --all -- --check` 全部通过；确认无 `unsafe`、无 unwrap/expect/panic 新增（宪法第九条）

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖，可立即开始
- **Foundational (Phase 2)**: 依赖 Setup 完成——**BLOCKS 所有 user story**
- **User Stories (Phase 3+)**: 依赖 Foundational 完成
  - US1 (Phase 3) 可先行；US2 (Phase 4) 与 US3 (Phase 5) 依赖 US1 的 `JsonFileSessionStore`/`AgentConfig` 接入落地后独立推进
- **Polish (Phase 6)**: 依赖 US1/US2/US3 完成

### User Story Dependencies

- **US1 (P1)**: 依赖 Phase 2（T004-T007）；无其他 story 依赖——MVP
- **US2 (P2)**: 依赖 US1 的 T014（`AgentConfig.session_store` 注入）——自定义后端通过注入接入
- **US3 (P3)**: 依赖 US1 的 T013（`JsonFileSessionStore` 完整实现，含 list/delete 基础语义）——会话管理在其上完善轻量语义

### Within Each User Story

- 测试（T008-T012、T019、T021-T022）必须先写并确认 FAIL 再实现（宪法第六条）
- 存储后端实现（T013）→ AgentConfig 接入（T014）→ 恢复逻辑（T015）→ 保存点（T016/T017）
- Story 完成后再进入下一优先级

### Parallel Opportunities

- T002/T003（Setup）并行；T008-T012（US1 测试）并行；T013-T018 顺序依赖
- US2 与 US3 测试（T019、T021-T022）可并行
- T024/T025/T026 文档可并行

---

## Parallel Example: User Story 1

```bash
# Launch all US1 tests together（先 FAIL 后实现）:
Task: "T008 往返无损测试"（json_file_store_tests.rs）
Task: "T009 原子/损坏/非法标识测试"（json_file_store_tests.rs）
Task: "T010 跨进程恢复测试"（agent_persistence_tests.rs）
Task: "T011 自动落盘测试"（agent_persistence_tests.rs）
Task: "T012 中断保存测试"（agent_persistence_tests.rs）

# 测试确认 FAIL 后，顺序实现:
Task: "T013 JsonFileSessionStore 实现" → "T014 AgentConfig 字段" → "T015 构建期恢复" → "T016/T017 保存点"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. 完成 Phase 1: Setup
2. 完成 Phase 2: Foundational（T004-T007，BLOCKS 所有 story）
3. 完成 Phase 3: User Story 1（T008-T018）
4. **STOP and VALIDATE**: 运行 quickstart 场景 1/2/3/4/6，US1 独立可测
5. 若需演示，可先交付（默认持久化开箱即用 + 会话恢复即核心价值）

### Incremental Delivery

1. Setup + Foundational → 存储基础就绪
2. US1 → 自动持久化 + 恢复可用（MVP）→ 验证
3. US2 → 自定义后端扩展点 + 文档契约 → 验证
4. US3 → 会话管理轻量列表/幂等删除 → 验证
5. 每 story 独立交付，不破坏既有行为（宪法第十六条）

### Parallel Team Strategy

1. Team 完成 Setup + Foundational
2. Foundational 完成后：
   - Developer A: US1（测试先行 + 实现 + 保存点）
   - Developer B: US2 文档契约（依赖 US1 的 config 接入确认后）
   - Developer C: US3 轻量 list_meta / 幂等 delete 测试
3. Story 独立集成，Polish 阶段合并文档与验收

---

## Notes

- [P] 任务 = 不同文件、无依赖
- [Story] 标签映射 user story 以追溯（US1/US2/US3）
- 每个 user story 独立可完成、可测试
- 测试先写并确认 FAIL 再实现
- 每个任务或逻辑组提交一次
- 任一 checkpoint 可停下独立验证 story
- 避免：模糊任务、同文件冲突、破坏 story 独立性的跨 story 依赖
- `agent_scope_state` 的 `tempfile` 仅作 dev-dependency，不增加运行时依赖（对齐 crate 先例）
