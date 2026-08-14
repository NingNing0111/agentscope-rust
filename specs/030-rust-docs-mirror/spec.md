# Feature Specification: docs/rust 项目文档一比一镜像 docs/python

**Feature Branch**: `030-rust-docs-mirror`

**Created**: 2026-08-13

**Status**: Draft

**Input**: User description: "参考docs/python项目文档格式，一比一实现docs/rust项目文档。文档里需要补充示例，示例放到examples下"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Rust 开发者按 docs/rust 快速上手 (Priority: P1)

一名具备 Rust 基础、尚未接触过 AgentScope 的开发者，打开 `docs/rust`，从索引页开始，跟随快速上手指南完成依赖引入、模型凭据配置，并在 30 分钟内运行起第一个能流式对话的 Agent。索引页与快速上手共同构成整个文档体系的第一价值入口。

**Why this priority**: 如果新用户无法在合理时间内跑通第一个 Agent，后续所有模块文档都不会被阅读。这是文档体系的 MVP——其余模块文档与示例都是建立在「用户已经能运行起一个 Agent」这一前提之上。

**Independent Test**: 仅交付 `index` + `quickstart` + `release-notes` 三个页面及其引用的 `examples/quickstart` 示例即可独立验证：一位新用户按照文档操作，在干净环境中成功运行起一个流式对话 Agent，并看到预期的终端输出。

**Acceptance Scenarios**:

1. **Given** 用户已安装 Rust 工具链且拥有有效模型服务 API Key，**When** 用户从 `docs/rust/zh/index` 进入并按照 `quickstart` 逐步操作，**Then** 用户能在 30 分钟内运行起第一个可对话的 Agent 并收到模型响应。
2. **Given** 用户未配置必需的 API Key，**When** 用户运行 `examples/quickstart`，**Then** 程序给出明确的环境变量缺失提示（`DASHSCOPE_API_KEY`），且文档说明了正确的配置方法。
3. **Given** 用户完成了第一个 Agent，**When** 用户想继续深入某个能力（工具、记忆、RAG），**Then** 索引页能将其引导至对应模块文档，模块文档再引导至对应的 examples/ 示例。

---

### User Story 2 - 按模块查阅文档并运行对应示例 (Priority: P2)

一名正在基于 AgentScope Rust 开发应用的开发者，需要使用某个具体能力（如工具系统、MCP、记忆、RAG、工作空间、沙箱）。他打开对应模块文档，了解该模块解决什么问题、核心概念与主要公开类型，并将文档引用的 `examples/` 下对应示例复制或运行，作为自己的起点。

**Why this priority**: 这是文档体系的主体，覆盖开发者日常最高频的「这个模块怎么用」需求。它独立于快速上手——即使快速上手尚未撰写，单模块文档对已经开始使用框架的开发者仍有直接价值。

**Independent Test**: 任选一个模块（如 MCP），只交付该模块文档 + `examples/mcp` 示例即可独立验证：开发者仅依据该文档和示例，能够连接一个 MCP server 并让 Agent 成功调用其工具，无需阅读源码。

**Acceptance Scenarios**:

1. **Given** 某模块文档已存在，**When** 开发者阅读该模块文档，**Then** 他能了解到该模块的核心概念、主要公开类型及其职责，无需翻阅源码。
2. **Given** 某模块文档中包含运行方式说明，**When** 开发者按文档执行对应的 `examples/` 示例，**Then** 示例能够编译并按文档描述的行为运行（真实模型调用需凭据，无凭据时给出明确错误）。
3. **Given** 开发者使用了某模块暂不支持的能力，**When** 他查阅该模块文档，**Then** 文档以统一的状态标注明确说明该能力在 Rust 中「未实现/计划中」及缺失范围，而非让他误以为可用（禁止伪兼容）。
4. **Given** 已实现的每个用户可见能力模块，**When** 用户浏览文档索引，**Then** 每个模块都有对应的文档入口，文档间通过链接相互引用相关模块，无悬空链接。

---

### User Story 3 - 熟悉 docs/python 的用户一比一对照迁移 (Priority: P2)

一名熟悉 AgentScope Python 文档的开发者（可能是迁移用户或跨语言使用者），因文档站导航习惯在 Python 版与 Rust 版之间切换。他按照相同的目录树与页面结构，在 docs/rust 中找到与 docs/python 对应的页面，并清楚地看到每个页面在 Rust 中的实现状态——已实现的页面给出 Rust 用法与示例，未实现的页面给出明确的「计划中」标注与缺失说明。

**Why this priority**: 项目宪法将「与 Python 参考实现的可观察兼容性」作为核心目标，「一比一镜像」的价值正是让导航对照成为可能——用户不必在 Python 版与 Rust 版之间猜测哪个页面对应哪个页面。这独立于 US1/US2：即使部分模块内容未撰写，结构对照本身对迁移用户就有价值。

**Independent Test**: 仅交付「完整目录树（含未实现页面的状态标注）+ 索引页」即可独立验证：一名熟悉 docs/python 的用户能按相同的路径在 docs/rust 中找到对应页面，且未实现页面不出现误导性的伪内容。

