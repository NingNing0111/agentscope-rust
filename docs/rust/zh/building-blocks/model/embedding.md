---
title: "嵌入模型"
description: "将文本转换为向量，服务检索、RAG 与记忆"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用。当前内置 Embedding 为 OpenAI（rig 后端）。
</Note>

嵌入模型（Embedding）把一段文本转换为一组稠密的数值向量（`Vec<f32>`），让「语义相近」的文本在向量空间中彼此靠近。它不生成新文本，而是为语义检索、RAG 召回与长期记忆提供「可比较」的数字表示——这是向量数据库与相似度搜索的基础。

Rust 版通过 `EmbeddingModel` trait（`agent_scope_embedding`）抽象，内置实现：

| 模型 API | Rust 类型 |
|----------|-----------|
| OpenAI（`text-embedding-3-*`） | `RigEmbeddingModel` |

当前仅 OpenAI 提供 Embedding 后端（`RigEmbeddingModel::openai`）。

## 创建模型

嵌入模型接收 API Key 与模型名，向量维度由 rig 按模型标识自动查表：

```rust
use std::sync::Arc;
use agent_scope_rig::RigEmbeddingModel;

let embedding = Arc::new(RigEmbeddingModel::openai(api_key, "text-embedding-3-small")?);
```

## 调用

`EmbeddingModel` trait 将输入转为向量：

| 方法 | 说明 |
|------|------|
| `embed(inputs)` | 将输入转为向量，返回 `EmbeddingResponse`（每条输入对应一个向量） |
| `model_card()` | 返回模型卡（静态元数据） |
| `supports_multimodal()` | 是否支持多模态（`DataBlock`）输入 |

输入类型 `EmbeddingInput` 是一个枚举：

| 变体 | 说明 |
|------|------|
| `Text(String)` | 纯文本输入（始终支持） |
| `DataBlock(String)` | 多模态输入（需模型卡声明 `supports_multimodal = true`） |

返回的 `EmbeddingResponse` 含 `embeddings: Vec<Vec<f32>>`（与输入一一对应，每个向量长度等于模型维度）与 `usage`（token 用量）。当前 OpenAI 后端不支持多模态：传入 `DataBlock` 会返回 `MultimodalNotSupported` 错误。

## 模型卡

`EmbeddingModelCard` 描述嵌入模型的静态元数据：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `String` | 模型标识（如 `text-embedding-v3`） |
| `dimensions` | `u32` | 输出向量维度 |
| `supports_multimodal` | `bool` | 是否支持多模态输入 |

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
