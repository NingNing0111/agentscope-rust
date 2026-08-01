# Implementation Plan: AgentScope Rust 模块化使用文档

**Branch**: `018-usage-docs` | **Date**: 2026-08-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/018-usage-docs/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

为 AgentScope Rust（Feature 001-017 已完成：14 个 crate + 根 facade + 7 个示例）在仓库根目录 `docs/` 下编写中英双语、按模块组织的使用文档。交付物包括：双语索引入口、快速上手指南、12 个能力模块的双语使用文档、Python→Rust 迁移参考、至少 1 个端到端场景教程。文档示例锚定受 `cargo build --examples` 编译约束的示例代码，兼容性标注以 `specs/001-compatibility-baseline/capability-matrix.json` 为权威来源，保证文档不引入"伪兼容"。

## Technical Context

**Language/Version**: Markdown（文档）；示例代码 Rust 2024 edition（跟随 workspace）

**Primary Dependencies**: 无新增依赖；示例代码复用现有 examples/（chat、verify_agent、memory_test、rag_test、session_test、streaming_tool_test、common）

**Storage**: 文件系统（docs/ 目录，Markdown 文件，随 git 版本化）

**Testing**: `cargo build --examples`（示例编译验证）；`cargo test --examples`（示例行为测试，已存在）；文档交付前的手工验证清单（quickstart.md）

**Target Platform**: GitHub 仓库内 Markdown 渲染 + 本地文本阅读

**Project Type**: library（Rust workspace，本次为文档特性，无 crate 变更）

**Performance Goals**: N/A（文档特性）

**Constraints**:
- 不引入文档站点生成器（mdBook/Docusaurus），纯 Markdown + GitHub 渲染
- 不得破坏 `docs/superpowers/` 既有内容
- 双语版本结构一致、信息等价（FR-012）
- 文档 MUST NOT 宣称实际返回 `UnsupportedFeature` 的能力（宪法第五条延伸）

**Scale/Scope**:
- 14 个 crate（types/message/event/model/dashscope/tool/agent/memory/state/rag/embedding/workspace/sandbox/utils）+ 根 facade `agentscope`
- 12 个模块文档主题（FR-004 清单）× 2 语言 + 索引 × 2 + 快速上手 × 2 + 迁移参考 × 2 + 场景教程 ≥ 1 × 2
- 兼容性标注来源：`specs/001-compatibility-baseline/capability-matrix.json`（280 capabilities）
- 凭据配置事实：examples 通过 `dotenv` 加载 `.env`，环境变量名 `API_KEY`，经 clap `env` feature 注入；`DashScopeChatModel::new(api_key, model_name)` 显式传参

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 适用性 | 评估 | 状态 |
|------|--------|------|------|
| 第一条 兼容性优先 | 适用 | 文档描述的行为以代码实际行为与 capability-matrix.json 为准；FR-007 强制文档与兼容性矩阵一致 | ✅ |
| 第二条 锁定上游版本 | 适用 | 迁移参考与索引 MUST 记录当前锁定的上游版本（release + commit，引自 Feature 001 基线） | ✅ |
| 第三条 Python 是行为基准 | 适用 | 迁移参考的 API 对照与差异说明基于 Feature 001 的实测运行记录，非推测 | ✅ |
| 第四条 先契约后实现 | 适用 | 本特性以 contracts/ 先定义文档结构契约（模块文档模板、布局与双语约定），再进入撰写 | ✅ |
| 第五条 不允许伪兼容 | 适用 | FR-007：文档 MUST NOT 宣称 unsupported 能力；模块文档含"不支持的能力与已知限制"必备章节 | ✅ |
| 第六条 测试驱动兼容性 | 部分适用 | 文档示例 MUST 锚定受编译约束的示例代码（FR-006）；文档正确性由 quickstart.md 验证场景保证 | ✅ |
| 第七条 Trace 是核心验收产物 | 间接适用 | 事件与流式模块文档 MUST 覆盖事件顺序与 trace 语义（用户可观察行为） | ✅ |
| 第八条 Rust 原生设计 | 适用 | 文档使用 Rust 惯用术语（trait、Arc、Result、CancellationToken），不照搬 Python 概念 | ✅ |
| 第九条 安全 Rust 优先 | 间接适用 | 文档特性无代码变更；示例代码沿用现有受 `#![deny(unsafe_code)]` 约束的 examples | ✅ |
| 第十条 结构化并发 | 不适用 | 无并发代码变更 | N/A |
| 第十一条 分层与依赖方向 | 适用 | 模块文档按 crate 分层组织（基础类型 → 抽象 → Provider → Agent → 能力模块），与依赖方向一致 | ✅ |
| 第十二条 稳定数据协议 | 间接适用 | 消息模型文档覆盖序列化协议与未知字段处理的用户可见语义 | ✅ |
| 第十三条 稳定错误模型 | 适用 | 各模块文档的"不支持的能力与已知限制"/错误章节覆盖类型化错误与 `UnsupportedFeature` 语义 | ✅ |
| 第十四条 可观测性 | 不适用 | 无可观测性代码变更（模块文档可说明 tracing 能力，属内容而非实现） | N/A |
| 第十五条 性能不能牺牲正确性 | 不适用 | 无性能相关变更 | N/A |
| 第十六条 小步交付 | 适用 | 本特性为独立文档模块，依赖 Feature 001-017 已完成的全部能力 | ✅ |
| 第十七条 完成的定义 | 适用 | 文档特性适配版 DoD：双语完整、链接有效、示例编译、与兼容性矩阵一致、quickstart 验证场景全通过 | ✅ |
| 第十八条 兼容性分级 | 适用 | 每篇模块文档 MUST 标注 L1-L4 兼容等级（FR-007），来源 capability-matrix.json | ✅ |
| 第十九条 变更治理 | 不适用 | 无宪法违反 | N/A |

