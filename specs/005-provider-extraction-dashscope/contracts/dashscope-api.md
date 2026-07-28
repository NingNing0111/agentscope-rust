# Contract: DashScope API (兼容模式)

**Feature**: 005-provider-extraction-dashscope | **Version**: 0.1.0

## Base Endpoint

```
https://dashscope.aliyuncs.com/compatible-mode/v1
```

## Authentication

```
Authorization: Bearer <api_key>
```

## Endpoints Used

### POST /chat/completions

Request body (compatible with OpenAI Chat Completions):

```json
{
  "model": "qwen-plus",
  "messages": [
    {"role": "user", "content": "Hello"}
  ],
  "stream": true,
  "stream_options": {"include_usage": true},
  "max_tokens": 1024,
  "temperature": 0.7,
  "top_p": 0.9,
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get current weather",
        "parameters": {"type": "object", "properties": {}}
      }
    }
  ],
  "tool_choice": "auto",
  "enable_search": false,
  "repetition_penalty": 1.1,
  "enable_thinking": false
}
```

**DashScope-specific fields** (not in OpenAI API):
- `enable_search`: bool — 联网搜索增强
- `repetition_penalty`: float (> 0) — 重复惩罚系数
- `enable_thinking`: bool — 思考模式（Qwen 专有）
- `thinking_budget`: u32 — 思考 token 预算（仅在 `enable_thinking: true` 时发送）

**DashScope constraints**:
- `tool_choice: "required"` 部分模型不支持 → 返回 `UnsupportedFeature`
- `enable_thinking: true` + `tool_choice: "required"` 互斥 → 请求前校验拒绝
- 无原生 `response_format: {"type": "json_schema"}` → 结构化输出通过 tool-calling

### Response (non-streaming)

```json
{
  "id": "chatcmpl-xxx",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "qwen-plus",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help?",
        "tool_calls": null
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 8,
    "total_tokens": 18
  }
}
```

### Response (streaming SSE)

```
data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":" World"},"finish_reason":null}]}

data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":8,"total_tokens":18}}

data: [DONE]
```

**SSE Edge Cases**:
- 最终 chunk 可能 `choices: []`（空数组）但包含 `usage` — 需正确处理而非 panic
- `data: [DONE]` 标记流结束
- `stream_options.include_usage` 仅在启用 `stream` 时有效

### Error Response

DashScope 兼容模式的错误响应支持两种格式：

**格式 1 — OpenAI 兼容嵌套格式**:
```json
{
  "error": {
    "code": "InvalidApiKey",
    "message": "Invalid API-key provided.",
    "type": "invalid_request_error"
  }
}
```

**格式 2 — 百炼扁平格式** (部分端点返回):
```json
{
  "code": "InvalidApiKey",
  "message": "Invalid API-key provided.",
  "request_id": "xxx"
}
```

**解析策略**: 优先尝试嵌套格式 `{"error": ...}`，失败后回退到扁平格式 `{"code": ..., "message": ...}`。

**Mapping to ModelError**:
| HTTP Status | DashScope error.code | ModelErrorKind |
|-------------|---------------------|----------------|
| 400 | `InvalidParameter` | `BadRequest` |
| 400 | `ModelNotFound` | `BadRequest` |
| 401 | `InvalidApiKey` | `Authentication` |
| 429 | `Throttling.RateQuota` | `RateLimit` |
| 500 | `InternalError` | `InternalServer` |
| 502/503 | `ServiceUnavailable` | `InternalServer` |
| 504 | `GatewayTimeout` | `ApiTimeout` |

### Tool Choice Constraints

- `"required"` 模式在 `enable_thinking=true` 时被拒（互斥）
- 部分模型仅支持 `"auto"` 和 `"none"` — 传入 `"required"` 时返回 `UnsupportedFeature`
