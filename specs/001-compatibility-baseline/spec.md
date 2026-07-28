# Feature Specification: AgentScope Compatibility Baseline

**Feature Number**: 000-compatibility-baseline

**Created**: 2026-07-28

**Status**: Draft

**Input**: 建立 AgentScope Rust 项目的兼容性基线，包括上游版本锁定、公开 API 清单、能力矩阵、可观察行为清单、依赖图、示例清单、兼容性状态矩阵、差分测试 Trace 定义及归一化规则。

## Clarifications

### Session 2026-07-28

- Q: API Inventory 和能力矩阵的生成依赖多少自动化？ → A: 脚本辅助 + 人工标注——使用 Python 脚本自动提取模块/类/函数符号列表，人工补充语义信息（描述、优先级、兼容等级、依赖关系）
- Q: 基线产物是一次性快照还是可复现流水线？ → A: 一次性快照 + 方法文档——基线数据为针对锁定版本的具体产物，同时输出一份描述生成流程的方法论文档，供将来上游版本升级时参考复用
- Q: 基线产物如何组织存储？ → A: 分文件 JSON——每种产物一个独立的 JSON 文件，统一放在 feature 目录下
- Q: 预期的 API Inventory 中大概有多少个能力条目？ → A: 100-300（中等规模），覆盖主要模块的类、方法、函数和数据结构
- Q: 本次兼容基线 feature 中，需要实际运行 Python AgentScope 来记录行为吗？ → A: 纯静态分析——仅通过源码阅读和文档分析生成清单，不实际运行 Python AgentScope

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 锁定上游兼容版本 (Priority: P1)

作为项目维护者，我需要将兼容目标绑定到明确的 AgentScope release 和 Git commit，以便所有后续开发工作在固定基线上开展，不会因为上游 main 分支的持续变化而失去参照。

**Why this priority**: 版本锁定是所有兼容性工作的前提。没有固定的上游版本，所有后续的能力分析和差分测试都失去了可比对的参照物。

**Independent Test**: 可通过读取生成的版本锁定文件来验证——文件存在、格式有效、包含所有必填字段（仓库地址、release tag、commit hash、Python 版本、依赖版本、生成日期）。

**Acceptance Scenarios**:

1. **Given** 一个 AgentScope Python 项目的仓库地址和目标 release，**When** 生成版本锁定文件，**Then** 文件包含完整的 commit hash、仓库地址、release tag、Python 版本要求、核心依赖版本列表和基线生成日期。
2. **Given** 已生成的版本锁定文件，**When** 自动化脚本读取该文件，**Then** 能够解析出所有版本信息字段且格式为机器可读的 JSON/YAML。

---

### User Story 2 - 查看公开能力清单 (Priority: P1)

作为 Rust 实现者，我需要查看 AgentScope 的完整公开 API 清单（模块、类、函数、协议、事件、异常和数据结构的详细列表），以便明确每一阶段需要实现的范围和优先级。

**Why this priority**: 没有完整的清单就不知道"要兼容什么"。这是整个项目的需求输入，直接决定后续所有 feature 的范围。

**Independent Test**: 可通过验证清单文件的存在性和完整性来测试——每个条目拥有唯一 capability ID、模块归属、Python import path、符号名称与类型、功能说明、优先级和兼容状态。

**Acceptance Scenarios**:

1. **Given** 锁定的 AgentScope 版本源码，**When** 生成公开 API 清单，**Then** 清单中的每个条目至少包含：capability_id、category、upstream_symbol、source_location、description、symbol_type、is_public_api、priority、target_level、status。
2. **Given** 生成的清单，**When** 按模块过滤，**Then** 能获得每个模块的完整能力子集，且子集内无遗漏的核心符号。

---

### User Story 3 - 确定 MVP 范围 (Priority: P1)

作为项目负责人，我需要将全部能力划分为明确的实现阶段（MVP_REQUIRED / CORE_REQUIRED / ADVANCED / DEFERRED / INTENTIONALLY_UNSUPPORTED），以避免在单个 feature 中试图重构整个 AgentScope。

**Why this priority**: 范围划分决定了开发顺序和资源分配。不划分阶段的庞大清单无法指导迭代式开发。

**Independent Test**: 可通过验证每个能力条目都拥有明确的优先级标记来测试——MVP_REQUIRED 的能力应覆盖 Message、Model、Tool、Agent Loop、Event、Middleware、Error Model、Cancellation、Timeout 所涉及的核心类型和流程。

**Acceptance Scenarios**:

1. **Given** 完整的 capability 清单，**When** 进行优先级划分，**Then** 每项能力均被标记为 MVP_REQUIRED、CORE_REQUIRED、ADVANCED、DEFERRED 或 INTENTIONALLY_UNSUPPORTED 之一。
2. **Given** MVP 范围已定义，**When** 审核 MVP_REQUIRED 能力集合，**Then** 其至少包含 Message、ContentBlock、ModelRequest/Response、Streaming、Tool/Toolkit、Agent/ReAct Loop、Memory、Event、Middleware、Error Model、Cancellation、Timeout 的核心类型。

