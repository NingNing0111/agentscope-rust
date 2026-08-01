# Research: AgentScope Rust 模块化使用文档

**Date**: 2026-08-01 | **Feature**: [spec.md](./spec.md)

本文件汇总 Phase 0 的全部技术决策。本特性为纯文档特性，无代码实现未知项；研究重点为文档组织、示例锚定、双语维护与兼容性标注的可持续机制。

## D1: 文档载体 —— 纯 Markdown + GitHub 渲染，不引入站点生成器

- **Decision**: 文档为仓库内纯 Markdown 文件，存放于 `docs/`，依赖 GitHub 原生渲染与本地编辑器阅读，不引入 mdBook/Docusaurus/VitePress。
- **Rationale**: 项目当前未发布文档站点；引入生成器会新增工具链依赖、CI 构建步骤与主题维护成本，违背"轻量路径"原则。spec Assumptions 已明确此默认。后续如需站点，可将 Markdown 源无缝接入生成器（平面 Markdown 是所有生成器的通用输入）。
- **Alternatives considered**: mdBook（Rust 生态主流，但引入 SUMMARY.md 双重维护与构建步骤）；docs.rs rustdoc（覆盖 API 参考而非分模块使用文档，二者互补不冲突——本特性聚焦使用文档，rustdoc 注释维持现状）。

## D2: 双语组织 —— `docs/zh/` 与 `docs/en/` 镜像目录树

- **Decision**: 中文与英文各为完整目录树，结构一一镜像；`docs/README.md` 为双语总索引，承担语言选择与全站导航。
- **Rationale**: (1) GitHub 渲染下目录内相对链接无需跨语言跳转，链接断裂风险减半；(2) 单语种读者可完整浏览自己的树；(3) 双语结构一致性可通过目录树 diff 机械验证（`diff <(cd zh && find .) <(cd en && find .)`）；(4) 未来新增语种零结构调整。
- **Alternatives considered**: 同目录 `<name>.zh.md` / `<name>.en.md` 后缀配对（文件相邻便于同步，但单语种浏览混杂、索引链接必须带语种后缀、目录文件数翻倍）；单文件双语混排（违反 FR-012 的"双文件"决策，且阅读体验差）。

## D3: 代码示例锚定 —— 引用受编译约束的 examples/，内联片段标注来源

- **Decision**: 文档中的完整示例 MUST 引用 `examples/` 下受 `cargo build --examples` / `cargo test --examples` 约束的文件（标注文件路径与行区间或函数名）；文档内联的最小片段（≤20 行）允许直接书写，但 MUST 来自对应 examples 文件的真实代码抽取，并在示例上方以注释行标注来源路径。
- **Rationale**: FR-006 要求"API 变更后过期示例能被发现"。仓库已有 `[[example]]` 显式声明 + `autoexamples = false`，示例编译已被 CI 约束；文档复用该约束即获得免费的过期检测，无需引入 doctest 抽取工具（Markdown 内代码块无法直接参与 cargo 编译）。
- **Alternatives considered**: 全部示例改为 doctest（需将示例移入 crate doc comment，改动面大且与现有 examples 体系重复）；引入 `markdown-test`/`skeptic` 类工具（新增依赖与 CI 步骤，收益与 D3 方案相同）；不做验证（违反 FR-006）。

## D4: 兼容性标注 —— capability-matrix.json 为唯一权威源，文档引用而非复制

- **Decision**: 模块文档的"兼容性"章节标注 L1-L4 等级与已知偏差，权威来源为 `specs/001-compatibility-baseline/capability-matrix.json`（280 条目，字段：`capability_id`/`category`/`status`/`target_level`/`notes`）；文档中注明该文件为权威来源，偏差条目逐条对应矩阵记录，不凭空新增。索引与迁移参考记录锁定的上游版本（release + commit，引自 Feature 001 基线数据）。
- **Rationale**: 宪法第二条/第十八条要求兼容性声明可追溯到锁定基线；单一权威源避免文档与矩阵漂移（SC-005 零冲突目标）。
- **已知数据问题（2026-08-01 验证）**: 矩阵全部 280 条目的 `status` 字段停留为 `NOT_ANALYZED`，未随 Feature 001-017 的完成而更新——这与宪法第一条"兼容性矩阵 MUST 在每个功能发布时更新"存在落差。应对策略：文档标注等级时以 `target_level` + 各 feature spec 声明的目标等级 + 代码实际实现状态三者交叉核实为准；矩阵 `status` 字段的陈旧状态在迁移参考中如实注明，不替矩阵编造实现状态。矩阵回填不属于本特性范围。
- **Alternatives considered**: 在文档中独立维护兼容性表（必然漂移）；在 CI 中从矩阵自动生成文档片段（引入生成步骤，与 D1 的纯 Markdown 决策冲突，留作后续优化）；先回填矩阵再写文档（扩大本特性范围，违反小步交付，记录为后续任务）。

