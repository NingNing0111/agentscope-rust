# Quickstart 验证指南（Feature 030）

**目的**: 提供可运行的端到端验证场景，证明「docs/rust 一比一镜像 docs/python + 示例可编译」这一 feature 按要求完成。实现细节见 tasks.md；本文档为验证/运行指南。

## 前置条件

- Rust stable 工具链（与 CI `dtolnay/rust-toolchain@stable` 一致）
- 仓库已 checkout 到 `030-rust-docs-mirror` 分支
- 可选：`DASHSCOPE_API_KEY`（仅运行真实模型示例时需要）

## 验证场景

### 场景 A：镜像结构一比一

```bash
# 1. 对比 docs/rust/zh 与 docs/python/zh 的目录树（忽略 openapi.json 例外）
find docs/python/zh -type f | sed 's|docs/python/zh|.|' | sort > /tmp/py.txt
find docs/rust/zh  -type f | sed 's|docs/rust/zh|.|'  | sort > /tmp/rs.txt
diff /tmp/py.txt /tmp/rs.txt   # 预期：无输出（完全一致）
```

**预期**: `diff` 无差异；`docs/rust/mirror-map.md` 覆盖 50 页并登记 openapi.json 例外。

### 场景 B：全页状态标注与无伪兼容

```bash
# 2. 每个 .mdx 页面都含统一状态块（已实现/部分支持/计划中）
grep -L "Rust 实现状态" docs/rust/zh/**/*.mdx docs/rust/zh/*.mdx 2>/dev/null
# 预期：无输出（每页均有状态块）

# 3. 计划中页面不得出现 Rust 代码块（伪兼容检查）
#    计划中页面（agent-service/channel/hub/console 等）中不应有 ```rust 或 rust 代码片段
```

**预期**: 每页均有状态块；计划中页面无伪造 Rust 用法。

### 场景 C：示例可编译（核心锚点）

```bash
# 4. 示例全部注册为 workspace 成员并通过编译
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

**预期**: 10 个新示例 crate 均通过 `check` 与 `clippy`；CI 无新增失败。

### 场景 D：文档↔示例绑定一致

```bash
# 5. 文档中引用的每个 `cargo run -p <name>` 对应真实示例 crate
grep -rhoE "cargo run -p [a-z-]+" docs/rust/zh/ | sort -u
# 对照 examples/ 下实际 crate 名，预期一致
```

**预期**: 文档引用的每个示例名存在于 `examples/`，运行命令正确。

### 场景 E：站内链接与配置项一致

```bash
# 6. 站内链接无悬空（指向存在的目标页面）——脚本校验
# 7. 文档中的环境变量名/参数默认值与代码一致（抽查 DASHSCOPE_API_KEY 等）
```

**预期**: 无悬空链接；抽查配置项与代码 100% 一致。

### 场景 F：真实体验（可选，需凭据）

```bash
# 8. 快速上手：按 quickstart.mdx 运行 examples/quickstart，得到模型回复
export DASHSCOPE_API_KEY=sk-...
cargo run -p quickstart -- --prompt "你好，请用一句话介绍你自己"
```

**预期**: 得到流式事件与最终回复；无凭据时给出明确错误提示。

## 完成判定

| 验证项 | 通过标准 |
|--------|----------|
| 场景 A | diff 无差异，mirror-map 50 页 + 例外登记 |
| 场景 B | 每页有状态块；计划中页无伪 Rust 用法 |
| 场景 C | 10 示例过 check + clippy，CI 全绿 |
| 场景 D | 文档引用示例名与实际一致 |
| 场景 E | 无悬空链接；配置项 100% 一致 |
| 场景 F | 有凭据时跑通，无凭据时明确报错 |