---

### User Story 4 - 了解每项能力的兼容等级 (Priority: P2)

作为项目使用者，我需要明确 Rust 实现与 Python 参考实现之间的兼容程度（L0-L5），而不是看到模糊的"兼容 AgentScope"声明。

**Why this priority**: 兼容等级帮助用户和开发者精确理解每项能力当前的可依赖程度，这是迁移决策的关键信息。

**Independent Test**: 每项能力在兼容矩阵中拥有独立的 target_level 和当前 status，所有等级符合 L0-L5 定义。

**Acceptance Scenarios**:

1. **Given** 任意已登记的能力，**When** 查看其兼容等级，**Then** 可看到明确的 L0-L5 等级标记，且每个等级有明确的定义说明。
2. **Given** 兼容矩阵全部条目，**When** 用户按 target_level 筛选，**Then** 能准确获得各等级的能力分布和统计。

---

### User Story 5 - 查看能力间的依赖关系 (Priority: P2)

作为 Rust 开发者，我需要了解核心能力之间的依赖关系，以确定实现顺序并避免循环依赖。

**Why this priority**: 依赖图决定了实现顺序——必须先实现基础协议（如 Message），才能依赖其构建高层的 Agent。没有依赖图会导致实现时才发现前置能力缺失。

**Independent Test**: 可通过验证生成的依赖图来测试——依赖图准确反映了各能力间的 requires/provides 关系，且无循环依赖。

**Acceptance Scenarios**:

1. **Given** 所有已登记的能力，**When** 生成依赖关系图，**Then** 图明确展示哪些能力是基础协议、哪些依赖 Model、Tool、Agent，哪些可以独立实现。
2. **Given** 一个依赖关系图，**When** 执行拓扑排序，**Then** 不产生循环依赖，且能给出合理的实现顺序建议。

---

### User Story 6 - 了解如何验证兼容性 (Priority: P2)

作为测试开发者，我需要知道每项兼容行为如何被验证，以及差分测试使用的标准 Trace 结构和归一化规则。

**Why this priority**: 兼容性的定义必须可验证。没有标准化的验证方法（Trace 结构 + 归一化规则），兼容性测试无法在两个独立实现之间进行确定性比较。

**Independent Test**: 可通过验证 Trace 定义和归一化规则文档的完整性来测试——Trace 覆盖所有必需字段类别，归一化规则明确列出允许标准化和禁止忽略的字段。

**Acceptance Scenarios**:

1. **Given** 一项 P0 或 P1 capability，**When** 查看其验证来源，**Then** 关联了至少一种验证来源（AgentScope 测试/官方示例/文档/源码分析/Runtime Probe/边界测试）。
2. **Given** Trace 结构定义和归一化规则，**When** 测试框架生成比对 Trace，**Then** 归一化规则明确了哪些字段可被标准化（时间戳、UUID 等）以及哪些字段不可被忽略（事件顺序、Tool 参数等）。

---

### User Story 7 - 查看明确不支持的能力 (Priority: P3)

作为用户和开发者，我需要知道哪些 AgentScope 能力在当前目标中明确不支持以及原因，以避免对功能可用性产生错误预期。

**Why this priority**: 明确的不支持声明和原因记录比隐式省略更有价值——防止用户假设功能存在而遇到运行时错误，也防止开发者重复讨论已被否决的功能。

**Independent Test**: 存在一份明确的不支持能力列表，每项包含能力名称、不支持原因和替代建议（如有）。

**Acceptance Scenarios**:

1. **Given** 兼容性基线文档，**When** 查找某个不在 MVP 或 CORE 范围内的能力，**Then** 该能力或功能类别出现在 INTENTIONALLY_UNSUPPORTED 或 DEFERRED 列表中，而非简单地从清单中缺失。

---

### Edge Cases

- 当 AgentScope 上游源码中存在未在文档中记录的公开符号时如何处理？（应标记为"仅源码可见"，仍需登记）
- 当多个模块中存在同名符号时，如何区分？（通过完整的 Python import path 区分）
- 当 AgentScope 的依赖库（如 Pydantic）版本差异导致行为差异时，如何处理？（在版本锁定文件中记录完整依赖链）
- 当公开 API 的行为在不同平台（Linux/macOS/Windows）有差异时，以哪个平台为准？（以 Python 参考实现在 Linux 上的行为为基准）
- 当 AgentScope 源码中存在 `__all__` 声明与实际导出的符号不一致时，以哪个为准？（以实际可导入的符号为准）
- 当上游版本锁定后，如何记录后续发现的上游缺陷？（在已知偏差列表中登记，标记是否要在 Rust 侧修正）

