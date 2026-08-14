# Implementation Plan: docs/rust 项目文档一比一镜像 docs/python

**Branch**: `030-rust-docs-mirror` | **Date**: 2026-08-13 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/030-rust-docs-mirror/spec.md`

## Summary

在 `docs/rust/` 下为 AgentScope Rust 建立一套**中文**使用文档，目录树与 `docs/python/zh` 一比一镜像（50 个页面），每个页面如实标注对应能力在 Rust 中的实现状态（已实现/部分支持/计划中），禁止伪兼容。已实现模块的文档通过 `examples/` 下的 workspace 成员示例 crate 提供可编译验证的代码示例（每模块一个），由 CI 的 `cargo check --workspace --all-targets` 自动编译校验。

关键事实：兼容基线为 AgentScope Python **v2.0.5**（commit `27b6a0d2`），而 `docs/python` 是 **2.0.7dev** 的 Mintlify 文档——镜像时须在文档中注明版本差，2.0.7dev 新增而 2.0.5/Rust 未实现的能力一律标注「计划中」。文档本身使用 `.mdx`（Mintlify 组件），仅交付 `zh/`，`en/` 留待未来对称补充。

## Technical Context

**Language/Version**: Rust，edition 2024（`workspace.package.edition = "2024"`），stable 工具链（CI 用 `dtolnay/rust-toolchain@stable`）。

**Primary Dependencies**:
- 文档格式：`.mdx`（与 `docs/python` 同款 Mintlify 语法：frontmatter、`CardGroup`/`Card`/`Note`/`Tip`/`Accordion`/`Tree`/`Steps`/`Frame`/`Badge`、mermaid 图、版本化站内链接 `/versions/<ver>/zh/...`）。
- 示例 crate：复用 workspace 已有依赖（`tokio`、`serde`/`serde_json`、`schemars`、`async-trait`、`futures`、`dotenv`、`clap`）与 14 个 `agent_scope_*` crate。示例各自声明最小依赖，不引入非必要重依赖。
- 无新第三方依赖需求；若 sandbox/rag 示例确需，优先复用 `agent_scope_*` 内置能力。

**Storage**: N/A（文档无运行时存储）。示例可选的运行时产物（会话目录、Memory 文件）落在示例自己的 workdir，不落库。

**Testing**: `cargo test --workspace`、`cargo clippy --workspace --all-targets -D warnings`、`cargo fmt --all -- --check`。关键约束：`examples/` 下新增示例 crate 必须被 `cargo check --workspace --all-targets` 覆盖（加入根 `Cargo.toml` 的 `[workspace] members`），使示例成为文档正确性的编译锚点。文档内部不含可执行代码断言（无 doctest 依赖），正确性完全由 examples/ 编译校验背书。

**Target Platform**: 文档站（Mintlify 兼容 `.mdx`，供未来文档站部署）；示例面向 ubuntu/macos/windows（CI 三平台矩阵均需通过 `cargo check`）。

**Project Type**: 文档交付 + workspace 下的多个示例 crate（library workspace 的用户文档层）。非 web 服务、非 CLI 工具本体。

**Performance Goals**: N/A（文档）。约束为 CI 成本：每个示例 crate 应保持轻量，不得显著拖慢 workspace 编译。

**Constraints**:
- 宪法第一条（兼容性优先）：文档描述的行为必须与 Python 参考实现（2.0.5 基线、2.0.7dev 文档）一致，偏差须标注。
- 宪法第五条（不允许伪兼容）：未实现/部分支持能力必须以状态标注如实呈现，MUST NOT 出现伪造的 Rust 用法。
- 宪法第十八条（兼容性分级 L1-L4）：每篇已实现模块文档标注兼容等级。
- `docs/` 是独立嵌套 git 仓库：文档工作只新增 `docs/rust/`，不得改动 `docs/python/` 或破坏嵌套仓库结构。
- 兼容基线 v2.0.5 与镜像源 2.0.7dev 的版本差，必须在索引/页面中说明。

**Scale/Scope**: 50 个中文页面（`docs/rust/zh/`，目录树镜像 `docs/python/zh`）+ 10 个示例 crate（`examples/quickstart|chat|tool|mcp|skill|agent|memory|rag|workspace|sandbox`）+ 1 份镜像映射清单。`docs/rust/en/` 本期不创建。

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

本 feature 为文档交付，逐条对照宪法：

| 条款 | 判定 | 说明 |
|------|------|------|
| §1 兼容性优先 | ✅ 通过 | 文档描述的行为以 Python 参考实现为准；版本差（2.0.5 基线 vs 2.0.7dev 镜像源）在文档中显式声明 |
| §2 锁定上游版本 | ✅ 通过 | 镜像源锁定 docs/python（2.0.7dev），兼容基线锁定 v2.0.5（commit `27b6a0d2`），均在 plan/文档中记录 |
| §3 Python 是行为基准 | ✅ 通过 | 已实现页面内容必须与 Python 实际行为一致，不得凭猜测杜撰 |
| §5 不允许伪兼容 | ✅ 通过（核心 GATE） | 未实现能力以「计划中」状态标注呈现，禁止伪造 Rust 用法；部分支持须给出边界 |
| §16 小步交付 | ✅ 通过 | 示例按模块逐个交付（10 个独立 crate），每篇文档独立可验证 |
| §17 完成的定义 | ✅ 通过（核心 GATE） | 示例可编译（CI check/clippy）、文档已更新、无未登记 UnsupportedFeature |
| §18 兼容性分级 | ✅ 通过 | 每篇已实现模块文档标注 L1-L4 等级 |
| §4/§6/§7/§10-§15 | ✅ 不适用或通过 | 条款针对代码实现/运行时；示例代码仍须遵守 §9 安全 Rust（`#![deny(unsafe_code)]`）、§13 typed errors，但文档正文不受影响 |
| §8 Rust 原生设计 | ✅ 通过 | 示例代码须体现 Rust 原生 API（`Arc<dyn ChatModel>` 等），不得机械照搬 Python 示例 |

