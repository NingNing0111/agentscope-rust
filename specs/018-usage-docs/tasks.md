---

description: "Task list for Feature 018 usage docs implementation"
---

# Tasks: AgentScope Rust 模块化使用文档

**Input**: Design documents from `/specs/018-usage-docs/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (documentation-layout.md, module-doc-template.md), quickstart.md

**Tests**: 本特性为文档特性，无代码测试任务；验证通过 quickstart.md 的 6 个验证场景执行（嵌于各故事 Checkpoint 与 Polish 阶段）。

**Organization**: 任务按用户故事分组，US2 的 12 个模块文档任务全部可并行。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- 文档交付物: `docs/`（仓库根）
- 撰写辅助（内部参考，非交付物）: `specs/018-usage-docs/authoring-notes.md`
- 示例锚点（只读引用）: `examples/*.rs`
- 兼容性权威源（只读引用）: `specs/001-compatibility-baseline/capability-matrix.json`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 文档站点骨架与既有内容基线

- [X] T001 创建文档目录骨架：`docs/zh/modules/`、`docs/zh/tutorials/`、`docs/en/modules/`、`docs/en/tutorials/`（契约 documentation-layout.md §1 布局）
- [X] T002 [P] 记录 `docs/superpowers/` 既有内容基线（文件清单）至 `specs/018-usage-docs/authoring-notes.md` 首节，作为交付时"未破坏既有内容"（FR-001）的对照依据

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 撰写所有文档前必须固化的已核实事实库（authoring-notes.md），防止凭记忆杜撰（契约 E-2、FR-011）

**⚠️ CRITICAL**: 事实库未完成前，任何文档撰写不得开始

- [X] T003 在 `specs/018-usage-docs/authoring-notes.md` 中建立示例锚点清单：通读 `examples/chat.rs`、`examples/common.rs`、`examples/memory_test.rs`、`examples/rag_test.rs`、`examples/session_test.rs`、`examples/streaming_tool_test.rs`、`examples/verify_agent.rs`，记录每个文件的关键行区间与用途（如 chat.rs 流式事件循环、common.rs `create_model` 构造、rag_test.rs 检索管线），供文档示例引用（契约 E-1）
- [X] T004 向 `specs/018-usage-docs/authoring-notes.md` 追加已核实的配置事实：环境变量 `API_KEY`、`.env` + `dotenv` 加载方式（`examples/chat.rs:388`）、clap `env` feature 注入、`DashScopeChatModel::new(api_key, model_name)` 与 `DashScopeEmbedding::new(api_key, model_card)` 签名、API key 为空的错误行为（对照 `crates/agent_scope_dashscope/src/` 源码逐条核实，research.md D7）
- [X] T005 向 `specs/018-usage-docs/authoring-notes.md` 追加上游版本锁定信息（从 `specs/001-compatibility-baseline/` 基线数据提取 release version + commit hash）与兼容性矩阵现状说明（280 条目 `status` 全为 `NOT_ANALYZED` 的陈旧状态及文档侧应对策略，research.md D4）

**Checkpoint**: authoring-notes.md 事实库就绪 — 文档撰写可以开始

---

## Phase 3: User Story 1 - 新用户快速上手 (Priority: P1) 🎯 MVP

**Goal**: 新用户仅凭快速上手指南，30 分钟内从零跑通第一个流式对话 Agent，并能经索引导航到模块文档

**Independent Test**: 按 `docs/zh/getting-started.md` 步骤执行 `cargo run --example chat` 成功对话；未配置 `API_KEY` 时报错与文档描述一致（quickstart.md 场景 1）

### Implementation for User Story 1

- [X] T006 [US1] 撰写 `docs/zh/getting-started.md`：环境准备（Rust 工具链）、依赖引入（根 facade `agentscope` crate）、凭据配置（`API_KEY` / `.env` / dotenv，事实取自 authoring-notes.md T004 节）、创建并运行第一个流式对话 Agent（锚定 `examples/chat.rs` + `examples/common.rs`，标注来源行区间）、常见错误排查（缺 key、网络、模型名错误）、下一步导航（链接 modules/ 与 migration.md）；结构遵循 contracts/module-doc-template.md 精神但以上手流程为主线
- [X] T007 [US1] 撰写 `docs/en/getting-started.md`：与 T006 镜像——标题数量/顺序一致、代码块完全相同、信息等价（契约 B-1/B-2）
- [X] T008 [US1] 撰写 `docs/README.md` 双语总索引（中英并列）：项目简介、上游版本锁定信息（取自 authoring-notes.md T005 节）、推荐阅读顺序、`zh/` 与 `en/` 入口链接、12 个模块文档与迁移参考、教程的规划结构链接、"规划中"章节声明 Multi-agent / Distributed runtime 未实现（契约 X-3）
- [X] T009 [US1] 验证 US1（quickstart.md 场景 1）：严格按 `docs/zh/getting-started.md` 步骤执行至 `cargo run --example chat` 跑通；临时移除 `API_KEY` 验证报错描述准确；核对双语 getting-started 标题序列与代码块一致；修正发现的问题

**Checkpoint**: US1 独立可交付 — 新用户可上手，MVP 达成

---

## Phase 4: User Story 2 - 按模块查阅使用文档 (Priority: P2)

**Goal**: 12 个能力模块各有双语使用文档，开发者不读源码即可理解模块概念、复制可运行示例、知晓错误类型与不支持的能力

**Independent Test**: 任选一篇模块文档（如 tool.md），仅依据该文档注册自定义工具并被 Agent 成功调用；每篇含 ≥1 可运行示例与兼容性章节（quickstart.md 场景 2/4/5）

**撰写通用要求**（适用于 T010-T021 每个任务）:
- 结构 MUST 遵循 `specs/018-usage-docs/contracts/module-doc-template.md` 7 章节契约
- 兼容性章节 MUST 遵循 `specs/018-usage-docs/contracts/documentation-layout.md` §5 格式，等级/偏差对照 `specs/001-compatibility-baseline/capability-matrix.json`（按 `category` 过滤相关条目）与各 feature spec 交叉核实
- 示例 MUST 锚定 `examples/` 真实代码并标注 `<!-- source: ... -->`；配置项对照源码核实（authoring-notes.md）
- 双语双文件同任务交付：`docs/zh/modules/<name>.md` + `docs/en/modules/<name>.md`，标题序列一致、代码块相同

### Implementation for User Story 2

- [X] T010 [P] [US2] 撰写 message-types 模块文档（`docs/zh/modules/message-types.md` + `docs/en/modules/message-types.md`）：覆盖 `agent_scope_types` + `agent_scope_message`——Message/ContentBlock 结构、序列化协议与未知字段处理的用户可见语义（宪法第十二条）、角色与块类型清单
- [X] T011 [P] [US2] 撰写 event-streaming 模块文档（`docs/zh/modules/event-streaming.md` + `docs/en/modules/event-streaming.md`）：覆盖 `agent_scope_event` + 流式语义——AgentEvent/Block 类型、事件发布顺序、流式 chunk 累积与 EndEvent 携带完整内容（Feature 014）、取消行为（CancellationToken）、trace 语义（宪法第七条）
- [X] T012 [P] [US2] 撰写 model 模块文档（`docs/zh/modules/model.md` + `docs/en/modules/model.md`）：覆盖 `agent_scope_model`——ChatModel trait、ChatResponse、StreamAccumulator、流式/非流式调用、超时与重试、自定义 Provider 接入方式（`Arc<dyn ChatModel>`）
- [X] T013 [P] [US2] 撰写 dashscope 模块文档（`docs/zh/modules/dashscope.md` + `docs/en/modules/dashscope.md`）：覆盖 `agent_scope_dashscope`——凭据配置（authoring-notes.md T004 事实）、Chat/Embedding 用法、模型名与参数、Provider 错误分类
- [X] T014 [P] [US2] 撰写 tool 模块文档（`docs/zh/modules/tool.md` + `docs/en/modules/tool.md`）：覆盖 `agent_scope_tool`——Tool trait、ToolCall/ToolResult 生命周期、参数 schema（schemars）、注册自定义工具、工具错误类型；示例锚定 `examples/streaming_tool_test.rs` 与 `examples/chat.rs` 的 calculator 工具
- [X] T015 [P] [US2] 撰写 agent 模块文档（`docs/zh/modules/agent.md` + `docs/en/modules/agent.md`）：覆盖 `agent_scope_agent`——ReActAgent、reasoning-acting 循环、中间件钩子（pre_reply/post_reply/pre_acting/post_acting）、事件流消费、中断与取消；示例锚定 `examples/verify_agent.rs` 与 `examples/chat.rs`
- [X] T016 [P] [US2] 撰写 memory 模块文档（`docs/zh/modules/memory.md` + `docs/en/modules/memory.md`）：覆盖 `agent_scope_memory`——记忆类型、读写语义、索引截断中间件行为；示例锚定 `examples/memory_test.rs`
- [ ] T017 [P] [US2] 撰写 session 模块文档（`docs/zh/modules/session.md` + `docs/en/modules/session.md`）：覆盖 session 管理 + `agent_scope_state`（状态层并入）——会话生命周期、持久化、上下文修剪、会话隔离；示例锚定 `examples/session_test.rs`
- [ ] T018 [P] [US2] 撰写 rag 模块文档（`docs/zh/modules/rag.md` + `docs/en/modules/rag.md`）：覆盖 `agent_scope_embedding` + `agent_scope_rag`（含 turbovec 本地向量库）——Embedding、Parser/Chunker、VectorStore、KnowledgeBase + RAGMiddleware 完整链路；示例锚定 `examples/rag_test.rs`
- [ ] T019 [P] [US2] 撰写 workspace 模块文档（`docs/zh/modules/workspace.md` + `docs/en/modules/workspace.md`）：覆盖 `agent_scope_workspace`——工作空间管理、存储布局、与 Skill 的关系
- [ ] T020 [P] [US2] 撰写 skill 模块文档（`docs/zh/modules/skill.md` + `docs/en/modules/skill.md`）：覆盖 Skill 工具集成——技能定义、加载、在 Agent 中作为工具使用
- [ ] T021 [P] [US2] 撰写 sandbox 模块文档（`docs/zh/modules/sandbox.md` + `docs/en/modules/sandbox.md`）：覆盖 `agent_scope_sandbox`——local-process 沙箱、L2 兼容目标、硬隔离明确 unsupported 及 UnsupportedFeature 错误（宪法第五条重点篇目）
- [ ] T022 [US2] 验证 US2（quickstart.md 场景 2/4/5）：`cargo build --examples && cargo test --examples` 全绿；逐篇核对 7 章节结构齐全；抽查 3 处内联示例与 source 标注一致；逐篇第 6 章与 capability-matrix.json 交叉核对零冲突；抽查配置项与源码一致；修正发现的问题

**Checkpoint**: US2 独立可交付 — 12 篇模块文档齐备

---

## Phase 5: User Story 3 - Python AgentScope 用户迁移参考 (Priority: P2)

**Goal**: Python 用户依据迁移参考完成 API 对照迁移，理解行为差异与各模块 L1-L4 兼容等级，不读 Rust 源码

**Independent Test**: 依据 `docs/zh/migration.md` 将 Python ReActAgent + 工具调用应用改写为等价 Rust 应用（参照 `examples/verify_agent.rs`），差异说明与实际一致（quickstart.md 场景 6）

### Implementation for User Story 3

- [ ] T023 [US3] 撰写迁移参考（`docs/zh/migration.md` + `docs/en/migration.md`）：上游版本锁定信息（authoring-notes.md T005 节）；Python→Rust 主要公开 API 对照表（Message/ContentBlock、ReActAgent、Tool、ChatModel、Memory、Session、RAG、流式事件，名称/所在 crate/用法差异，基于 capability-matrix.json 的 `upstream_symbol` ↔ 实际 Rust API 交叉整理）；各模块 L1-L4 等级汇总表；已知兼容性偏差清单（与矩阵 `notes` 一致，含矩阵 status 陈旧状态如实说明）；Rust 惯用差异提示（trait object、Arc、Result、CancellationToken，宪法第八条）
- [ ] T024 [US3] 验证 US3（quickstart.md 场景 6）：以迁移参考为唯一依据完成一次 Python ReActAgent + 工具调用 → Rust 的迁移走查（对照 `examples/verify_agent.rs`）；核对偏差清单与矩阵一致；核对双语标题序列与表格内容一致；修正发现的问题

**Checkpoint**: US3 独立可交付 — 迁移路径闭环

---

## Phase 6: User Story 4 - 端到端场景教程 (Priority: P3)

**Goal**: 用户按教程构建一个完整的 RAG 知识库问答应用，串联 rag + agent + memory + session 模块

**Independent Test**: 按 `docs/zh/tutorials/rag-knowledge-chat.md` 完成全部步骤，产物可编译运行并行为符合教程描述（quickstart.md 场景 1/2 变体）

### Implementation for User Story 4

- [ ] T025 [US4] 撰写场景教程（`docs/zh/tutorials/rag-knowledge-chat.md` + `docs/en/tutorials/rag-knowledge-chat.md`）：场景目标与架构串联说明；前置条件（`API_KEY` 凭据、示例知识库数据准备、真实模型调用成本说明）；分步构建（知识库构建 → 检索接入 → Agent 集成 → 会话记忆）；完整产物锚定 `examples/rag_test.rs` + `examples/chat.rs`（契约 E-1）；前置阅读与相关模块链接（rag.md、agent.md、memory.md、session.md）
- [ ] T026 [US4] 验证 US4：按教程步骤执行，`cargo run --example rag_test` 行为与教程描述一致；核对双语标题序列与代码块一致；核对前置条件说明完整（凭据/数据/成本）；修正发现的问题

**Checkpoint**: US4 独立可交付 — 场景教程可用

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 全站一致性、完整性与最终验收（宪法第十七条适配版 DoD）

- [ ] T027 定稿 `docs/README.md`：补齐全部已交付文档的实际链接（12 模块 × 2 语种 + migration + tutorial），全站链接完整性清扫（无悬空链接，契约 K-1/K-2/K-3，SC-002）
- [ ] T028 [P] 执行双语镜像检查（quickstart.md 场景 3）：`diff <(cd docs/zh && find . -type f | sort) <(cd docs/en && find . -type f | sort)` 为空；全部对应文件标题序列一致、代码块相同（契约 B-1/B-2，SC-008）
- [ ] T029 [P] 执行兼容性零冲突复查（quickstart.md 场景 4）：全部模块文档第 6 章与 `specs/001-compatibility-baseline/capability-matrix.json` 最终核对，文档宣称的能力无矩阵非 IMPLEMENTED 条目冲突（SC-005）
- [ ] T030 [P] 执行配置项最终抽查（quickstart.md 场景 5）：各文档环境变量名、构造参数、默认值与源码 100% 一致（SC-007）
- [ ] T031 执行完整验收：quickstart.md 场景 1-6 全量通过；`git status` 确认 `docs/superpowers/` 相对 T002 基线无变化（FR-001）；对照 spec.md SC-001~SC-008 逐项确认；更新 `specs/018-usage-docs/checklists/requirements.md` 交付状态

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖，立即开始
- **Foundational (Phase 2)**: 依赖 T001（authoring-notes.md 路径就绪）；**阻塞所有用户故事**
- **User Stories (Phase 3-6)**: 均依赖 Phase 2 完成；US1/US2/US3/US4 之间无内容依赖，可并行或按优先级顺序执行
- **Polish (Phase 7)**: 依赖全部目标用户故事完成（T027 依赖全部文档落地）

### User Story Dependencies

- **US1 (P1)**: Phase 2 后可开始，无其他故事依赖
- **US2 (P2)**: Phase 2 后可开始，无其他故事依赖（模块文档互链使用契约规定的相对路径，目标文件由本故事内任务交付）
- **US3 (P2)**: Phase 2 后可开始；引用模块等级信息时可复用 US2 已核实的兼容性结论，但不阻塞
- **US4 (P3)**: Phase 2 后可开始；内容串联 US2 模块主题，建议 US2 完成后执行以获得准确互链

### Within Each User Story

- 先写 zh 版事实内容，再产出 en 镜像（T006→T007 模式）
- 每个故事以验证任务收尾，验证不通过则回到对应撰写任务修正
- 故事完成并通过 Checkpoint 后再进入下一优先级

### Parallel Opportunities

- T001 与 T002 可并行
- Phase 2 完成后，US1/US2/US3 可三路并行
- US2 内 T010-T021（12 个模块任务）全部可并行（不同文件）
- Polish 阶段 T028/T029/T030 可并行

---

## Parallel Example: User Story 2

```bash
# 12 个模块文档任务可同时启动（每个任务交付双语双文件，文件互不重叠）：
Task: "撰写 message-types 模块文档 (T010)"
Task: "撰写 event-streaming 模块文档 (T011)"
Task: "撰写 model 模块文档 (T012)"
Task: "撰写 dashscope 模块文档 (T013)"
Task: "撰写 tool 模块文档 (T014)"
Task: "撰写 agent 模块文档 (T015)"
# ... T016-T021 同理
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. 完成 Phase 1: Setup（T001-T002）
2. 完成 Phase 2: Foundational（T003-T005，事实库）
3. 完成 Phase 3: US1（T006-T009）
4. **STOP and VALIDATE**: quickstart.md 场景 1 通过 → 可交付的 MVP（快速上手 + 索引）

### Incremental Delivery

1. Setup + Foundational → 事实库就绪
2. + US1 → 验证场景 1 → MVP 交付
3. + US2 → 验证场景 2/4/5 → 模块文档全集交付
4. + US3 → 验证场景 6 → 迁移参考交付
5. + US4 → 教程验证 → 完整文档站点
6. Polish → 场景 1-6 全量验收

### Parallel Team Strategy

1. 共同完成 Phase 1 + Phase 2
2. Phase 2 完成后：作者 A 负责 US1 + US3（叙事类文档），作者 B/C/D 瓜分 US2 的 12 个模块任务，之后汇合执行 US4 与 Polish

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- 每个模块文档任务（T010-T021）交付双语双文件，撰写通用要求见 Phase 4 头部
- 撰写时禁止凭记忆填写 API/配置/兼容性信息——一切以 authoring-notes.md 与源码核实为准（契约 E-2）
- authoring-notes.md 为内部撰写参考，非交付物，无需双语
- Stop at any checkpoint to validate story independently