## Requirements *(mandatory)*

### Functional Requirements

#### 版本锁定

- **FR-001**: 系统必须生成机器可读的上游版本锁定文件。文件必须包含：上游仓库地址、release/tag 名称、完整 Git commit hash（40 字符）、Python 版本约束、核心依赖及版本号、锁定文件生成日期。
- **FR-002**: 版本锁定文件必须使用 JSON 格式，具有稳定的 schema 和版本号，以便自动化工具可靠解析。

#### API Inventory

- **FR-003**: 系统必须生成公开 API 清单（API Inventory），列出所锁定 AgentScope 版本中所有公开可访问的模块、类、函数、方法、协议、枚举、事件、异常、序列化结构和装饰器。
- **FR-004**: 清单必须区分至少以下 symbol 类型：`module`、`class`、`function`、`method`、`protocol`、`enum`、`event`、`exception`、`serialized_structure`、`decorator`、`extension_point`。
- **FR-005**: 清单中每个条目必须包含：唯一的 `capability_id`、`category`（所属功能域）、`python_import_path`、`symbol_name`、`symbol_type`、`description`、`source_location`（文件路径及行号）、`doc_location`（文档链接）、`is_public_api`（布尔值）、`has_runtime_behavior`（布尔值）、`dependencies`（依赖的其他 capability_id 列表）、`priority`、`current_status`。

#### Capability Matrix

- **FR-006**: 系统必须生成能力兼容矩阵，每个能力条目至少包含：`capability_id`、`category`、`upstream_symbol`、`source_location`、`description`、`dependencies`、`priority`、`target_level`、`status`、`test_fixture_ids`、`notes`。
- **FR-007**: 每项能力的 `priority` 必须为以下之一：`MVP_REQUIRED`、`CORE_REQUIRED`、`ADVANCED`、`DEFERRED`、`INTENTIONALLY_UNSUPPORTED`。
- **FR-008**: 每项能力的 `target_level` 必须为以下之一：`L0`（尚未支持）、`L1`（数据协议兼容）、`L2`（核心运行行为兼容）、`L3`（公开 API 语义兼容）、`L4`（官方示例可低成本迁移）、`L5`（完整目标范围兼容）。
- **FR-009**: 每项能力的 `status` 必须为以下之一：`NOT_ANALYZED`、`ANALYZING`、`SPECIFIED`、`IMPLEMENTING`、`PARTIAL`、`COMPATIBLE`、`DEFERRED`、`UNSUPPORTED`、`BLOCKED`。

#### Observable Behavior Inventory

- **FR-010**: 系统必须为每项拥有运行时行为的能力记录其外部可观察行为（Observable Behavior Inventory）。外部可观察行为包括：输入参数名称/类型/默认值、返回值结构、序列化结果、事件类型、事件顺序、流式输出顺序、状态变化、Tool 调用、Memory 写入、异常、Timeout 行为、Cancellation 行为、Side Effects。

#### Dependency Map

- **FR-011**: 系统必须生成核心能力之间的依赖关系图（Dependency Map）。依赖关系必须具备以下分析能力：识别哪些能力是基础协议（无内部依赖）、哪些能力依赖 Model 抽象、哪些能力依赖 Tool 抽象、哪些能力依赖 Agent 抽象、哪些能力可以独立实现、哪些能力应延期实现。

#### Example Inventory

- **FR-012**: 系统必须登记 AgentScope 官方示例清单（Example Inventory），每个示例至少关联其使用的 `capability_id` 列表。

#### Trace Definition

- **FR-013**: 基线必须定义 Python 与 Rust 差分测试所使用的标准 Trace 结构，至少涵盖：`input`、`model_requests`、`model_responses`、`streaming_chunks`、`tool_calls`、`tool_results`、`events`、`memory_mutations`、`state_transitions`、`errors`、`cancellation`、`final_result`。

#### Normalization Rules

- **FR-014**: 基线必须定义差分比较时的字段归一化规则（Normalization Rules），明确列出允许在比较前被标准化的候选字段：时间戳、UUID、Trace ID、Request ID、网络耗时、Provider 生成的随机 ID、Map key 顺序、有界浮点误差。
- **FR-015**: 归一化规则必须明确声明以下内容不得默认忽略：事件顺序、Tool 参数值、Message Role、Finish Reason、Error Category、State Mutation、Cancellation State、Side Effects。

#### Explicit Exclusions

