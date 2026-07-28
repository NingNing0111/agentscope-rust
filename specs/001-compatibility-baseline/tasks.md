# Tasks: AgentScope Compatibility Baseline

**Input**: Design documents from `specs/001-compatibility-baseline/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: 本 Feature 无代码产物，质量验证通过 JSON schema 校验 + 人工交叉比对完成。

**Organization**: Tasks 按 user story 分组，支持独立实施和独立验证。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行执行（不同文件，无依赖关系）
- **[Story]**: 所属 User Story（如 US1, US2, US3）
- 每个任务描述包含精确文件路径

## Path Conventions

基线产物均为文档/数据文件，全部位于 `specs/001-compatibility-baseline/` 目录下。
无源代码变更。

---

## Phase 1: Setup（环境搭建）

**Purpose**: 准备 AgentScope 上游源码分析环境

- [X] T001 Clone AgentScope 上游仓库到本地临时工作区：`git clone -b main https://github.com/agentscope-ai/agentscope /tmp/agentscope-upstream`
- [X] T002 [P] 安装 Python 3.11+ 环境并安装 `pip install agentscope`（用于验证 `__init__.py` 导出列表）——使用静态 AST 分析替代（Python 3.10 环境，符合 research.md 策略）
- [X] T003 [P] 安装 JSON schema 验证工具：`npm install -g ajv-cli` 或 `pip install check-jsonschema`（用于 contract 校验）

---

## Phase 2: Symbol Extraction（基础前提）

**Purpose**: 从 AgentScope 源码提取公开符号，作为所有后续产物的基础输入

**⚠️ CRITICAL**: 本阶段完成前，任何 User Story 产物生成都无法开始

- [X] T004 编写并运行 Python 符号提取脚本，遍历 `src/agentscope/` 下所有 `__init__.py`，使用 `inspect` + `importlib` 提取公开符号（module/class/function/method/protocol/enum/event/exception/serialized_structure/decorator/extension_point）
- [X] T005 交叉验证提取结果：对比 `__init__.py` 中的 `__all__` 声明与实际 `dir()` 结果，标记仅源码可见的符号，产生原始符号清单（临时文件 `_raw-symbols.json`）

**Checkpoint**: 原始符号清单就绪，可以开始生成各基线产物

---

## Phase 3: User Story 1 - 锁定上游兼容版本 (Priority: P1) 🎯 MVP

**Goal**: 产出 `version-lock.json`，锁定兼容目标的完整版本信息

**Independent Test**: `jq '.commit_hash | length' specs/001-compatibility-baseline/version-lock.json` 输出 `40`

### Implementation for User Story 1

- [X] T006 [US1] 从 `/tmp/agentscope-upstream` 获取完整信息：`git log -1 --format="%H"` 获取 40 字符 commit hash，`git describe --tags` 获取 release tag，记录仓库 URL
- [X] T007 [US1] 从 `pip show agentscope` 或 `pyproject.toml` 提取 Python 版本约束和核心依赖版本（Pydantic、httpx、openai 等）
- [X] T008 [US1] 创建 `specs/001-compatibility-baseline/version-lock.json`，填入所有必填字段，使用 `jq` 或 `ajv` 对照 `contracts/version-lock.schema.json` 校验

**Checkpoint**: 上游版本已锁定，`version-lock.json` 通过 schema 校验

---

## Phase 4: User Story 2 - 查看公开能力清单 (Priority: P1) 🎯 MVP

**Goal**: 产出 `api-inventory.json`，枚举所有 AgentScope 公开能力并关联可观察行为

**Independent Test**: `jq '.capabilities | length' specs/001-compatibility-baseline/api-inventory.json` 应在 100-300 范围内

### Implementation for User Story 2

- [X] T009 [US2] 基于 `_raw-symbols.json`，逐条目填充必填字段：`capability_id`（kebab-case）、`category`（功能域）、`module`、`python_import_path`、`symbol_name`、`symbol_type`、`source_location`、`doc_location`、`is_public_api`、`has_runtime_behavior`、`dependencies`
- [X] T010 [P] [US2] 为每个能力编写功能说明（`description`）：阅读对应源码文件和文档，提取一句话功能描述
- [X] T011 [P] [US2] 为 `has_runtime_behavior=true` 的能力填写可观察行为信息（`observable_behaviors`）：从函数签名提取 `input_params`/`param_defaults`，从源码推断 `return_type`、事件类型、异常类型
- [X] T012 [US2] 创建 `specs/001-compatibility-baseline/api-inventory.json`，包含所有已填充的 Capability 条目
- [X] T013 [US2] 使用 `ajv validate -s contracts/api-inventory.schema.json -d api-inventory.json` 校验；手动交叉检查：确保所有 14 个核心模块（agent/model/message/event/tool/formatter/middleware/permission/workspace/state/mcp/skill/embedding/credential）在 Inventory 中均有对应 `module` 条目

