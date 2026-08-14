---

description: "Task list for docs/rust 一比一镜像 docs/python"

---

# Tasks: docs/rust 项目文档一比一镜像 docs/python

**Input**: Design documents from `/specs/030-rust-docs-mirror/`

**Prerequisites**: plan.md、spec.md（4 用户故事）、research.md（crate 实现状态）、data-model.md（实体/50 页/10 示例）、contracts/（doc-page/example-crate/mirror-map 契约）、quickstart.md（验证场景 A-F）

**Tests**: 本 feature 为文档交付，以编译校验 + 结构校验为验证手段（quickstart.md 场景 A-F），不生成单元测试。

**Organization**: 任务按用户故事分组，每个故事可独立实现与验证。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行（不同文件、无依赖）
- **[Story]**: 所属用户故事（US1-US4）
- 描述含精确文件路径

## 关键约束（来自 research/contracts）

- 状态三档：`已实现`/`部分支持`/`计划中`；每页顶部 `<Note>` 状态块；计划中页禁 Rust 代码（宪法 §5）
- 镜像源 docs/python（2.0.7dev）；Rust 兼容基线 v2.0.5（commit `27b6a0d2`）；索引页声明版本差
- 示例 crate 登记根 `Cargo.toml` `[workspace] members`，过 `cargo check --workspace --all-targets` + clippy
- `docs/` 为独立嵌套 git 仓库，只新增 `docs/rust/`，不触碰 `docs/python/`
- 站内链接版本号用 Rust 侧版本（当前 `0.1.0`）

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 建立 docs/rust 目录骨架、契约锚点、示例 workspace 注册

- [x] 001 创建 `docs/rust/zh/` 目录树，镜像 `docs/python/zh` 全部 50 个页面路径（含 building-blocks/deploy/others 子目录），不复制内容，仅建目录占位
- [x] 002 创建 `docs/rust/mirror-map.md` 头部：版本声明（镜像源 2.0.7dev、Rust 基线 v2.0.5 commit 27b6a0d2、生成日期）+ openapi.json 例外登记 + 表头（python_page/rust_page/status/compat_level/example_crate/note）
- [x] 003 [P] 在根 `Cargo.toml` 的 `[workspace] members` 追加 10 个示例 crate 路径：`examples/quickstart`、`examples/chat`、`examples/tool`、`examples/mcp`、`examples/skill`、`examples/agent`、`examples/memory`、`examples/rag`、`examples/workspace`、`examples/sandbox`
- [x] 004 [P] 为 10 个示例 crate 建立 `Cargo.toml` 骨架（name/version/edition 走 workspace；默认 `src/main.rs`），依赖按 `examples/pi-rust` 模式声明所需 `agent_scope_*` crate 与 tokio/serde/schemars 等，暂不实现逻辑，先保证 `cargo check --workspace --all-targets` 通过

**Checkpoint**: 目录骨架就位；`cargo check` 因空 main 通过；mirror-map 头已建

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 全量镜像映射清单 + 状态标注规范——所有用户故事以此为权威依据

**⚠️ CRITICAL**: 未完成本阶段，任何页面状态标注都无从对齐

- [x] 005 生成 `docs/rust/mirror-map.md` 全量 50 条登记：按 research.md R1 逐页填写 status（已实现/部分支持/计划中）、compat_level（L1-L4 或空）、example_crate（按 data-model 第 4 节映射）、note（偏差/版本差）
- [x] 006 建立状态块规范落地：在 `docs/rust/` 下放一份 `STATUS-BLOCK.md` 或直接以 contracts/doc-page-contract 为准，给出三档 `<Note>` 模板（已实现/部分支持须列边界/计划中须列缺失）
- [x] 007 [P] 建立 `docs/rust/README.md`：说明 docs/rust 与 docs/python 的镜像关系、状态三档含义、版本差声明、如何对照 Python 文档
- [x] 008 确认 10 个示例 crate 的 README 骨架（运行命令、凭据要求 `DASHSCOPE_API_KEY`、预期输出占位），对齐 example-crate-contract

**Checkpoint**: mirror-map 50 条登记完整；状态块规范确定；README 就绪

---

## Phase 3: User Story 1 - 新用户按 docs/rust 快速上手 (Priority: P1) 🎯 MVP

**Goal**: index + quickstart + release-notes 三页 + examples/quickstart 示例，新用户 30 分钟内跑起第一个对话 Agent

