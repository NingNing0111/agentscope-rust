# Quickstart: RAG System (Feature 011)

**Feature**: 011-rag-system
**Date**: 2026-07-31

## Prerequisites

- Rust toolchain (stable 1.75+)
- Project workspace with all crates built: `cargo build --workspace`
- Required crates: `agent_scope_embedding`, `agent_scope_rag`, `agent_scope_dashscope`, `agent_scope_types`
- (Optional) DashScope API Key for live embedding tests

## Scenario 1: Mock Embedding Model Test

验证 `EmbeddingModel` trait 可通过 mock 实现完成单元测试（SC-002）。

### Setup

```bash
cargo test -p agent_scope_embedding -- mock_embedding_model
```

### Expected behavior

1. 创建 mock `EmbeddingModel`（固定返回维度=4 的向量）
2. 调用 `embed(["hello", "world"])`
3. 验证返回 2 个向量，各长度=4
4. 验证 `usage.total_tokens > 0`
5. 验证 `model_card().dimensions` 返回 4
6. 传入 `DataBlock` 到 `supports_multimodal=false` 的模型 → 返回 `MultimodalNotSupported`

### Expected output

```
test embedding::tests::test_mock_embedding_model ... ok
```

## Scenario 2: FileEmbeddingCache Test

验证缓存命中时零额外 API 调用（SC-005）。

### Setup

```bash
cargo test -p agent_scope_embedding -- file_embedding_cache
```

### Expected behavior

1. 创建临时缓存目录
2. 存储键 "key1" → `vec![vec![1.0, 2.0]]`
3. `lookup("key1")` 返回缓存值
4. `lookup("key2")` 返回 `None`
5. 存储 100 个条目后全部命中

### Expected output

```
test cache::tests::test_file_embedding_cache_hit ... ok
test cache::tests::test_file_embedding_cache_miss ... ok
```

## Scenario 3: Parser → Chunker Pipeline

验证文本解析和切分管道的正确性。

### Setup

```bash
cargo test -p agent_scope_rag -- parser_chunker_pipeline
```

### Expected behavior

1. 构造 500 词纯文本
2. `TextParser::parse()` 返回 1 个 Section
3. `ApproxTokenChunker` (chunk_size=100, overlap=20) 切分
4. 生成 5+ 个 Chunk
5. 所有 Chunk 的 `source` 相同
6. `chunk_index` 从 0 递增，`total_chunks` 一致
7. 空文件 → 空 Section 列表 → 空 Chunk 列表
8. 两个不同 source 的 Section → 不跨 Section 合并

### Expected output

```
test parser::tests::test_text_parser_basic ... ok
test parser::tests::test_text_parser_empty_file ... ok
test chunker::tests::test_approx_token_chunker ... ok
test chunker::tests::test_cross_section_boundary ... ok
```

## Scenario 4: KnowledgeBase with Mock Backends

验证 KnowledgeBase 的 search/insert/delete/list 操作（SC-003）。

### Setup

```bash
cargo test -p agent_scope_rag -- knowledge_base
```

### Expected behavior

1. 创建 mock `EmbeddingModel`（维度=4）
2. 创建 mock `VectorStore`（in-memory HashMap）
3. 创建 `KnowledgeBase` 实例
4. `insert_document(chunks)` → 返回文档 ID
5. `search([query])` → 返回插入的匹配块
6. 验证结果去重（相同 document_id + chunk_index → 保留最高分）
7. 验证按 score 降序排列
8. `list_documents()` → 返回插入的文档摘要
9. `delete_document(id)` → 后续搜索无结果
10. `metadata_filter` 覆盖 chunk metadata（安全边界测试）
11. 首次操作时懒创建 collection

### Expected output

```
test knowledge_base::tests::test_insert_and_search ... ok
test knowledge_base::tests::test_search_deduplication ... ok
test knowledge_base::tests::test_delete_document ... ok
test knowledge_base::tests::test_list_documents ... ok
test knowledge_base::tests::test_metadata_filter_override ... ok
test knowledge_base::tests::test_lazy_collection_init ... ok
```

## Scenario 5: RAGMiddleware Static Mode

验证 static 模式下 RAGMiddleware 自动注入上下文（SC-007）。

### Setup

```bash
cargo test -p agent_scope_rag -- rag_middleware_static
```

### Expected behavior

1. 创建包含已知内容的 KnowledgeBase（mock embedding + mock vector store）
2. 创建 `RAGMiddleware`（Static 模式，绑定 1 个 KB）
3. 构造 `AgentState`，包含用户消息 "查询"
4. 调用 `middleware.pre_reply(&mut agent_state)`
5. 验证 `agent_state.context` 中插入了 `HintBlock`
6. HintBlock 包含匹配的 chunk 内容和 source 引用
7. 空知识库 → 不注入任何上下文
8. 多 KB 绑定 → 结果跨 KB 聚合

### Expected output

```
test rag_middleware::tests::test_static_mode_injects_context ... ok
test rag_middleware::tests::test_static_mode_empty_results ... ok
test rag_middleware::tests::test_static_mode_multiple_kbs ... ok
```

## Scenario 6: RAGMiddleware Agentic Mode

验证 agentic 模式下 RAGMiddleware 注册 Tool。

### Setup

```bash
cargo test -p agent_scope_rag -- rag_middleware_agentic
```

### Expected behavior

1. 创建 KnowledgeBase（mock backends）
2. 创建 `RAGMiddleware`（Agentic 模式）
3. 调用 `middleware.post_acting(&mut agent_state, &mut tools)`
4. 验证 `tools` 中包含一个 `search_{kb_name}` Tool
5. Tool 执行 → 调用 `kb.search()` → 返回格式化结果
6. 多个 KB → 每个注册独立 Tool
7. 重复调用 `post_acting` → 不重复注册 Tool

### Expected output

```
test rag_middleware::tests::test_agentic_mode_registers_tool ... ok
test rag_middleware::tests::test_agentic_tool_execution ... ok
test rag_middleware::tests::test_agentic_multi_kb_tools ... ok
test rag_middleware::tests::test_agentic_no_duplicate_registration ... ok
```

## Scenario 7: DashScope Embedding Integration (optional, requires API Key)

### Setup

```bash
# Set DASHSCOPE_API_KEY env var first
cargo test -p agent_scope_dashscope -- dashscope_embedding -- --ignored
```

### Expected behavior

1. 创建 `DashScopeEmbeddingModel` 并指定模型 "text-embedding-v3"
2. 调用 `embed(["你好，世界"])`
3. 验证返回的向量维度与 `model_card().dimensions` 一致
4. 验证 `usage.total_tokens > 0`

## Test Summary

| Scenario | Crate | Requires API Key | Key Verification |
|---|---|---|---|
| Mock Embedding | agent_scope_embedding | No | SC-002 |
| File Cache | agent_scope_embedding | No | SC-005 |
| Parser + Chunker | agent_scope_rag | No | SC-004 |
| KnowledgeBase | agent_scope_rag | No | SC-003, SC-006 |
| RAGMiddleware Static | agent_scope_rag | No | SC-007 |
| RAGMiddleware Agentic | agent_scope_rag | No | N/A |
| DashScope Embedding | agent_scope_dashscope | Yes | N/A |
