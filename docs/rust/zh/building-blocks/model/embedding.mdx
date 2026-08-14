---
title: "嵌入模型"
description: "将文本转换为向量，服务检索、RAG 与记忆"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L2）。当前内置 Embedding 为 DashScope。
</Note>

嵌入模型（Embedding）将文本转换为稠密向量，支撑语义检索、RAG 与记忆召回。Rust 版通过 `EmbeddingModel` trait（`agent_scope_embedding`）抽象，内置实现：

| 模型 API | Rust 类型 |
|----------|-----------|
| DashScope | `DashScopeEmbeddingModel` |

## 创建模型

嵌入模型接收 API Key、模型卡与输入维度。`EmbeddingModelCard` 声明模型名、维度与是否归一化：

```rust
use std::sync::Arc;
use agent_scope_dashscope::DashScopeEmbeddingModel;
use agent_scope_embedding::EmbeddingModelCard;

let embedding = Arc::new(DashScopeEmbeddingModel::new(
    api_key,
    EmbeddingModelCard::new("text-embedding-v3", 1024, false),
));
```

## 调用

`EmbeddingModel` 将文本转为 `Embedding`（`Vec<f64>`）：

| 方法 | 说明 |
|------|------|
| `embed(inputs)` | 将输入文本转为向量，返回 `EmbeddingResponse` |
| `embedding_model()` | 返回模型卡 |

## 与 RAG / 记忆联动

嵌入模型是 RAG 与向量记忆的基础组件：

- **RAG**：`KnowledgeBase` 接收一个 `EmbeddingModel` 与一个 `VectorStore`，对文档分块后嵌入入库（见 [RAG](../rag)）。
- **长期记忆**：`TurbovecMemory` 使用嵌入模型建立向量索引做语义检索（见 [长期记忆](../long-term-memory)）。

```rust
let kb = Arc::new(KnowledgeBase::new(
    "kb".to_string(),
    "knowledge base".to_string(),
    embedding,
    vector_store,
    "collection".to_string(),
    None,
));
```