**Independent Test**: 新用户按 quickstart.mdx 运行 `cargo run -p quickstart -- --prompt ...`，得到流式事件与最终回复；无凭据时得到明确错误提示

### Implementation for User Story 1

- [x] 009 [P] [US1] 实现 `examples/quickstart/`：最小 Agent（DashScopeChatModel + ToolKit + ReActAgent），`--prompt` 参数，无凭据时打印 `error: 缺少环境变量 DASHSCOPE_API_KEY` 并退出（不 panic）
- [x] 010 [P] [US1] 编写 `docs/rust/zh/index.mdx`：索引入口，含镜像版本声明（Rust 基线 v2.0.5 / 镜像源 2.0.7dev）、能力总览（CardGroup 引导至 agent/tool/context/workspace）、实现状态索引
- [x] 011 [P] [US1] 编写 `docs/rust/zh/quickstart.mdx`：环境准备、依赖引入（Cargo.toml 依赖列表）、凭据配置（DASHSCOPE_API_KEY 与代码一致）、运行 examples/quickstart、reply/reply_stream 两种用法、预期输出；引用 T009 示例路径
- [x] 012 [US1] 编写 `docs/rust/zh/release-notes.mdx`：基于 CHANGELOG.md 整理 0.1.0 版本历史与各模块新增能力，标注与 Python 2.0.5/2.0.7dev 的关系
- [x] 013 [US1] 自检 US1：运行 examples/quickstart 编译；核对 quickstart.mdx 中环境变量名/参数默认值与 T009 代码一致

**Checkpoint**: US1 独立可用——新用户可 30 分钟跑通第一个 Agent（MVP 达成）

---

## Phase 4: User Story 2 - 按模块查阅文档并运行对应示例 (Priority: P2)

**Goal**: 已实现模块的完整文档 + 对应 examples/ 示例，开发者按文档运行示例作为起点

**Independent Test**: 任选一模块（如 mcp），仅凭该模块文档 + examples/<name> 能连接 MCP server 并调用其工具，无需读源码

### Implementation for User Story 2

**模块批次 1：消息与事件 + 模型**
- [x] 014 [P] [US2] 实现 `examples/chat/`：流式对话 + 事件流分发（`EventType` match 各事件分支）
- [x] 015 [P] [US2] 编写 `docs/rust/zh/building-blocks/message-and-event.mdx`：Msg/ContentBlock(7 块)、AgentEvent(33 事件)、流式 delta；引用 examples/chat；标 L1/L2
- [x] 016 [P] [US2] 编写 `docs/rust/zh/building-blocks/model/overview.mdx`：ChatModel trait、credential、模型卡；注明仅 DashScope provider、无 TTS/Realtime
- [x] 017 [P] [US2] 编写 `docs/rust/zh/building-blocks/model/llm.mdx`：DashScopeChatModel 用法（stream/thinking）、ChatResponse；标 L2
- [x] 018 [P] [US2] 编写 `docs/rust/zh/building-blocks/model/embedding.mdx`：EmbeddingModel trait + DashScopeEmbeddingModel；标 L2
- [x] 019 [P] [US2] 编写 `docs/rust/zh/building-blocks/model/tts.mdx`：状态=计划中（Rust 无 TTS），说明 Python 侧 TTS 能力与 Rust 缺失范围，无 Rust 代码

**模块批次 2：工具系统**
- [x] 020 [P] [US2] 实现 `examples/tool/`：FunctionTool 自定义工具 + ToolKit 注册 + 内置工具（Bash/Read/Write/Edit/Grep/Glob/ResetTools/Skill，注明无 ListDir）
- [x] 021 [P] [US2] 编写 `docs/rust/zh/building-blocks/tool/overview.mdx`：Tool trait/ToolKit/ToolGroup 三概念；引用 examples/tool；标 L2
- [x] 022 [P] [US2] 编写 `docs/rust/zh/building-blocks/tool/python-tool.mdx`：状态=部分支持——等价能力为 FunctionTool（Rust 函数），非 Python 执行器；列已支持/缺失边界
- [x] 023 [P] [US2] 编写 `docs/rust/zh/building-blocks/tool/manage-tools.mdx`：ToolKit 注册/分组激活/ResetTools；标 L2
- [x] 024 [P] [US2] 实现 `examples/mcp/`：复用 `crates/agent_scope_mcp/examples/mcp_excalidraw_debug.rs` 形态——McpClient::connect + list_tools + call_tool，连一个 stdio MCP server
- [x] 025 [P] [US2] 编写 `docs/rust/zh/building-blocks/tool/mcp.mdx`：McpClient/McpTool/McpExt（客户端接入）；引用 examples/mcp；标 L2；注明无服务端 gateway
- [x] 026 [P] [US2] 实现 `examples/skill/`：SkillLoader + Skill 工具读取技能
- [x] 027 [P] [US2] 编写 `docs/rust/zh/building-blocks/tool/skill.mdx`：SkillViewer/LocalSkillLoader/SkillTool/SkillManager；引用 examples/skill；标 L2

