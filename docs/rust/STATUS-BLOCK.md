# 状态块规范（Status Block Guide）

**目的**: 定义 `docs/rust/zh/` 每个 `.mdx` 页面的「Rust 实现状态」标注规范。三个档位为唯一合法取值，禁止其他措辞（宪法 §5 不允许伪兼容、§18 兼容性分级）。

## 三档模板

每个页面在 frontmatter 之后、首节之前 MUST 放置一个 `<Note>` 状态块。

### 已实现

```mdx
<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用，兼容基线为 AgentScope Python v2.0.5。
</Note>
```

### 部分支持

```mdx
<Note>
**Rust 实现状态**: 部分支持。
- 已支持：<列出支持范围>
- 尚未实现：<列出缺失边界>

兼容基线为 AgentScope Python v2.0.5。
</Note>
```

### 计划中

```mdx
<Note>
**Rust 实现状态**: 计划中。该能力在 AgentScope Python <版本> 中提供，AgentScope Rust 尚未实现。当前 Rust 侧可用的替代能力见 <链接>（如有）。
</Note>
```

## 使用规则

- 状态块必须与 `docs/rust/mirror-map.md` 及兼容性矩阵的记录一致。
- **已实现**页：可包含真实 Rust 代码示例（引用 `examples/`）与兼容等级（L1-L4）标注。
- **部分支持**页：必须同时列出「已支持 / 尚未实现」两列，不得笼统标注。
- **计划中**页：MUST NOT 出现 Rust 代码块或伪造的 Rust API 用法；仅描述 Python 侧能力简介与 Rust 缺失范围，可链接替代能力页面。
- 禁止措辞：「兼容」「完全支持」「即将支持」「差不多支持」等模糊表述。

## 兼容等级（宪法 §18）

| 等级 | 定义 |
|------|------|
| L1 | 协议兼容：数据结构、序列化格式与 Python 兼容 |
| L2 | 核心行为兼容：Agent/Model/Tool/Event/Memory 核心流程可观察行为兼容 |
| L3 | 公开 API 语义兼容：主要公开接口有等价 Rust API 且行为等价 |
| L4 | 示例迁移兼容：官方示例可低成本迁移 |
