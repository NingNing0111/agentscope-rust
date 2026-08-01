# Quickstart: Turbovec RAG 向量存储

**Feature**: 016-turbovec-rag
**Date**: 2026-07-31

## Prerequisites

- Rust 工具链 1.89+
- 64-bit Linux/macOS（turbovec 要求 `target_pointer_width = "64"`）

## Setup

确保 workspace 编译通过：

```bash
# 添加 turbovec 依赖后
cargo build -p agent_scope_rag
```

## Verification Scenarios

### Scenario 1: 基本 CRUD（US1）

```bash
# 运行 turbovec store 单元测试
cargo test -p agent_scope_rag turbovec

# 预期：所有测试通过，包括：
#   - test_create_and_has_collection
#   - test_insert_and_search
#   - test_delete_then_search_empty
#   - test_list_documents
#   - test_metadata_filter_search
#   - test_dimension_mismatch_error
#   - test_empty_search_on_empty_collection
```

**预期结果**:
- `create_collection("test", 16)` → 成功
- `has_collection("test")` → `true`
- `insert("test", 100 条 16 维向量)` → 成功，<100ms
- `search("test", query, top_k=10)` → 返回 10 条结果，分数降序
- `delete("test", "doc-1")` → 成功，后续搜索不返回 doc-1
- `list_documents("test", None)` → 返回剩余文档摘要
- `has_collection("nonexistent")` → `false`

### Scenario 2: 持久化 Round-Trip（US2）

```bash
# 运行持久化测试
cargo test -p agent_scope_rag turbovec_persist

# 预期：以下测试通过
#   - test_save_load_roundtrip
#   - test_save_empty_store
#   - test_save_load_append_more
#   - test_load_corrupted_manifest_error
```

**预期结果**:
- 插入 200 条向量 → save → load → search 相同 query → 结果一致
- 空 store save → load → has_collection 全 false
- Save 后继续 insert → save → load → 向量总数正确
- 损坏的 manifest → `load()` 返回错误，不 panic

### Scenario 3: KnowledgeBase 集成（US3）

```bash
# 运行 KnowledgeBase 集成测试（需 DashScope API key 已在环境变量）
# 或使用 mock embedding model
cargo test -p agent_scope_rag knowledge_base_with_turbovec -- --ignored

# 如果环境变量不可用，查看示例：
cargo run --example turbovec_rag 2>/dev/null || echo "示例尚未创建"
```

**预期结果**:
- KnowledgeBase 使用 TurbovecVectorStore 时，行为与使用 MockVectorStore 一致
- `kb.search(["query"])` 自动创建 backing collection
- `metadata_filter` 强制覆盖生效

### Scenario 4: 性能基准（可选）

```bash
# 性能基准测试（需 nightly Rust）
cargo bench -p agent_scope_rag --bench turbovec_bench 2>/dev/null || echo "benchmark 尚未配置"
```

**预期结果**:
- 1000 条 1536 维向量插入 < 1s
- top-10 search < 5ms
- save + load round-trip < 0.5s

### Scenario 5: turbovec 原生行为验证

```bash
# 验证 turbovec crate 本身编译和基础行为
cargo test -p turbovec 2>/dev/null && echo "turbovec OK" || echo "turbovec tests at turbovec/ path"

# 或者验证 turbovec 的正确引入（在 workspace 中）
# turbovec 的路径在 ../turbovec/turbovec
```

## 测试覆盖率

运行完整的测试套件确认覆盖率：

```bash
# 运行 agent_scope_rag 全部测试
cargo test -p agent_scope_rag

# clippy 检查
cargo clippy -p agent_scope_rag -- -D warnings

# fmt 检查
cargo fmt -p agent_scope_rag -- --check
```

## 常见问题

**Q: turbovec 编译失败，提示 "requires a 64-bit target"**

A: turbovec 不支持 32-bit 或 WASM 目标。确保使用 `--target x86_64-unknown-linux-gnu` 或 `aarch64-apple-darwin`。

**Q: 插入大量向量后内存占用高于预期**

A: turbovec 的压缩优势在 2-bit 模式下最显著（1536 维 → 384 bytes/vector）。如果需要更低内存占用，尝试 `bit_width=2`。

**Q: search 返回的分数与余弦相似度不一致**

A: 确保插入和搜索的向量都已 L2 归一化。`TurbovecVectorStore` 在 `insert` 和 `search` 路径上自动执行归一化。