**模块批次 3：Agent 编排 + 权限 + Middleware + Plan**
- [x] 028 [P] [US2] 实现 `examples/agent/`：AgentConfig 组装 + reply_stream + 权限确认（PermissionEngine）+ 中断恢复（JsonFileSessionStore）+ 任务规划工具
- [x] 029 [P] [US2] 编写 `docs/rust/zh/building-blocks/agent/overview.mdx`：Agent trait/ReActAgent/主循环；引用 examples/agent；标 L2
- [x] 030 [P] [US2] 编写 `docs/rust/zh/building-blocks/agent/configure-agent.mdx`：AgentConfig::builder（model/toolkit/permission/workspace/session/injection）；注明构造期配置
- [x] 031 [P] [US2] 编写 `docs/rust/zh/building-blocks/agent/run-agent.mdx`：reply/reply_stream/observe；标 L2；结构化输出如实标注「模型层支持、Agent 循环未接线」
- [x] 032 [P] [US2] 编写 `docs/rust/zh/building-blocks/agent/human-in-the-loop.mdx`：PermissionResult::RequireConfirm + RequireUserConfirmEvent；引用 examples/agent；标 L2
- [x] 033 [P] [US2] 编写 `docs/rust/zh/building-blocks/agent/interrupt-agent.mdx`：ReActAgent::interrupt + UserInterruptEvent；标 L2
- [x] 034 [P] [US2] 编写 `docs/rust/zh/building-blocks/permission-system/overview.mdx` + `permission-mode.mdx` + `permission-rule.mdx` + `tool-check.mdx`：PermissionEngine/5 模式/allow-deny-ask/决策矩阵；标 L2；注明比 Python 细粒度 admin policy 为部分覆盖
- [x] 035 [P] [US2] 编写 `docs/rust/zh/building-blocks/middleware.mdx`：Middleware trait 9 钩子 + MemoryMiddleware/RAGMiddleware；标 L2
- [x] 036 [P] [US2] 编写 `docs/rust/zh/building-blocks/plan.mdx`：任务规划工具 TaskCreate/List/Get/Update（Feature 024 替代 Planner）；引用 examples/agent；标 L2

**模块批次 4：记忆 + RAG + Context**
- [x] 037 [P] [US2] 实现 `examples/memory/`：FileMemory 读写 + TurbovecMemory 向量检索 + MemoryMiddleware
- [x] 038 [P] [US2] 编写 `docs/rust/zh/building-blocks/long-term-memory.mdx`：Memory/FileMemory/TurbovecMemory 双后端；引用 examples/memory；标 L2
- [x] 039 [P] [US2] 实现 `examples/rag/`：DashScopeEmbeddingModel + TurbovecVectorStore + KnowledgeBase + RAGMiddleware（Static/Agentic）
- [x] 040 [P] [US2] 编写 `docs/rust/zh/building-blocks/rag.mdx`：KnowledgeBase/Chunker/Parser/VectorStore/RAGMiddleware 两模式；引用 examples/rag；标 L2
- [x] 041 [P] [US2] 编写 `docs/rust/zh/building-blocks/context/overview.mdx` + `compress-context.mdx` + `offload-context.mdx` + `environment-awareness.mdx`：ContextConfig/offload/InjectionConfig(Feature 026)；标 L2；注明压缩策略为移除最旧+占位摘要（模型摘要 deferred）

