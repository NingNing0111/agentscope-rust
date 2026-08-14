---
title: "大语言模型"
description: "创建并调用对话模型，驱动智能体对话与工具调用"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L2）。当前内置 Chat Model 为 DashScope（OpenAI 兼容端点）；`ChatModel` trait 支持接入自定义 Provider。
</Note>

大语言模型（**Chat Model**）驱动智能体的对话与工具调用。Rust 版通过 `ChatModel` trait 抽象，内置实现：

| 模型 API | Rust 类型 |
|----------|-----------|
| DashScope（Qwen / Model Studio） | `DashScopeChatModel` |

## 创建模型

模型接收 API Key、模型名，并可用 `with_stream(true)` 开启流式：

```rust
use std::sync::Arc;
use agent_scope_dashscope::DashScopeChatModel;

let model = Arc::new(DashScopeChatModel::new(&api_key, "qwen-plus").with_stream(true));
```

## 模型调用

`ChatModel` 提供 `call` 与 `call_api` 两个入口，返回 `ModelCallResult`（`Complete` 或 `Stream`）。智能体内部通过 trait 调用，无需关心具体 Provider：

| 方法 | 说明 |
|------|------|
| `call(msgs, ...)` | 发起一次对话，返回 `ChatResponse` |
| `generate_structured_output(msgs, schema)` | 生成符合 JSON Schema 的结构化输出 |
| `count_tokens(msgs)` | 估算 token 数 |
| `list_models()` | 列出 Provider 可用模型 |
| `validate_tool_choice(choice)` | 校验工具选择是否被支持 |

模型构造携带重试与取消支持。thinking 模式下对 `tool_choice` 做了 `required → auto` 的安全回退（见 DashScope 已知行为）。

## 结构化输出

`ChatModel::generate_structured_output` 在**模型层**已实现（默认走 tool-calling bypass），可生成符合给定 JSON Schema 的结构化结果：

```rust
let schema = serde_json::json!({
    "type": "object",
    "properties": { "city": { "type": "string" } },
    "required": ["city"]
});
let result = model.generate_structured_output(&msgs, &schema).await?;
```

> **注意**：当前 Agent 循环（`ReActAgent`）尚未直接接线结构化输出——`reply_context.structured_schema` 字段已定义，但模型层的结构化输出需由调用方直接使用。这是「部分支持」边界。
