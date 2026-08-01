# Data Model: AgentScope Rust 模块化使用文档

**Date**: 2026-08-01 | **Feature**: [spec.md](./spec.md)

本特性为文档特性，"数据模型"描述文档站点的结构实体及其约束。所有实体均为 Markdown 文件或文件间关系，随 git 版本化。

## 实体一览

```text
DocSite (docs/)
├── Index (docs/README.md)                    1
├── LanguageTree ×2 (docs/zh/, docs/en/)      镜像对
│   ├── GettingStarted                        每语种 1
│   ├── ModuleDoc ×12 (modules/)              每语种 12
│   ├── MigrationGuide                        每语种 1
│   └── Tutorial ≥1 (tutorials/)              每语种 ≥1
├── CodeExample（锚点，位于 examples/）        被引用，非 docs/ 内容
└── CompatibilityAnnotation（章节，嵌于 ModuleDoc/Index/MigrationGuide）
```

## 实体定义

### DocSite（文档站点）

`docs/` 目录整体。

| 字段 | 约束 |
|------|------|
| 位置 | 仓库根 `docs/` |
| 组成 | README.md ×1 + zh/ + en/ + 既有无关内容（superpowers/，只读） |
| 版本化 | 随 git，与代码同仓库 |

**Validation**: 既有 `docs/superpowers/` 内容不被修改（FR-001）。

### Index（索引入口）

`docs/README.md`，双语书写（同一文件内中英并列）。

| 字段 | 约束 |
|------|------|
| 语言 | 中英双语（FR-012 的"索引双语"） |
| 内容 | 项目简介、上游版本锁定信息（release + commit）、阅读顺序、zh/ 与 en/ 入口链接、规划中模块声明（Multi-agent、Distributed runtime） |
| 链接 | 指向两个 LanguageTree 的入口文档 |

**Validation**: 无悬空链接（SC-002）；上游版本信息与 Feature 001 基线一致（宪法第二条）。

### LanguageTree（语种文档树）

`docs/zh/` 与 `docs/en/`，互为镜像。

| 字段 | 约束 |
|------|------|
| 结构 | 两树文件路径集合完全相同（getting-started.md、migration.md、modules/*.md、tutorials/*.md） |
| 内容 | 对应文件章节结构一一对应、信息等价（FR-012） |

**Validation**: 目录树 diff 为空；对应文件的一级/二级标题序列一致（SC-008）。

### GettingStarted（快速上手指南）

每语种 1 篇（`zh/getting-started.md`、`en/getting-started.md`）。

| 字段 | 约束 |
|------|------|
| 覆盖 | 环境准备、依赖引入、凭据配置（`API_KEY` / `.env` / dotenv，事实见 research.md D7）、第一个流式对话 Agent、错误排查、下一步导航 |
| 示例锚点 | `examples/chat.rs`、`examples/common.rs` |

**Validation**: 新用户 30 分钟跑通（SC-001）；配置项与代码一致（SC-007）。

### ModuleDoc（模块文档）

每语种 12 篇，位于 `<lang>/modules/`。

| 字段 | 约束 |
|------|------|
| 主题清单 | message-types、event-streaming、model、dashscope、tool、agent、memory、session、rag、workspace、skill、sandbox |
| 结构 | 遵循 `contracts/module-doc-template.md` 的 7 章节契约 |
| crate 映射 | types/message→message-types；event→event-streaming；model→model；dashscope→dashscope；tool→tool；agent→agent；memory→memory；state→session（并入）；embedding/rag→rag；workspace→workspace；skill→skill；sandbox→sandbox；utils→不单独成文 |

**Validation**: 每篇含 ≥1 端到端可运行示例（SC-004）；兼容性章节与矩阵一致（SC-005）；12 篇齐整（SC-002）。

### MigrationGuide（迁移参考）

每语种 1 篇（`<lang>/migration.md`）。

| 字段 | 约束 |
|------|------|
| 内容 | Python→Rust 主要公开 API 对照表、行为差异、各模块 L1-L4 等级、已知偏差、上游版本锁定信息 |
| 来源 | capability-matrix.json + Feature 001 实测记录（research.md D4） |

**Validation**: 覆盖 ReActAgent + 工具调用迁移路径（SC-006）；偏差条目与矩阵一一对应（SC-005）。

### Tutorial（场景教程）

每语种 ≥1 篇，位于 `<lang>/tutorials/`。首篇：`rag-knowledge-chat.md`（RAG 知识库问答，串联 rag + agent + memory + session）。

| 字段 | 约束 |
|------|------|
| 内容 | 场景目标、前置条件（凭据/数据/成本说明）、分步构建、完整产物指向 examples/ 锚点 |
| 串联模块 | ≥2 个 ModuleDoc 主题 |

**Validation**: 教程产物可编译运行（SC-003/FR-009）。

### CodeExample（代码示例锚点）

`examples/` 下受编译约束的 `.rs` 文件，被文档引用。

| 字段 | 约束 |
|------|------|
| 编译约束 | `cargo build --examples` / `cargo test --examples` 通过 |
| 引用方式 | 文档标注仓库根相对路径 + 行区间或函数名（research.md D3） |
| 内联片段 | ≤20 行，抽取自真实 examples 代码，上方注释标注来源 |

**Validation**: 文档交付时 `cargo build --examples` 全绿（SC-003）。

### CompatibilityAnnotation（兼容性标注）

嵌于 ModuleDoc 第⑥章、Index、MigrationGuide 的章节级实体。

| 字段 | 约束 |
|------|------|
| 等级 | L1 / L2 / L3 / L4 之一（宪法第十八条定义） |
| 偏差 | 逐条对应 capability-matrix.json 记录，含原因 |
| 权威源 | `specs/001-compatibility-baseline/capability-matrix.json`（引用，非复制为第二事实源） |

**Validation**: 文档宣称的能力 ∩ 矩阵非 `IMPLEMENTED` 状态（如 `NOT_ANALYZED`）的条目集合 = ∅（SC-005）。矩阵条目结构：`.entries[]`，含 `capability_id`、`category`、`status`、`target_level`、`notes` 字段。

## 关系与一致性规则

1. **镜像规则**: `zh/` 与 `en/` 文件路径集合恒等；对应文件标题序列恒等（SC-008）。
2. **锚定规则**: ModuleDoc/GettingStarted/Tutorial → CodeExample 为多对多引用；任一引用路径失效即视为缺陷。
3. **溯源规则**: CompatibilityAnnotation → capability-matrix.json 为多对一溯源；标注内容不得超出矩阵记录范围。
4. **导航规则**: Index → LanguageTree → ModuleDoc 单向导航；ModuleDoc 之间经"相关模块"章节互链；跨语种跳转仅经 Index。
5. **只读规则**: `docs/superpowers/` 及 specs/ 下各 feature 目录对本特性只读。