- **FR-016**: 所有不在当前目标范围内的能力必须出现在明确的排除清单中，并附排除原因，不得仅从清单中省略。
- **FR-017**: 基线必须附带一份方法论文档（Methodology Document），描述从上游源码生成基线产物（版本锁定、API Inventory、能力矩阵、依赖图、示例清单、Trace 定义）的完整流程，包括脚本使用方式、人工判断准则和输出格式说明。
- **FR-018**: 各基线产物必须采用独立的 JSON 文件存储，统一放在 feature 目录（`specs/001-compatibility-baseline/`）下，每种产物一个文件。至少包含：`version-lock.json`、`api-inventory.json`、`capability-matrix.json`、`dependency-map.json`、`example-inventory.json`、`trace-schema.json`、`normalization-rules.json`、`exclusion-list.json`、`methodology.md`。

### Key Entities

- **Upstream Version Lock**: 记录兼容目标的上游 AgentScope 完整版本信息，包括仓库、release、commit、Python 版本、依赖版本。
- **Capability**: AgentScope 的一个可识别、可追踪的公开能力单元——可以是模块、类、函数、方法、协议、枚举、事件、异常或数据结构。
- **Capability Matrix**: 所有能力的结构化汇总，包含优先级、兼容等级、当前状态等关键元数据。
- **Observable Behavior**: 一项能力的输入、输出、事件、状态变化、副作用等所有外部可观察行为的结构化描述。
- **Dependency Map**: 能力间 `requires`/`provides` 关系的有向图。
- **Example Reference**: 一个 AgentScope 官方示例及其所使用的能力列表的映射。
- **Standard Trace**: 差分测试中记录的一次执行全过程的完整结构化日志，包含输入、中间步骤和最终结果。
- **Normalization Rule**: 在 Trace 比较前对特定字段进行标准化的规则定义。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 上游版本已锁定到完整的 40 字符 Git commit hash，版本锁定文件满足所有必填字段且可被自动化脚本解析。
- **SC-002**: 所有顶层公开模块均已登记在 API Inventory 中，无遗漏的核心符号（通过交叉验证 `__init__.py` 导出列表和实际导入行为验证覆盖率）。
- **SC-003**: 所有已识别公开 API 均拥有 inventory 条目，每个条目包含全部必填属性。
- **SC-004**: 所有能力均已标记 `priority` 和 `target_level`，无任何能力使用默认/未定义值。
- **SC-005**: 所有 MVP_REQUIRED 能力均关联至少一个源码位置或文档来源作为分析依据。
- **SC-006**: 所有 MVP_REQUIRED 能力均拥有至少一个确定性测试场景（基于 Mock Model / Scripted Model / Recorded Model）。
- **SC-007**: 已生成能力依赖图，依赖关系可进行拓扑排序且不产生循环依赖。
- **SC-008**: MVP 边界清晰可辨——MVP_REQUIRED 能力集合可独立形成一个可理解的子集。
- **SC-009**: 已形成延期（DEFERRED）和不支持（INTENTIONALLY_UNSUPPORTED）能力列表，每项均附有原因说明。
- **SC-010**: 已定义标准 Trace 格式，覆盖输入、模型交互、工具调用、事件、错误和输出的完整生命周期。
- **SC-011**: 已定义差分比较的归一化规则，明确区分可标准化字段和禁止忽略的字段。
- **SC-012**: 基线中无未解释的范围遗漏——所有 AgentScope 顶层公开模块在 Inventory 中均有对应条目或明确的不支持声明。
- **SC-013**: Specification 中不包含 Rust crate 划分、trait 设计、依赖库选择或任何代码实现细节。
- **SC-014**: 基线附带方法论文档（methodology.md），描述从上游源码生成基线产物的完整流程。

## Assumptions

- 兼容目标 AgentScope 版本为最新稳定 release，使用 Python 3.10+。
- AgentScope Python 包的源码可通过 PyPI 或 GitHub 获取，且可安装运行用于行为验证。
- 本项目团队能够访问 AgentScope 的 GitHub 仓库和 PyPI 包。
- 差分测试将在 Linux 环境中执行（以消除平台差异）。
- API Inventory 的符号提取使用 Python 脚本自动化（遍历模块 `__init__.py`、`inspect` 反射），语义信息（描述、优先级、兼容等级、依赖关系）由人工标注补充。提取脚本本身不属于本 feature 的交付产物。
- MVP 范围的能力清单基于对 AgentScope 公开文档和源码的人工分析得出，后续可能根据分析深度进行调整。
- 本 baseline 仅通过源码静态分析和文档分析生成，不实际运行 Python AgentScope。运行时行为验证（如 Python Runtime Probe 记录事件顺序和状态变化）属于后续各模块 feature 的范围。
- 本 baseline 的基线数据针对锁定的 AgentScope 版本生成，是具体的一次性产物，后续上游版本升级时将参照本 feature 产出的方法论文档重新执行基线生成流程。
- API Inventory 的预期规模约为 100-300 个能力条目，覆盖 AgentScope 主要公开模块的类、方法、函数和数据结构。若实际分析发现超出此范围，将在进度报告中调整预期。