**模块批次 5：Workspace + Sandbox + Console**
- [x] 042 [P] [US2] 实现 `examples/workspace/`：LocalWorkspace 文件操作 + workspace 绑定自动注入内置工具（Feature 029）
- [x] 043 [P] [US2] 编写 `docs/rust/zh/building-blocks/workspace/overview.mdx` + `manage-resources.mdx`（部分支持：无独立资源配额模型）+ `run-workspace.mdx`（引用 examples/workspace）；标 L2
- [x] 044 [P] [US2] 编写 `docs/rust/zh/building-blocks/workspace/mcp-gateway.mdx`：状态=部分支持——仅客户端接入（McpExt::connect_mcp），无服务端 gateway
- [x] 045 [P] [US2] 实现 `examples/sandbox/`：LocalSandboxSession 命令执行 + 路径防护（SandboxPathResolver）+ CapabilityReport
- [x] 046 [P] [US2] 编写 `docs/rust/zh/building-blocks/workspace/overview.mdx` 沙箱小节（或独立说明）+ 标注：Sandbox=本地隔离非 Docker；cpu/memory 限制本地后端不可强制
- [x] 047 [P] [US2] 编写 `docs/rust/zh/building-blocks/console.mdx`：状态=计划中（Rust 无库级 console；pi-rust 示例的 ratatui TUI 可作参考，说明运行方式）

**Checkpoint**: 全部已实现模块文档 + 示例完成；每个模块可按文档独立运行示例

---

## Phase 5: User Story 3 - 熟悉 docs/python 的用户一比一对照迁移 (Priority: P2)

**Goal**: 完整目录树一比一 + 全量状态标注（含未实现页），Python 用户可按相同路径对照迁移

**Independent Test**: 熟悉 docs/python 的用户按相同路径在 docs/rust 定位对应中文页；未实现页有统一状态标注、无伪 Rust 用法

### Implementation for User Story 3

- [x] T048 [P] [US3] 补齐全部**计划中**页面（无独立示例）：`deploy/agent-service.mdx`、`deploy/agent-team.mdx`（注明库级 SubAgent 部分支持）、`deploy/sharing.mdx`、`deploy/workspace-manager.mdx`（注明本地 WorkspaceManager）、`deploy/rag.mdx`（注明库级 RAGMiddleware）——每页：Python 能力简介 + Rust 缺失范围 + 替代能力链接，禁 Rust 代码
- [x] T049 [P] [US3] 补齐 `deploy/channel/` 五页（overview/custom/feishu/discord/routing）：全部计划中，同 T048 格式
- [x] T050 [P] [US3] 补齐 `deploy/hub/` 三页（overview/mcp-hub/skill-hub）：全部计划中，注明本地 SkillManager/McpRegistry 为替代
- [x] T051 [US3] 编写 `docs/rust/zh/others/change-log.mdx`：Python 2.0 vs 1.0 差异摘译 + 每项标注 Rust 对应状态
- [x] T052 [US3] 编写 `docs/rust/zh/others/faq.mdx`：面向 Rust 版的常见问题（版本差、provider 现状、TTS/console/deploy 缺失、sandbox 形态、MCP 客户端 vs gateway）
- [x] T053 [US3] 校验每页状态块与 mirror-map 登记一致；已实现页标注 compat_level 且与兼容性矩阵一致（宪法 §18）
- [x] T054 [US3] 校验每个页面存在「对应 docs/python 源页」引用（双向对照），计划中页无 rust 代码块

**Checkpoint**: 50 页全量一比一 + 状态标注完成；Python 用户可对照迁移

---

## Phase 6: User Story 4 - 维护者保持文档/示例/代码同步 (Priority: P3)

**Goal**: 编译锚点 + 结构漂移检测，使文档过期可自动化发现

**Independent Test**: 改动公开 API 后 CI 出现示例编译错误；docs/python 增删页面后 mirror-map 提示结构漂移

### Implementation for User Story 4

- [x] T055 [P] [US4] 验证 10 个示例 crate 全部通过 `cargo check --workspace --all-targets` 与 `cargo clippy --workspace --all-targets -D warnings`，CI 无新增失败
- [x] T056 [P] [US4] 建立结构 diff 校验：`find docs/python/zh -type f | sed 's|docs/python/zh|.|' | sort` vs `docs/rust/zh` 对比（忽略 openapi.json 例外），产出一键校验脚本 `scripts/check-docs-mirror.sh`（仓库根 `scripts/`）
- [x] T057 [P] [US4] 建立站内链接悬空检测：扫描 docs/rust/zh 全部 `.mdx` 的 `/versions/<ver>/zh/...` 链接，校验指向目标存在；产出脚本并入 T056
- [x] T058 [US4] 建立配置项一致性抽查：核对文档中环境变量名（DASHSCOPE_API_KEY 等）与 examples/ 代码实际读取一致；核对引用示例名与 examples/ 实际 crate 一致

**Checkpoint**: 维护者可通过脚本/CI 自动化发现文档过期与结构漂移

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 全量验证与收尾

