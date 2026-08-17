---
title: "模型概览"
description: "连接模型提供商并发现模型"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的 Chat Model 与 Embedding 能力在 AgentScope Rust 中可用；TTS 与 Realtime 模型为「计划中」，暂未实现。
</Note>

模型层通过 trait 抽象将 AgentScope Rust 连接到 LLM 提供商。核心是两个 trait：

| trait | 职责 | 内置实现 |
|-------|------|----------|
| `ChatModel`（`agent_scope_model`） | 对话与工具调用 | `RigChatModel` |
| `EmbeddingModel`（`agent_scope_embedding`） | 文本转稠密向量 | `RigEmbeddingModel` |

模型直接接收 API Key 与模型名构造，运行配置通过链式方法设置：

```rust
let model = RigChatModel::openai(&api_key, "qwen3.7-plus")?.with_stream(true);
```

## 模型族支持

| 模型族 | Rust 状态 | 类型 |
|--------|-----------|------|
| LLM（Chat Model） | ✅ 已实现 | `ChatModel` trait、`RigChatModel` |
| Embedding | ✅ 已实现 | `EmbeddingModel` trait、`RigEmbeddingModel` |
| TTS | ⏳ 计划中 | 无 TTS 类型（仅音频数据块能力） |
| Realtime | ⏳ 计划中 | 未实现 |

## 创建模型

所有模型共享同一构造模式：接收 API Key、模型名，以及可选的流式开关：

```rust
use agent_scope_rig::{RigChatModel, RigEmbeddingModel};

let model = RigChatModel::openai("sk-your-key", "qwen3.7-plus")?;
let stream_model = RigChatModel::openai("sk-your-key", "qwen3.7-plus")?.with_stream(true);
```

各模型族的使用细节见对应页面：[LLM](llm)、[Embedding](embedding)、[TTS](tts)。

## 模型卡（ModelCard）

`ModelCard`（`agent_scope_model`）以声明式描述模型的能力与参数 schema，供 UI 或工具动态渲染。它由 Provider 侧从 YAML/JSON 构建（`ModelCard::from_value`）。嵌入模型卡有更简洁的构造（`agent_scope_embedding`）：

```rust
use agent_scope_embedding::EmbeddingModelCard;

let card = EmbeddingModelCard::new("text-embedding-v3", 1024, false);
```

模型卡被 agent 用于展示与校验，也用于嵌入模型的维度声明（见 [Embedding](embedding)）。