无违规项，无需 Complexity Tracking 豁免。Phase 1 设计完成后将重评。

## Constitution Check 重评（Phase 1 设计后）

设计工件（research.md / data-model.md / contracts/ / quickstart.md）已生成，重评各 GATE：

| 条款 | 重评 | 设计后证据 |
|------|------|-----------|
| §1 兼容性优先 | ✅ 通过 | research R3 锁定版本差；已实现页内容以 Python v2.0.5 行为基准 |
| §2 锁定上游版本 | ✅ 通过 | research R3：镜像源 2.0.7dev + 兼容基线 2.0.5 commit 双锁定，文档索引页声明 |
| §5 不允许伪兼容 | ✅ 通过（核心 GATE） | contracts/doc-page-contract 定义三档状态块 + 计划中页禁 Rust 代码；research R1 逐模块判定真实状态 |
| §6 测试驱动兼容性 | ✅ 通过 | contracts/example-crate-contract：示例全过 check+clippy（编译锚点）；示例凭据缺失明确报错 |
| §9 安全 Rust | ✅ 通过 | 示例契约强制 `#![deny(unsafe_code)]`、typed errors |
| §13 稳定错误模型 | ✅ 通过 | 示例契约禁止对用户输入 unwrap/panic |
| §16 小步交付 | ✅ 通过 | 10 个示例独立 crate，逐个可验证 |
| §17 完成的定义 | ✅ 通过 | quickstart.md 场景 A-F 覆盖编译/文档/示例全部验收 |
| §18 兼容性分级 | ✅ 通过 | doc-page-contract 要求已实现页标注 L1-L4 |

**结论**：设计后无新增违规。核心保证——「未实现能力以状态标注如实呈现、示例可编译、无伪兼容」——通过 doc-page-contract、example-crate-contract、quickstart 验证场景闭环落实。

## Project Structure

