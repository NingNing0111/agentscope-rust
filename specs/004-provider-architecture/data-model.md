# Data Model: Provider Architecture & DashScope (Feature 004)

**Date**: 2026-07-28 | **Spec**: [spec.md](./spec.md)

## Entity Overview

```
agent_scope_model (core, existing)
├── ChatModel (trait)          ← Provider 实现的目标
├── ChatResponse / Usage        ← Provider 输出
├── Formatter (trait)           ← 消息格式转换
├── ModelError / ModelErrorKind ← 错误传播
└── ToolChoice                  ← 工具选择配置

agent_scope_openai (extracted)
├── OpenAIChatModel             ← ChatModel impl
├── OpenAIChatFormatter         ← Formatter impl
├── OpenAIChatParameters        ← 参数 schema
└── ReasoningEffort             ← 推理强度枚举

agent_scope_dashscope (new)
├── DashScopeChatModel          ← ChatModel impl
├── DashScopeFormatter          ← Formatter impl
├── DashScopeParameters         ← 参数（含 enable_search）
└── (no ReasoningEffort)        ← DashScope 不支持
```

## 1. DashScopeChatModel

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `api_key` | `String` | yes | — | 阿里云百炼 API Key |
| `base_url` | `String` | yes | `"https://dashscope.aliyuncs.com/compatible-mode/v1"` | API 端点 |
| `model_name` | `String` | yes | — | 模型名称（如 `qwen-plus`） |
| `parameters` | `DashScopeParameters` | yes | `Default::default()` | 模型参数 |
| `stream` | `bool` | yes | `true` | 是否默认流式 |
| `max_retries` | `u32` | yes | `3` | 最大重试次数 |
| `retry_delay` | `f64` | yes | `1.0` | 重试间隔（秒） |
| `context_size` | `i64` | yes | `131072`（qwen-plus 默认） | token 上下文窗口 |
| `formatter` | `Box<dyn Formatter>` | yes | `DashScopeFormatter::default()` | 消息格式化器 |
| `client` | `reqwest::Client` | yes | internal | HTTP 客户端 |
| `extra_body` | `HashMap<String, JsonValue>` | yes | `{}` | 透传额外请求体字段 |

## 2. DashScopeParameters

百炼特有参数。通过 `schemars::JsonSchema` derive 生成 JSON Schema 供 ModelCard 使用。

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `max_tokens` | `Option<u32>` | no | `None` | 最大输出 token 数 |
| `temperature` | `Option<f64>` | no | `None` | 采样温度 (0~2) |
| `top_p` | `Option<f64>` | no | `None` | 核采样参数 |
| `top_k` | `Option<u32>` | no | `None` | Top-K 采样 |
| `enable_search` | `bool` | yes | `false` | 是否启用联网搜索（百炼特有） |
| `enable_thinking` | `bool` | yes | `false` | 启用思考模式/reasoning（返回 reasoning_content） |
| `thinking_budget` | `Option<u32>` | no | `None` | 思考 token 预算（仅在 enable_thinking=true 时生效） |
| `repetition_penalty` | `Option<f64>` | no | `None` | 重复惩罚系数，有效范围 (0, +∞) |
| `seed` | `Option<u64>` | no | `None` | 随机种子（可复现输出） |
| `stop` | `Option<Vec<String>>` | no | `None` | 停止词列表 |

**Constraints**:
- `repetition_penalty` MUST be `> 0` — 构建请求时需校验
- `tool_choice="required"` + `enable_thinking=true` 互斥 — DashScope 思考模式下不支持强制 tool call
- `enable_search` 仅部分模型支持，对不支持的模型需返回 `UnsupportedFeature`
- `stream_options.include_usage` 仅在 `stream=true` 时添加到请求体

## 3. DashScopeFormatter

实现 `Formatter` trait。

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `input_types` | `Vec<String>` | yes | `["text/plain"]` | 支持的输入媒体类型 |

**format() 转换逻辑**:
- 单 TextBlock 消息 → `{"role": "...", "content": "<text>"}`
- 多模态（Text + DataBlock）→ `{"role": "...", "content": [{"type": "text", "text": "..."}, {"type": "image_url", "image_url": {"url": "data:..."}}]}`
- ToolCall → `{"role": "assistant", "tool_calls": [{"id": "...", "type": "function", "function": {"name": "...", "arguments": "..."}}]}`
- ToolResult → `{"role": "tool", "tool_call_id": "...", "content": "..."}`

## 4. Provider Crate 依赖拓扑（FR-001 验证）

```
agent_scope_openai
├── agent_scope_model          (ChatModel trait)
├── agent_scope_message        (Msg, ContentBlock)
├── agent_scope_types          (基础类型)
├── reqwest 0.12               (HTTP 调用)
├── tokio-stream               (流式)
└── serde / serde_json / base64 / ...

agent_scope_dashscope
├── agent_scope_model          (ChatModel trait)
├── agent_scope_message        (Msg, ContentBlock)
├── agent_scope_types          (基础类型)
├── reqwest 0.12               (HTTP 调用)
├── tokio-stream               (流式)
└── serde / serde_json / base64 / ...

agent_scope_model (after split)
├── agent_scope_message
├── agent_scope_types
├── agent_scope_utils
├── futures                     (Pin<Box<dyn Stream>> — model_trait.rs)
├── base64                      (audio codec — response.rs + accumulator.rs)
├── uuid / chrono / schemars    (serde metadata)
└── ❌ NO reqwest / tokio-stream / tokio-util / serde_yaml (verified via cargo tree)
```

**Note**: `serde_yaml` is removed from `agent_scope_model`. `ModelCard::from_yaml()` is refactored to `ModelCard::from_raw(raw: &HashMap, base_schema: &JsonValue)` — Provider crates handle YAML file reading and parsing, then pass the parsed data to the core model.

## State Transitions

### ChatModel::call() — retry machine (与 Feature 003 相同)

```
call() called
    │
    ▼
[attempt = 0]
    │
    ▼
_call_api() ──success──▶ ModelCallResult::Complete or Stream
    │
    ▼ (error)
retryable_errors() match? ──no──▶ propagate ModelError
    │ yes
    ▼
attempt <= max_retries? ──no──▶ ModelError::RetryExhausted
    │ yes
    ▼
sleep(retry_delay) → attempt += 1 → loop
```

### SSE Stream 解析状态机（DashScope/OpenAI 通用）

```
[HTTP response byte stream]
    │
    ▼ (chunk by line)
"data: {json}" ──▶ parse JSON ▷ extract delta fields ▷ build ChatResponse chunk
    │
    ▼
"data: [DONE]" ──▶ close stream, yield final is_last=true chunk
    │
    ▼ (empty line)
skip (SSE event separator)
```