## D5: 模块文档划分 —— 12 篇主题文档映射 14 crate + facade

- **Decision**: 按用户可见能力而非 crate 边界划分 12 篇模块文档（见 plan.md Project Structure）：`state` 并入 session 文档（Session 持久化的状态层），`utils` 不单独成文（内部基础设施，相关内容散见各模块），`embedding`/`rag`/turbovec 合并为一篇 rag.md（用户视角的完整 RAG 链路），根 facade `agentscope` 的用法在 getting-started 与索引中说明。
- **Rationale**: FR-004 的清单即按能力划分；crate 拆分是实现细节（宪法第十一条），用户按"我要用 RAG"而非"我要用 agent_scope_embedding"查找文档。
- **Alternatives considered**: 每 crate 一篇（14 篇，rag 链路被拆散到 3 篇，用户需自行拼接）；按 Feature 编号划分（001-017 是项目管理视角，非用户视角）。

## D6: 模块文档统一结构 —— 7 章节模板

- **Decision**: 每篇模块文档遵循 7 章节固定结构（契约见 `contracts/module-doc-template.md`）：① 模块概述 ② 核心概念与主要公开类型 ③ 快速示例 ④ 关键用法模式 ⑤ 错误与不支持的能力 ⑥ 兼容性（L1-L4 等级 + 已知偏差）⑦ 相关模块链接。
- **Rationale**: FR-005 要求统一结构；将"错误与不支持的能力"与"兼容性"独立成章，直接支撑宪法第五条（反伪兼容）与第十八条（分级标注）的文档落地，使 SC-005 可逐章检查。
- **Alternatives considered**: 自由结构（每篇质量参差，SC-005 无法机械核对）；按 Python 官方文档章节照搬（Python 文档面向动态类型 API，与 Rust trait 体系不匹配，违反宪法第八条精神）。

## D7: 配置项事实核对 —— 已验证的凭据与配置事实

- **Decision**: 快速上手与 dashscope 文档使用以下已核对事实（2026-08-01 对照代码验证）：
  - 环境变量名：`API_KEY`（examples 通过 `dotenv` 加载 `.env`，clap `env` feature 注入 CLI 参数）
  - 模型构造：`DashScopeChatModel::new(api_key, model_name)`，API key 显式传参，crate 内不自行读取环境变量
  - Embedding：`DashScopeEmbedding::new(api_key, model_card)`
  - API key 为空时 DashScope 侧返回错误（已有测试 `test_dashscope_missing_api_key`）
- **Rationale**: FR-011/SC-007 要求文档配置项与代码 100% 一致；在 plan 阶段固化事实，避免撰写时凭记忆杜撰。撰写其余模块文档时，每个配置项 MUST 以同样方式对照代码核实。
- **Alternatives considered**: 撰写完成后再统一核对（返工成本高）；文档不覆盖配置项（违反 FR-003）。

## D8: 链接与导航约定

- **Decision**: 所有站内链接使用相对路径（同语种树内直接相对；`docs/README.md` 双语并列给出两个入口）；模块文档末尾"相关模块"章节链接同语种的相邻模块；示例引用使用仓库根相对路径（如 `examples/chat.rs`）以便本地与 GitHub 双环境可读。
- **Rationale**: 相对链接在 GitHub 渲染与本地编辑器中均有效；双语树隔离后不存在跨语种相对链接需求（跨语种跳转统一经 `docs/README.md`）。
- **Alternatives considered**: 绝对 URL（绑定 GitHub 默认分支名，fork 与本地阅读断裂）；无导航约定（索引悬空链接风险，SC-002 不可测）。

## 结论

全部 8 项决策落定，Technical Context 无遗留 NEEDS CLARIFICATION，可进入 Phase 1 设计。
