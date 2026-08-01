# Quickstart: 文档验证指南

**Date**: 2026-08-01 | **Feature**: specs/018-usage-docs

本指南定义 Feature 018 交付后的端到端验证场景。全部场景通过 = 文档特性完成（宪法第十七条适配版 DoD）。

## 前置条件

- 仓库根目录，Rust 工具链可用
- `.env` 含有效 `API_KEY`（仅场景 1 需要真实凭据）
- Feature 018 文档已全部落地 `docs/`

## 场景 1：新用户快速上手（对应 US1 / SC-001）

```bash
# 模拟新用户路径：仅按 docs/zh/getting-started.md 操作
cargo run --example chat
```

**预期**: 按文档步骤（含 `.env` 配置说明）能启动流式对话 Agent 并收到模型响应；未配置 `API_KEY` 时的报错与文档描述一致。英文版 `docs/en/getting-started.md` 步骤等价。

## 场景 2：示例编译锚定（对应 FR-006 / SC-003）

```bash
cargo build --examples && cargo test --examples
```

**预期**: 全绿。文档中引用的所有 examples/ 路径存在；抽查 3 处内联片段与其 `<!-- source: ... -->` 标注的真实代码一致。

## 场景 3：双语镜像与链接完整性（对应 FR-012 / SC-002 / SC-008）

```bash
# 目录树镜像
diff <(cd docs/zh && find . -type f | sort) <(cd docs/en && find . -type f | sort)
# 标题序列一致性（示例：tool 模块）
grep -c '^#' docs/zh/modules/tool.md && grep -c '^#' docs/en/modules/tool.md
# 站内相对链接抽查：从 docs/README.md 出发逐层点击无 404
```

**预期**: 目录树 diff 为空；双语对应文件标题数量与层级一致；代码块内容双语相同；无悬空链接。

## 场景 4：兼容性标注一致性（对应 FR-007 / SC-005）

```bash
# 抽查：文档宣称 supported 的能力不得处于矩阵的 NOT_ANALYZED 状态
jq '[.entries[] | select(.status!="IMPLEMENTED") | {id: .capability_id, status, category}]' \
  specs/001-compatibility-baseline/capability-matrix.json
# 人工对照：每篇模块文档第 6 章的等级(target_level)/偏差(notes)与矩阵记录一致
```

**预期**: 不存在文档宣称可用但矩阵标记 unsupported 的能力；偏差条目与矩阵一一对应；索引与迁移参考含上游版本锁定信息（release + commit）。

## 场景 5：配置项核对（对应 FR-011 / SC-007）

抽查模块文档中的环境变量名、构造参数、默认值，对照对应 crate 源码：

```bash
# 例：API_KEY 环境变量与 DashScopeChatModel::new 签名
grep -n "API_KEY" examples/*.rs
grep -n "pub fn new" crates/agent_scope_dashscope/src/*.rs
```

**预期**: 抽查项 100% 一致。

## 场景 6：迁移参考有效性（对应 US3 / SC-006）

人工验证：依据 `docs/zh/migration.md` 的 API 对照表，将一个 Python ReActAgent + 工具调用示例改写为 Rust（参照 `examples/verify_agent.rs`），不查阅 crate 源码。

**预期**: 对照表足以完成迁移；文档列出的行为差异与实际一致。

## 通过标准

| 场景 | 支撑 SC | 通过条件 |
|------|---------|---------|
| 1 | SC-001 | 30 分钟内跑通，报错描述准确 |
| 2 | SC-003, SC-004 | build+test 全绿，引用一致 |
| 3 | SC-002, SC-008 | 镜像 diff 空，无悬空链接 |
| 4 | SC-005 | 零冲突 |
| 5 | SC-007 | 抽查 100% 一致 |
| 6 | SC-006 | 迁移路径闭环 |