**Checkpoint**: `api-inventory.json` 通过 schema 校验，模块覆盖率 100%

---

## Phase 5: User Story 3 + 4 - MVP 范围 & 兼容等级 (Priority: P1 + P2)

**Goal**: 产出 `capability-matrix.json`，为每项能力标记优先级、兼容等级和状态

**Independent Test**: `jq '.entries[] | select(.priority == null or .target_level == null)' specs/001-compatibility-baseline/capability-matrix.json` 输出为空（无未标记条目）

**Depends on**: Phase 4（需要 capability_id 列表）

### Implementation for User Story 3 & 4

- [X] T014 [US3] 为所有能力标记 `priority`：根据 spec.md User Story 3 的要求，Message/ContentBlock/ModelRequest-Response/Streaming/Tool-Toolkit/Agent-ReActLoop/Memory/Event/Middleware/ErrorModel/Cancellation/Timeout 涉及的核心类型标记为 `MVP_REQUIRED`；辅助类型标记为 `CORE_REQUIRED` 或 `ADVANCED`；明确延期的标记为 `DEFERRED`；明确不支持的标记为 `INTENTIONALLY_UNSUPPORTED`
- [X] T015 [US4] 为所有能力标记 `target_level`（L0-L5）：对照 spec.md FR-008 的 6 级定义和数据协议复杂度评估
- [X] T016 [US3] 为所有能力设置初始 `status`：当前基线阶段所有条目标记为 `NOT_ANALYZED`，少数已通过分析确认的标记为 `SPECIFIED`
- [X] T017 [US3] 为每项能力补充 `test_fixture_ids`（引用验证来源：AgentScope 测试/官方示例/文档/源码分析/Runtime Probe/边界测试）和 `notes`
- [X] T018 [US3] 创建 `specs/001-compatibility-baseline/capability-matrix.json`，使用 `ajv validate -s contracts/capability-matrix.schema.json -d capability-matrix.json` 校验；审核 MVP_REQUIRED 能力集合是否覆盖所有 12 个核心领域

**Checkpoint**: `capability-matrix.json` 通过 schema 校验，MVP 边界清晰

---

## Phase 6: User Story 5 - 能力间依赖关系 (Priority: P2)

**Goal**: 产出 `dependency-map.json`，建立能力间 `requires`/`extends`/`uses` 有向图

**Independent Test**: 拓扑排序结果与节点数一致，无循环依赖

**Depends on**: Phase 5（需要 capability_id 和 category 信息）

### Implementation for User Story 5

- [X] T019 [US5] 分析每项能力的 `dependencies` 字段（已在 api-inventory.json 中），构建 `DepEdge` 列表（from/to/relation）
- [X] T020 [US5] 将每项能力分配到架构层 `layer`：`foundation`（Message、Event、State、Credential 等基础协议）、`model`（Model、Formatter）、`tool`（Tool、Toolkit、MCP）、`agent`（Agent、Middleware、Permission）、`extended`（Workspace、Skill、Embedding、RAG、app/ 服务层）
- [X] T021 [US5] 执行拓扑排序生成 `topological_order`；标记 `independent` 为 true 的能力（无内部依赖的 foundation 层能力）；验证无循环依赖
- [X] T022 [US5] 创建 `specs/001-compatibility-baseline/dependency-map.json`，使用 schema 校验通过

**Checkpoint**: `dependency-map.json` 通过 schema 校验，为后续 Feature 提供实现顺序

---

## Phase 7: User Story 6 - Trace 结构与归一化规则 (Priority: P2)

**Goal**: 产出 `trace-schema.json` 和 `normalization-rules.json`，定义差分测试标准

**Independent Test**: trace-schema 覆盖所有 12 个必需字段类别；normalization-rules 区分可标准化/不可忽略字段

### Implementation for User Story 6