### Documentation (this feature)

```text
specs/030-rust-docs-mirror/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
docs/rust/                          # 本期交付根（新增，不触碰 docs/python）
└── zh/
    ├── index.mdx                   # 索引入口（含镜像版本声明、能力总览、实现状态索引）
    ├── quickstart.mdx              # 快速上手（引用 examples/quickstart）
    ├── release-notes.mdx           # 版本历史（源自 CHANGELOG.md）
    ├── building-blocks/
    │   ├── agent/                  # overview / configure-agent / run-agent / human-in-the-loop / interrupt-agent
    │   ├── console.mdx             # 计划中（Rust 无 console 模块，pi-rust TUI 可作参考）
    │   ├── context/                # overview / compress-context / environment-awareness / offload-context
    │   ├── long-term-memory.mdx    # 引用 examples/memory
    │   ├── message-and-event.mdx   # 引用 examples/chat（事件流）
    │   ├── middleware.mdx          # 已实现（Middleware trait 9 hooks）
    │   ├── model/                  # overview / llm / embedding / tts
    │   ├── permission-system/      # overview / permission-mode / permission-rule / tool-check
    │   ├── plan.mdx                # 任务规划工具（Feature 024 替代 Planner）
    │   ├── rag.mdx                 # 引用 examples/rag
    │   ├── tool/                   # overview / python-tool / mcp / skill / manage-tools
    │   └── workspace/              # overview / manage-resources / mcp-gateway / run-workspace
    ├── deploy/
    │   ├── agent-service.mdx       # 计划中（无 FastAPI 后端）
    │   ├── agent-team.mdx          # 计划中（无 service 级团队编排；SubAgent 属库能力）
    │   ├── channel/                # overview / custom / feishu / discord / routing —— 全部计划中
    │   ├── hub/                    # overview / mcp-hub / skill-hub —— 全部计划中
    │   ├── rag.mdx                 # 计划中（无服务化 RAG；库级 RAG 见 building-blocks/rag）
    │   ├── sharing.mdx             # 计划中
    │   └── workspace-manager.mdx   # 计划中（无多租户 WorkspaceManager；见 workspace 文档边界）
    └── others/
        ├── change-log.mdx          # Python 2.0 vs 1.0 差异摘译，标注 Rust 对应状态
        └── faq.mdx                # 常见问题（面向 Rust 版）

docs/rust/mirror-map.md             # 镜像映射清单（50 页 ↔ docs/python 源 ↔ 状态 ↔ 引用示例）

examples/                           # 新增 10 个 workspace 成员 crate（pi-rust 模式）
├── quickstart/                     # 最小 Agent：凭据 + ChatModel + Toolkit + reply/reply_stream
├── chat/                           # 流式对话 + 事件流分发（EventType match）
├── tool/                           # FunctionTool 自定义工具 + ToolKit + 内置工具
├── mcp/                            # MCP stdio server 连接 + McpTool 调用（参考 mcp_excalidraw_debug）
├── skill/                          # SkillLoader + Skill 工具读取技能
├── agent/                          # Agent 编排 + 权限/人工确认 + 中断恢复
├── memory/                         # FileMemory / TurbovecMemory 读写与检索
├── rag/                            # KnowledgeBase + RAGMiddleware（Static/Agentic）
├── workspace/                      # LocalWorkspace 文件操作 + 内置工具注入
└── sandbox/                        # SandboxSession 命令执行 + 路径防护
```

**Structure Decision**: 采用「文档站镜像树 + workspace 示例 crate」双层结构。文档树严格镜像 `docs/python/zh` 保证一比一导航对照；示例按能力模块独立成 crate，登记进根 `Cargo.toml` 的 `[workspace] members`（与 `examples/pi-rust` 同模式），从而被 CI 自动编译校验并可按 `-p <name>` 单独运行。

## Complexity Tracking

无宪法违规，不适用。

> 填充条件：仅当 Constitution Check 存在需正当化的违规时填写。当前无违规。
