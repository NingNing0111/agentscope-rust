# Contract: 文档站点布局与双语约定

**Version**: 1.0.0 | **Date**: 2026-08-01 | **Feature**: specs/018-usage-docs

本契约为 Feature 018 全部文档文件的结构约束。实现阶段（tasks.md）与验收（quickstart.md）必须逐条核对。

## 1. 目录布局

```text
docs/
├── README.md            # 双语总索引（中英并列书写）
├── zh/
│   ├── getting-started.md
│   ├── migration.md
│   ├── modules/
│   │   ├── message-types.md
│   │   ├── event-streaming.md
│   │   ├── model.md
│   │   ├── dashscope.md
│   │   ├── tool.md
│   │   ├── agent.md
│   │   ├── memory.md
│   │   ├── session.md
│   │   ├── rag.md
│   │   ├── workspace.md
│   │   ├── skill.md
│   │   └── sandbox.md
│   └── tutorials/
│       └── rag-knowledge-chat.md
├── en/                  # 与 zh/ 完全镜像
└── superpowers/         # 既有内容，本特性只读
```

- **L-1**: `zh/` 与 `en/` 的相对路径集合 MUST 完全相同（机械可验证：`diff <(cd docs/zh && find . -type f | sort) <(cd docs/en && find . -type f | sort)` 为空）。
- **L-2**: 新增模块文档 MUST 同时落地双语双文件；不允许单语种文件长期存在。
- **L-3**: `docs/superpowers/` MUST NOT 被修改、移动或链接为文档站点内容。

## 2. 文件命名

- **N-1**: 全小写 kebab-case，扩展名 `.md`；双语对应文件同名（语种由目录区分，不带 `.zh`/`.en` 后缀）。
- **N-2**: 模块文档文件名 MUST 与上表清单一致；新增主题需先更新本契约。

## 3. 链接规则

- **K-1**: 站内链接 MUST 使用相对路径，MUST NOT 使用含分支名的 GitHub 绝对 URL。
- **K-2**: 同语种树内直接相对链接；跨语种跳转 MUST 经 `docs/README.md`，页面内 MAY 在页首提供对应语种链接（如 `English: ../en/modules/tool.md`）。
- **K-3**: 示例代码引用 MUST 使用仓库根相对路径（如 `examples/chat.rs`），并标注行区间或函数名。
- **K-4**: 引用 specs/ 下工件（capability-matrix.json 等）MAY 使用仓库根相对路径。

## 4. 双语一致性

- **B-1**: 双语对应文件的一级、二级标题序列 MUST 一致（标题文字不同，数量与顺序相同）。
- **B-2**: 代码块内容 MUST 双语完全相同（代码不翻译；注释可翻译）。
- **B-3**: 任一语种更新 MUST 同步更新对应文件；无法同步时 MUST 在未更新版本页首标注"待同步"状态。

## 5. 兼容性标注格式

每篇模块文档的兼容性章节 MUST 包含：

```markdown
## 兼容性 / Compatibility

- **兼容等级**: L1-L4 之一
- **权威来源**: specs/001-compatibility-baseline/capability-matrix.json
- **已知偏差**: （无偏差时写"无"；有偏差时逐条列出，格式：能力名 — 偏差描述 — 原因）
- **不支持的能力**: （返回 UnsupportedFeature 的能力清单及错误类型）
```

- **C-1**: 等级与偏差 MUST 与 capability-matrix.json 一致，MUST NOT 出现矩阵未记录的内容。
- **C-2**: 索引与迁移参考 MUST 记录上游版本锁定信息（release version + commit hash）。

## 6. 代码示例格式

- **E-1**: 完整示例 MUST 引用 examples/ 文件路径；内联片段 MUST ≤20 行且上方以 HTML 注释标注来源：`<!-- source: examples/chat.rs:L100-L120 -->`。
- **E-2**: 示例中的环境变量、参数名、默认值 MUST 与代码实际一致（撰写时对照源码核实）。
- **E-3**: 示例 MUST NOT 包含真实 API key；统一使用 `API_KEY` 环境变量或 `sk-...` 占位。

## 7. 内容边界

- **X-1**: 文档面向使用者，MUST NOT 描述内部私有实现细节；用户可观察的行为语义（事件顺序、取消、超时、错误分类）属于正当内容。
- **X-2**: MUST NOT 宣称 matrix 标记为 unsupported 的能力可用（宪法第五条延伸）。
- **X-3**: 未实现模块（Multi-agent、Distributed runtime）仅在索引"规划中"章节出现，不撰写使用文档。
