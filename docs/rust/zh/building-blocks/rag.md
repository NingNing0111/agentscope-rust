---
title: "RAG"
description: "为智能体构建检索增强生成（RAG）能力"
---

<Note>
**Rust 实现状态**: 已实现——**库级 / 中间件形态**。RAG 在 AgentScope Rust 中以 `KnowledgeBase` + `RAGMiddleware` 形式提供；服务化 RAG（HTTP 服务）为「计划中」，见 [deploy/rag](../deploy/rag)。
</Note>

AgentScope Rust 中的 RAG 由**可独立替换**的功能模块组成（`agent_scope_rag`）：

| 功能模块 | 描述 |
|----------|------|
| 解析器 Parser | 把原始字节拆成 `Section`（默认 `TextParser` 支持 `.txt` / `.md`；启用 `xberg` feature 后可用 `XbergParser` 解析 PDF / DOCX / PPTX / XLSX / HTML） |
| 切块器 Chunker | 把 `Section` 切成最终入库的 `Chunk`（`ApproxTokenChunker`） |
| 嵌入模型 Embedding | 把 `Chunk` 文本嵌入为向量（见 [Embedding](model/embedding)） |
| 向量库 Vector Store | 存储 `Chunk` 向量并支持检索（`TurbovecVectorStore`，基于 `turbovec`） |
| 知识库 KnowledgeBase | 绑定嵌入模型 + 向量库 + collection，封装 `insert_document` / `ingest_bytes` / `search` / `list_documents` / `delete_document` |

## 组装知识库

```rust
use std::sync::Arc;
use agent_scope_rig::RigEmbeddingModel;
use agent_scope_rag::{Chunk, KnowledgeBase, TurbovecVectorStore};

let embedding = Arc::new(RigEmbeddingModel::openai(api_key, "text-embedding-3-small")?);
let vector_store = Arc::new(TurbovecVectorStore::new(4)?);
let kb = Arc::new(KnowledgeBase::new(
    "project".into(),
    "Project documents.".into(),
    embedding,
    vector_store,
    "project".into(),
    None,
));

kb.insert_document(
    vec![Chunk { content: "…".into(), source: "doc".into(), chunk_index: 0, total_chunks: 1, metadata: Default::default() }],
    Some("doc-id".into()),
    None,
).await?;
```

从文件入库时，先解析再切块：

```rust
use agent_scope_rag::{ApproxTokenChunker, TextParser};

kb.ingest_bytes(
    &TextParser,
    &ApproxTokenChunker::new(200, 40),
    std::fs::read("notes.md")?,
    "notes.md",
    Some("notes".into()),
).await?;
```

多格式文档（PDF、Word、PPT、Excel、HTML）需要启用 `agent_scope_rag` 的 `xberg` feature，并使用 `XbergParser`。当前不包含 OCR，扫描件 PDF 无法抽出文字。

## 集成到智能体

通过 `RAGMiddleware` 将检索注入 agent：

| 模式 | 行为 |
|------|------|
| `RAGMode::Static` | 每轮自动注入检索到的相关上下文（`pre_reply`） |
| `RAGMode::Agentic` | 工具式检索：由 LLM 决定何时/是否搜索（`pre_reasoning` 注入工具 schema） |

```rust
use agent_scope_rag::{RAGMiddleware, RAGMode};

let rag = Arc::new(RAGMiddleware::new(vec![kb], RAGMode::Static, 3, None));
let agent = ReActAgent::new(config, ReActConfig::default(), ContextConfig::default(), vec![rag])?;
```

## 完整示例

见 [`examples/rag`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/rag/)（`cargo run -p rag`），构建知识库并运行一个基于知识库回答的 Agent（需要 `DEFAULT_API_KEY`）。
