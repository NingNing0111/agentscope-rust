---
title: "大语言模型"
description: "创建并调用对话模型，驱动智能体对话与工具调用"
---

<Note>
**Rust 实现状态**: 已实现。本文档描述的能力在 AgentScope Rust 中可用。当前内置 Chat Model 为 rig（OpenAI / Anthropic / DeepSeek）；`ChatModel` trait 支持接入自定义 Provider。
</Note>

大语言模型（**Chat Model**）驱动智能体的对话与工具调用。它接收一段对话消息，产出回复文本，并在需要时发起工具调用——是所有智能体能力的核心引擎。

Rust 版通过 `ChatModel` trait（`agent_scope_model`）抽象统一接口，内置实现：

| 模型 API | Rust 类型 |
|----------|-----------|
| OpenAI / Anthropic / DeepSeek | `RigChatModel`（`openai` / `anthropic` / `deepseek`） |

OpenAI 为示例默认后端（`RigChatModel::openai`）。

## 创建模型

模型接收 API Key 与模型名，构造后可用链式方法微调行为：

```rust
use std::sync::Arc;
use agent_scope_rig::RigChatModel;

let model = Arc::new(RigChatModel::openai(&api_key, "qwen3.7-plus")?.with_stream(true));
```

三个 Provider 构造入口签名一致，均为 `(api_key, model) -> Result<RigChatModel, ModelError>`：

| 构造器 | 后端 | 默认上下文窗口 |
|--------|------|----------------|
| `RigChatModel::openai` | OpenAI Chat Completions | 131072 |
| `RigChatModel::anthropic` | Anthropic Messages API | 200000 |
| `RigChatModel::deepseek` | DeepSeek（OpenAI 兼容） | 131072 |

`api_key` 必填非空（空白将返回校验错误）；上下文窗口由构造器按 Provider 自动设定，也可用 `with_context_size` 覆盖。

## 链式配置

`RigChatModel` 提供一组返回 `Self` 的链式方法，用于在构造后调整运行配置：

| 方法 | 参数 | 说明 |
|------|------|------|
| `with_stream` | `bool` | 设置默认流式模式（默认开启） |
| `with_base_url` | `impl Into<String>` | 覆盖 API 端点 URL（如代理 / 网关地址） |
| `with_parameters` | `RigParameters` | 设置生成参数（温度、token 上限等） |
| `with_max_retries` | `u32` | 最大重试次数（默认 3） |
| `with_retry_delay` | `f64` | 重试间隔秒数（默认 1.0） |
| `with_context_size` | `i64` | 覆盖上下文窗口大小 |

重试默认仅对可重试错误分类触发（网络连接、超时、限流、服务端内部错误）；参数校验类错误不会重试，而是立即返回。

## 生成参数（RigParameters）

`RigParameters` 描述一次生成请求的采样参数，全部字段可选（`None` 表示不设置）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `max_tokens` | `Option<u64>` | 生成 token 上限 |
| `temperature` | `Option<f64>` | 采样温度 |
| `top_p` | `Option<f64>` | nucleus 采样 |
| `top_k` | `Option<u64>` | top-k 采样 |
| `seed` | `Option<u64>` | 随机种子 |
| `stop` | `Option<Vec<String>>` | 停止序列 |
| `thinking_budget` | `Option<u64>` | 思考 token 预算 |
| `additional_params` | `Option<serde_json::Value>` | 透传给 Provider 的兜底参数 |

`RigParameters` 同样提供链式构造（`with_max_tokens` / `with_temperature` / `with_top_p` / `with_top_k` / `with_seed` / `with_stop` / `with_thinking_budget` / `with_additional_params`）：

```rust
use agent_scope_rig::{RigChatModel};
use agent_scope_rig::params::RigParameters;

let params = RigParameters::new()
    .with_temperature(0.7)
    .with_max_tokens(1024);

let model = RigChatModel::openai(&api_key, "qwen3.7-plus")?
    .with_parameters(params);
```

## 模型调用

`ChatModel` 提供 `call` 与 `call_api` 两个入口，返回 `ModelCallResult`（`Complete` 或 `Stream`）。智能体内部通过 trait 调用，无需关心具体 Provider：

| 方法 | 说明 |
|------|------|
| `call(msgs, tools, tool_choice)` | 发起一次对话，内置自动重试与取消逻辑，返回 `ChatResponse` |
| `generate_structured_output(msgs, schema)` | 生成符合 JSON Schema 的结构化输出 |
| `count_tokens(msgs, tools)` | 估算 token 数（默认按字节数 ÷ 4 估算） |
| `list_models(parsed_cards, base_schema)` | 从模型卡构建可用模型列表 |
| `validate_tool_choice(choice, tools)` | 校验工具选择是否被支持 |
| `model_name()` | 返回模型标识字符串 |
| `stream_enabled()` | 是否默认开启流式 |
| `max_retries()` / `retry_delay()` / `context_size()` / `retryable_errors()` | 读取重试与上下文配置 |

thinking 模式下对 `tool_choice` 做了 `required → auto` 的安全回退（Provider 无法在扩展思考与强制工具调用并发时降级）。

## 结构化输出

`ChatModel::generate_structured_output` 在**模型层**已实现，可生成符合给定 JSON Schema 的结构化结果。默认优先走 Provider 原生结构化输出路径，被拒绝或失败时回退为「注入工具调用 + 强制 tool_choice」的旁路方案：

```rust
let schema = serde_json::json!({
    "type": "object",
    "properties": { "city": { "type": "string" } },
    "required": ["city"]
});
let result = model.generate_structured_output(&msgs, &schema).await?;
```

<Note>
当前 Agent 循环（`ReActAgent`）尚未直接接线结构化输出——`reply_context.structured_schema` 字段已定义，但模型层的结构化输出需由调用方直接使用。这是「部分支持」边界。
</Note>