**Acceptance Scenarios**:

1. **Given** 用户熟悉 docs/python 的某个页面路径，**When** 他以相同路径在 docs/rust 中查找，**Then** 能定位到对应的中文页面（已实现页有完整内容，未实现页有状态标注）。
2. **Given** 某能力在 Rust 中未实现，**When** 用户打开对应页面，**Then** 页面统一标注「计划中/未实现」，说明该能力在 Python 中的作用、Rust 缺失的范围，以及（如有）替代能力，与兼容性矩阵一致。
3. **Given** 用户想确认某个页面在 Python 与 Rust 中的对应关系，**When** 他查阅页面上的对照信息，**Then** 每页均能定位其对应的 docs/python 源页面，双向对照清晰。

---

### User Story 4 - 维护者保持文档、示例与代码同步 (Priority: P3)

一名框架维护者在 API 变更或新增能力后，需要确保文档与示例不脱节。他依靠 CI 对 `examples/` 的编译校验发现过期示例，并依靠「镜像映射清单」确认 docs/rust 与 docs/python 的结构一致性没有因新增/删除页面而破坏。

**Why this priority**: 文档的可信度取决于其与代码的同步性。编译校验与结构对照机制把「文档过期」从人工发现变为自动化发现。它不构成用户使用框架的必经路径，因此优先级最低。

**Independent Test**: 仅交付「examples/ 全部为 workspace 成员 crate + 镜像映射清单」即可独立验证：改动任意公开 API 后运行 CI，过期的示例编译错误能被自动发现；新增/删除 Python 文档页面后，映射清单能提示结构漂移。

**Acceptance Scenarios**:

1. **Given** 某公开 API 发生破坏性变更，**When** 运行 CI，**Then** 引用该 API 的 examples/ 示例出现编译错误，维护者据此更新文档与示例。
2. **Given** docs/python 新增或删除了某个页面，**When** 维护者核对其镜像映射清单，**Then** 清单能反映出 docs/rust 的缺失或多余页面，提示补齐或同步移除。
3. **Given** 某模块的兼容性状态发生变化，**When** 维护者更新兼容性矩阵，**Then** 对应文档页的状态标注被同步更新，二者保持一致。

### Edge Cases

- docs/python 中 `deploy/openapi.json` 是 Python 后端自动生成的 OpenAPI 产物，Rust 当前无 agent-service 后端 → 该文件不在镜像范围内，须在镜像映射清单与文档中记录此例外，避免误以为遗漏。
- 某能力在 Rust 中「部分支持」（如 context 仅实现了运行时状态注入、权限系统仅有确认/拦截机制）→ 页面状态标注须区分「已实现」「部分支持」「计划中」三档，并对部分支持给出边界说明，不得笼统标注。
- 文档引用的示例需要真实模型 API Key，CI 中无法运行 → 示例的「可运行」以「可通过编译校验」为基准，真实调用依赖用户凭据；示例必须对缺失凭据给出明确、可操作的错误提示。
- 新示例 crate 引入重依赖（如 rag 的向量检索、sandbox 的隔离运行时）可能拖慢 CI → 示例应保持最小依赖，按需启用特性，不得引入非必要重依赖。
- docs/ 目录是独立 git 仓库且含 docs/python 既有内容 → 文档组织不得破坏 docs/ 的嵌套仓库结构、不得改动或删除 docs/python 任何文件。
- 未实现页面若内容空泛会沦为占位符，若照抄 Python 内容则构成伪兼容 → 未实现页面只保留「状态标注 + Python 能力简介 + Rust 缺失说明」，不展开伪造的 Rust 用法。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `docs/rust` 的目录树 MUST 一比一镜像 `docs/python/zh`（索引、快速上手、构建块、部署、其他全部章节与页面，共 50 个页面），不得缺页或新增多余页面；`deploy/openapi.json` 除外（见 FR-004）。
- **FR-002**: 本期文档语言为中文，位于 `docs/rust/zh/` 下；`en/` 目录本期不创建，但目录组织 MUST 保持可在未来对称补充英文版而不需重构。
- **FR-003**: 每个页面 MUST 沿用 `docs/python` 的 .mdx 格式：YAML frontmatter（`title`/`description`）、章节结构、Mintlify 组件（`CardGroup`/`Card`/`Note`/`Tip` 等）与站内链接模式（版本化路径，如 `/versions/<ver>/zh/...`）。
- **FR-004**: 每个页面 MUST 在文档中标注实现状态，区分三档：**已实现**（提供完整的 Rust 内容）、**部分支持**（说明支持范围与边界）、**计划中**（说明 Python 能力简介与 Rust 缺失范围）。未实现页面 MUST NOT 出现伪造的 Rust 用法（宪法第五条在文档层面的延伸）。
- **FR-005**: 已实现且含代码的页面 MUST 提供可运行示例，示例存放在仓库 `examples/` 下，文档通过路径与运行命令（如 `cargo run -p <example>`）引用；文档中禁止粘贴无法编译的代码片段。
- **FR-006**: `examples/` 下 MUST 按能力模块提供示例，每个示例为 workspace 成员 crate（与 `examples/pi-rust` 同模式，登记在根 `Cargo.toml` 的 `[workspace] members`），从而被 CI 的 `cargo check --workspace --all-targets` 自动编译校验。
- **FR-007**: 示例集 MUST 覆盖：快速上手（quickstart）、流式对话（chat）、自定义工具（tool）、MCP（mcp）、技能（skill）、Agent 编排与人工介入（agent）、记忆（memory）、检索增强问答（rag）、工作空间（workspace）、沙箱（sandbox）。一个示例 MAY 服务多个相关文档页。
- **FR-008**: 每篇含代码的文档页 MUST 引用至少一个 `examples/` 下的示例并说明运行方式；文档中的配置项（环境变量名、参数名、默认值）MUST 与代码实际定义逐一核对一致（如 `DASHSCOPE_API_KEY`）。
- **FR-009**: 已实现模块的文档 MUST 标注兼容性等级（L1-L4）与已知偏差（如有），与兼容性矩阵记录一致；MUST NOT 宣称支持实际返回 `UnsupportedFeature` 的能力。
- **FR-010**: 文档站站内链接 MUST 100% 有效，无悬空链接；每个页面 MUST 能定位其对应的 `docs/python` 源页面，实现双向对照。
- **FR-011**: 文档维护过程中 MUST 维护一份「镜像映射清单」（docs/python 页面 ↔ docs/rust 页面 ↔ 实现状态 ↔ 引用示例）作为一比一对齐的权威依据，随文档入库版本化。
- **FR-012**: 文档工作 MUST NOT 破坏 `docs/` 下已存在的任何内容（`docs/python` 全部文件、`docs/` 嵌套 git 仓库结构）。

