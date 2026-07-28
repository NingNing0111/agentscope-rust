# Contract: DashScope API

**Feature**: 004-provider-architecture | **Version**: 0.1.0

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
  "repetition_penalty": 1.1
}
```

**DashScope-specific fields** (not in OpenAI API):
- `enable_search`: bool — enable web search augmentation
- `repetition_penalty`: float — penalty for token repetition

**DashScope constraints** (vs OpenAI):
- `tool_choice: "required"` not supported by all models (fallback to `"auto"`)
- No native `response_format: {"type": "json_schema"}` — structured output via tool-calling

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

data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":8,"total_tokens":18}}

data: [DONE]
```

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

解析器 MUST 兼容两种格式，优先尝试嵌套格式，回退到扁平格式。

Mapping to `ModelError`:
| HTTP Status | DashScope error.code | ModelError |
|-------------|---------------------|------------|
| 400 | `InvalidParameter` | `ApiError { status: 400, ... }` → `ModelErrorKind::BadRequest` |
| 400 | `ModelNotFound` | `ApiError { status: 400, ... }` → `ModelErrorKind::BadRequest` |
| 401 | `InvalidApiKey` | `ApiError { status: 401, ... }` → `ModelErrorKind::Authentication` |
| 429 | `Throttling.RateQuota` | `ApiError { status: 429, ... }` → `ModelErrorKind::RateLimit` |
| 500 | `InternalError` | `ApiError { status: 500, ... }` → `ModelErrorKind::InternalServer` |
| 502/503 | `ServiceUnavailable` | `ApiError { status: 502/503, ... }` → `ModelErrorKind::InternalServer` |
| 504 | `GatewayTimeout` | `ApiError { status: 504, ... }` → `ModelErrorKind::ApiTimeout` |

**SSE Stream Edge Cases**:
- 仅含 `usage` 的最终 chunk 的 `choices` 可能为空数组 `[]` — 需正确处理而非 panic
- `data: [DONE]` 标记流结束 — 与 OpenAI 行为一致
- `stream_options.include_usage` 仅在启用 `stream` 时有效

**Tool Choice Constraints**:
- `"required"` 模式在 `enable_thinking=true` 时被拒绝（互斥）
- 部分模型仅支持 `"auto"` 和 `"none"` — 对不支持的模型传入 `"required"` 需返回 `UnsupportedFeature`