**Gate 结果**: 19/19 通过（3 项 N/A 为无代码变更的自然豁免），无违反，无需 Complexity Tracking 豁免。

## Project Structure

### Documentation (this feature)

```text
specs/018-usage-docs/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── documentation-layout.md
│   └── module-doc-template.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
docs/                              # 文档站点根（本特性唯一变更面）
├── README.md                      # 双语索引入口（导航 + 阅读顺序 + 上游版本锁定信息）
├── zh/                            # 中文文档树
│   ├── getting-started.md         # 快速上手（US1）
│   ├── migration.md               # Python→Rust 迁移参考（US3）
│   ├── modules/                   # 模块文档（US2，12 篇）
│   │   ├── message-types.md       # 基础类型与消息模型（types/message）
│   │   ├── event-streaming.md     # 事件与流式（event + streaming）
│   │   ├── model.md               # 模型抽象（model）
│   │   ├── dashscope.md           # DashScope Provider
│   │   ├── tool.md                # 工具系统（tool）
│   │   ├── agent.md               # Agent 系统（agent）
│   │   ├── memory.md              # 记忆（memory）
│   │   ├── session.md             # 会话管理（session/state）
│   │   ├── rag.md                 # RAG（embedding/rag/turbovec）
│   │   ├── workspace.md           # 工作空间（workspace）
│   │   ├── skill.md               # 技能（skill）
│   │   └── sandbox.md             # 沙箱（sandbox）
│   └── tutorials/                 # 场景教程（US4，≥1 篇）
│       └── rag-knowledge-chat.md  # RAG 知识库问答教程
├── en/                            # 英文文档树（与 zh/ 镜像，结构一致）
│   └── ...（同 zh/ 布局）
└── superpowers/                   # 既有内容，保持原样不动

examples/                          # 示例锚点（本特性不修改，文档引用）
├── chat.rs / common.rs / memory_test.rs / rag_test.rs
├── session_test.rs / streaming_tool_test.rs / verify_agent.rs
└── （实现阶段如模块文档需要新示例，按 tasks.md 在此新增，受 cargo build --examples 约束）
```

**Structure Decision**: 采用纯 Markdown 平面文件结构，`docs/zh/` 与 `docs/en/` 为镜像目录树（结构契约见 `contracts/documentation-layout.md`），`docs/README.md` 为双语总索引。模块文档统一遵循 `contracts/module-doc-template.md` 的结构契约。选择镜像目录而非 `<name>.zh.md` 后缀配对，原因：GitHub 渲染下目录内相对链接无需跨语言跳转、每语种可独立浏览整棵树、新增语种只需复制目录。文档特性无 crate/源码变更。

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

无违反项，本表留空。