- [X] T023 [US6] 基于 `contracts/trace-schema.schema.json` 的 $defs 定义，填充具体的字段约束和 `additionalProperties` 策略，创建 `specs/001-compatibility-baseline/trace-schema.json`
- [X] T024 [US6] 使用 `check-jsonschema` 校验 Trace 结构定义通过
- [X] T025 [US6] 定义归一化规则的可标准化字段列表：为每个字段（时间戳/UUID/Trace ID/Request ID/网络耗时/Provider ID/Map key 顺序/浮点误差）编写 `NormalizationRule` 条目
- [X] T026 [US6] 定义禁止忽略字段列表（`immutable_fields`）：写入 FR-015 中列出的 10 类字段的 JSONPath 表达式
- [X] T027 [US6] 创建 `specs/001-compatibility-baseline/normalization-rules.json`，使用 schema 校验通过
- [X] T028 [US6] 为所有 MVP_REQUIRED 能力创建至少一个测试场景 ID

**Checkpoint**: Trace 和归一化规则定义完成，所有 MVP_REQUIRED 能力拥有测试场景

---

## Phase 8: User Story 7 - 明确不支持的能力 (Priority: P3)

**Goal**: 产出 `exclusion-list.json`，列出所有明确排除的能力及其原因

**Independent Test**: `jq '.exclusions[] | select(.reason == "" or .reason == null)' specs/001-compatibility-baseline/exclusion-list.json` 输出为空

**Depends on**: Phase 5（需要知道哪些能力标记为 INTENTIONALLY_UNSUPPORTED）

### Implementation for User Story 7

- [X] T029 [US7] 从 capability-matrix.json 中筛选分析排除范围。当前 INTENTIONALLY_UNSUPPORTED 为 0，排除清单覆盖架构性排除（app 服务层、TTS providers、Python-specific 类型）
- [X] T030 [US7] 补充排除范围：AgentScope `src/agentscope/app/` 服务层（FastAPI 多租户服务）已在 matrix 中标记 DEFERRED，在 exclusion-list 中声明为架构性排除
- [X] T031 [US7] 创建 `specs/001-compatibility-baseline/exclusion-list.json`，使用 schema 校验通过；`jq '.exclusions[] | select(.reason == "" or .reason == null)'` 输出为空

**Checkpoint**: `exclusion-list.json` 通过 schema 校验，无未解释的范围遗漏

---

## Phase 9: Example Inventory & Methodology（交叉关注）

**Purpose**: 完成剩余产物，编写方法论文档，执行最终验证

- [X] T032 [P] 遍历 `/tmp/agentscope-upstream/examples/` 目录，登记所有官方示例的基本信息：`example_id`、`title`、`description`、`source_path`、`complexity`
- [X] T033 [P] 阅读每个示例源码，分析其使用的 AgentScope 能力，填写 `capabilities_used` 列表
- [X] T034 创建 `specs/001-compatibility-baseline/example-inventory.json`，使用 schema 校验通过
- [X] T035 编写 `specs/001-compatibility-baseline/methodology.md`：记录从上游源码到基线产物的完整生成流程
- [X] T036 运行 `specs/001-compatibility-baseline/quickstart.md` 中的自动化验证脚本，所有检查通过
- [X] T037 最终交叉验证：所有 AgentScope 顶层公开模块在 api-inventory.json 中有对应条目，app 服务层在 exclusion-list.json 中有明确排除声明

**Checkpoint**: 所有基线产物就绪，通过 quickstart.md 全量验证

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖，可立即开始
- **Symbol Extraction (Phase 2)**: 依赖 Phase 1（需要克隆的仓库和 Python 环境）
- **US1: Version Lock (Phase 3)**: 依赖 Phase 2（需要仓库信息），可与 Phase 4 并行
- **US2: API Inventory (Phase 4)**: 依赖 Phase 2（需要符号清单）
- **US3+US4: Capability Matrix (Phase 5)**: 依赖 Phase 4（需要 capability_id 列表）
- **US5: Dependency Map (Phase 6)**: 依赖 Phase 5（需要 capability_id 和 category）
- **US6: Trace & Normalization (Phase 7)**: 依赖 Phase 5（需要 capability_id 用于测试场景），可与 Phase 6 并行
- **US7: Exclusion List (Phase 8)**: 依赖 Phase 5（需要 INTENTIONALLY_UNSUPPORTED 标记）
- **Polish (Phase 9)**: 依赖所有 Phase 3-8

