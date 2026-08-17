---
title: "RAG 服务"
description: "服务化检索增强问答"
---

<Note>
**Rust 实现状态**: 部分支持。
- 已支持：库级 RAG（`KnowledgeBase` + `RAGMiddleware`，Static / Agentic 两种模式）。
- 尚未实现：服务化 RAG（HTTP 服务、文件托管、分布式索引）。
</Note>

# RAG 服务（部分支持）

服务化 RAG 通常带 HTTP 服务、文件托管与分布式索引。AgentScope Rust 未实现服务化形态，但在**库级**提供完整的 RAG 能力，可嵌入应用或自建 HTTP 服务暴露。

## 库级 RAG

| 能力 | Rust 状态 |
|------|-----------|
| 知识库构建 | ✅ `KnowledgeBase`（Parser/Chunker/Embedding/VectorStore 组合） |
| 静态注入检索 | ✅ `RAGMiddleware` + `RAGMode::Static` |
| 智能体主动检索 | ✅ `RAGMiddleware` + `RAGMode::Agentic` |
| RAG HTTP 服务 | ❌ 未实现 |
| 文件托管 / 分布式索引 | ❌ 未实现 |

库级 RAG 的完整用法见 [RAG](../building-blocks/rag) 与 `examples/rag`。

## 缺失范围

- 无 RAG HTTP 服务：需自建（如 axum/actix）并把 `KnowledgeBase` + `RAGMiddleware` 嵌入其中。
- 无文件托管与分布式索引：`TurbovecVectorStore` 为本地向量库。

## 替代能力

- 库级 RAG（`KnowledgeBase` + `RAGMiddleware`）见 [RAG](../building-blocks/rag)。
- 可在 Rust 侧自建 HTTP 服务包装 `KnowledgeBase` 检索端点。

## 相关

- 库级基础能力见对应的 building-blocks 页面。
- 各页面顶部状态块标注该能力的实现状态；仓库级逐页对照表为维护文件，不随站点发布。