### Key Entities *(include if feature involves data)*

- **文档站点（docs/rust）**: 仓库根目录下的 Rust 中文文档集合，.mdx 格式，随仓库版本化，与 docs/python 结构一比一对应。
- **文档页面（Doc Page）**: 镜像 docs/python 的单个 .mdx 页面，包含 frontmatter、章节内容、状态标注与站内链接。
- **状态标注（Status Annotation）**: 每个页面对其对应能力在 Rust 中的实现状态（已实现/部分支持/计划中）的统一声明，是「全量镜像 + 诚实标注」的核心载体。
- **示例 crate（Example Crate）**: `examples/` 下的 workspace 成员 crate，作为文档正确性的可编译验证锚点，被 CI 自动编译。
- **镜像映射清单（Mirror Map）**: docs/python 页面与 docs/rust 页面的双向对照表，含实现状态与引用示例，保证一比一结构不漂移。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `docs/rust/zh` 与 `docs/python/zh` 页面结构一比一：除 `deploy/openapi.json` 这一记录在案的例外外，无缺页、无多余页面（页面清单与镜像映射清单一致）。
- **SC-002**: 100% 页面具有统一的状态标注（已实现/部分支持/计划中），且标注内容与代码实际状态一致——不存在文档宣称可用、但实际返回 `UnsupportedFeature` 的能力，不存在未标注的已知偏差。
- **SC-003**: 每篇含代码的已实现模块文档引用至少 1 个 `examples/` 下示例，示例路径与运行命令与实际文件一一对应。
- **SC-004**: `examples/` 下全部示例通过 `cargo check --workspace --all-targets` 与 `cargo clippy --workspace --all-targets -D warnings`，CI 无新增失败。
- **SC-005**: 一名具备 Rust 基础的新用户仅依据索引与快速上手文档，能在 30 分钟内（不含工具链安装时间）运行起第一个可对话的 Agent。
- **SC-006**: 文档站内链接 100% 有效，无悬空链接；每个页面均能定位其对应 docs/python 源页面。
- **SC-007**: 抽查任一已实现模块文档中的配置项（环境变量、参数默认值），与代码实际定义 100% 一致。

## Assumptions

- 语言采用仅中文（用户已确认），置于 `docs/rust/zh/`；`en/` 留待后续对称补充，本期不创建。
- 镜像范围采用全量镜像 + 状态标注（用户已确认）：完整复制 docs/python 目录树，未实现能力页面以统一状态标注呈现，不展开伪内容。
- 示例组织采用每模块一个示例（用户已确认）：`examples/` 下按能力提供可运行示例 crate，复用 `examples/pi-rust` 的 workspace 成员模式。
- `docs/python/en/deploy/openapi.json` 是 Python 后端 OpenAPI 生成物，Rust 当前无对应后端，不在镜像范围内；此例外记录于镜像映射清单。
- docs/ 为独立 git 仓库且含既有 docs/python 内容，文档工作仅新增 docs/rust，不触碰 docs/python。
- 示例的「可运行」以 CI 编译校验为基准；真实模型调用需要用户凭据（如 `DASHSCOPE_API_KEY`），示例对缺失凭据给出明确错误提示。
- `release-notes` 与 `change-log` 页面内容基于仓库既有的 `CHANGELOG.md` 与历史发布记录，不虚构版本历史。
- 部分支持模块（如 context、permission-system）的支持边界以当前代码实际能力为准，写入文档前须经实现状态核验。