### User Story Dependencies

```
Phase 1 (Setup) ──► Phase 2 (Symbol Extraction)
                          │
          ┌───────────────┼───────────────┐
          ▼               ▼               ▼
    Phase 3 (US1)   Phase 4 (US2)         (独立)
    版本锁定          API Inventory         │
          │               │               │
          │               ▼               │
          │         Phase 5 (US3+US4)      │
          │         能力矩阵                │
          │               │               │
          │       ┌───────┴───────┐       │
          │       ▼               ▼       │
          │  Phase 6 (US5)  Phase 7 (US6) │
          │  依赖图          Trace/归一化  │
          │       │               │       │
          │       ▼               │       │
          │  Phase 8 (US7)  ◄─────┘       │
          │  排除清单                       │
          │       │                        │
          └───────┴────────────────────────┘
                       │
                       ▼
                Phase 9 (Polish)
          Example Inventory + Methodology + 验证
```

### Within Each User Story

- 数据分析/提取任务 → 产物创建任务 → Schema 校验任务
- 每个 Phase 内的 [P] 任务可并行执行
- 产物创建是串行瓶颈（单文件）

### Parallel Opportunities

- Phase 3 (US1) 与 Phase 4 (US2) 可部分并行（version-lock 不需要符号提取结果，api-inventory 需要）
- Phase 6 (US5) 与 Phase 7 (US6) 完全独立，可并行
- Phase 8 (US7) 可与 Phase 6/7 并行
- Phase 9 中 T032/T033（examples 分析）与 T035（methodology）可并行

---

## Parallel Example: Phase 5-8 Pipeline

```bash
# Phase 5: 能力矩阵（顺序执行，因为写入同一文件）
Task: "T014 标记 priority"
Task: "T015 标记 target_level"  
Task: "T016 设置初始 status"
Task: "T017 补充 test_fixture_ids"
Task: "T018 创建 capability-matrix.json"

# Phase 6 和 Phase 7 可同时启动（使用不同文件）
Task: "T019-T022 依赖图分析 → dependency-map.json"（并行）
Task: "T023-T028 Trace 结构 + 归一化规则 → trace-schema.json + normalization-rules.json"（并行）

# Phase 8: 排除清单
Task: "T029-T031 排除清单 → exclusion-list.json"
```

---

## Implementation Strategy

### MVP First（仅 Phase 1-5）

1. 完成 Phase 1: Setup
2. 完成 Phase 2: Symbol Extraction（CRITICAL）
3. 完成 Phase 3: US1 版本锁定
4. 完成 Phase 4: US2 API Inventory
5. 完成 Phase 5: US3+US4 能力矩阵（含 MVP 范围和兼容等级）
6. **STOP AND VALIDATE**: 此时已有版本锁定 + 能力清单 + MVP 范围，后续所有 Feature 可基于此开始实现
7. Phase 6-9 可在 MVP 后续迭代中完成

### Incremental Delivery

1. Setup + Symbol Extraction → 符号清单就绪
2. + Version Lock → 版本已固定（US1 ✅）
3. + API Inventory → 能力清单可查阅（US2 ✅）
4. + Capability Matrix → MVP 范围确定（US3+US4 ✅）**← 最小可交付基线**
5. + Dependency Map → 实现顺序可规划（US5 ✅）
6. + Trace & Normalization → 差分测试标准就绪（US6 ✅）
7. + Exclusion List → 范围无遗漏（US7 ✅）
8. + Example Inventory + Methodology + Validation → Feature 完整交付

### 单人执行策略

按 Phase 1→2→3→4→5→6/7 并行→8→9 顺序执行，预计 3-5 个工作日（大部分时间为人工标注）。

---

## Notes

- 本 Feature 无代码产物，所有 tasks 以文档编写和数据分析为主
- [P] 任务标注的是分析/提取层面的可并行性（例如分析不同模块的文档可同时进行）
- 每个 Phase 的 Checkpoint 表示可独立验证的里程碑
- `ajv` 或 `check-jsonschema` 验证命令需在所有 `.json` 产物创建后立即执行
- `_raw-symbols.json` 是临时中间产物，不纳入正式基线交付
- 所有产物路径均相对于 `specs/001-compatibility-baseline/`