- [x] T059 [P] 运行 quickstart.md 场景 A（镜像结构 diff 无差异）与场景 B（每页有状态块；计划中页无 rust 代码）
- [x] T060 [P] 运行 quickstart.md 场景 C（cargo check + clippy 全绿）与场景 D（文档引用示例名与实际一致）
- [x] T061 [P] 运行 quickstart.md 场景 E（无悬空链接；配置项一致）与场景 F（有凭据跑通 examples/quickstart，无凭据明确报错）
- [x] T062 [P] 更新 `docs/rust/mirror-map.md` 的生成日期与最终登记，确认 50 条与页面清单一致
- [x] T063 全量复核宪法 §17 完成定义：文档已更新、示例可编译、无未登记 UnsupportedFeature、无伪兼容

**Checkpoint**: 所有验证场景通过，feature 达成完成定义

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖，可立即开始
- **Foundational (Phase 2)**: 依赖 Setup 完成——BLOCKS 所有用户故事
- **US1 (Phase 3)**: 依赖 Foundational，无故事间依赖
- **US2 (Phase 4)**: 依赖 Foundational 完成；各模块批次间 [P] 可并行，批次内先后依赖
- **US3 (Phase 5)**: 依赖 Foundational 与 US2（未实现页需引用已实现页链接）；可与 US2 并行推进（不同页面）
- **US4 (Phase 6)**: 依赖 US2/US3 完成（示例与页面就绪后才能校验）
- **Polish (Phase 7)**: 依赖 US1-US4 全部完成

### User Story Dependencies

- **US1 (P1)**: 独立，MVP
- **US2 (P2)**: 独立于 US1，可并行（模块文档与示例）
- **US3 (P2)**: 独立于 US1/US2 主流程，未实现页与已实现页状态块并行推进
- **US4 (P3)**: 依赖 US1-US3 的产物就绪

### Within Each User Story

- 示例 crate 先于文档页（文档引用真实路径）
- 文档页状态标注必须与 mirror-map（T005）一致
- 每批完成即 `cargo check` 自检

### Parallel Opportunities

- Phase 1 的 T003/T004 可并行
- Phase 2 的 T007 可并行（README 独立）
- US2 各模块批次（消息事件/工具/Agent/记忆 RAG/Workspace 沙箱）5 批可并行，每批内 [P] 标记任务可并行
- US2 与 US3 可并行（不同页面）
- Phase 7 的 T059-T062 全部 [P] 可并行

---

## Parallel Example: US2 模块批次（示例 + 文档）

```bash
# 消息与事件批次：示例与文档并行
Task: "实现 examples/chat"（T014）
Task: "编写 message-and-event.mdx"（T015）

# 工具批次：三个示例与各自文档并行
Task: "实现 examples/tool"（T020）
Task: "实现 examples/mcp"（T024）
Task: "实现 examples/skill"（T026）

# Agent 批次：单个示例 + 多文档
Task: "实现 examples/agent"（T028）
Task: "编写 agent/overview.mdx"（T029）
Task: "编写 permission-system/*"（T034）
```

---

## Implementation Strategy

### MVP First (US1 Only)

1. 完成 Phase 1 Setup + Phase 2 Foundational（mirror-map 50 条）
2. 完成 Phase 3 US1：examples/quickstart + index + quickstart + release-notes
3. **STOP and VALIDATE**: 新用户按 quickstart 30 分钟跑通第一个 Agent（quickstart 场景 F）
4. 此时已交付「docs/rust 骨架 + 快速上手 + 版本声明」MVP

### Incremental Delivery

1. Setup + Foundational → 结构就位
2. US1 → 快速上手 MVP → 验证
3. US2 各模块批次 → 逐模块文档 + 示例 → 每批验证
4. US3 → 全量状态标注 + 未实现页 → 一比一完整
5. US4 → 维护者同步机制 → Polish 全量验证

### Parallel Team Strategy

1. 团队完成 Setup + Foundational
2. US1 单人；US2 按 5 批次分派 5 人并行；US3 并行补未实现页
3. US4 + Polish 收敛全量验证

---

## Notes

- [P] 任务 = 不同文件、无依赖
- [Story] 标签映射任务到用户故事（US1-US4）
- 每个用户故事独立可完成、可验证
- 文档页状态标注必须与 mirror-map 三方一致（页面/映射/兼容性矩阵）
- 计划中页严禁 Rust 代码（宪法 §5）
- Commit 按任务或逻辑组进行
- 任何 checkpoint 可暂停并独立验证该故事
