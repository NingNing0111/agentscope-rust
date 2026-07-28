# Contract: OpenAI Chat Model API

**Feature**: 003-model-api | **Version**: 0.1.0

## Overview

`OpenAIChatModel` is the reference ChatModel implementation for OpenAI's Chat Completions API (and OpenAI-compatible APIs). It communicates via HTTP using `reqwest`.

## Struct: OpenAIChatModel

```rust
pub struct OpenAIChatModel {
    // ── Authentication ──
    pub api_key: String,
    pub organization: Option<String>,
    pub base_url: String,

    // ── Model identity ──
    pub model_name: String,

    // ── Parameters ──
    pub parameters: OpenAIChatParameters,

    // ── Behaviour flags ──
    pub stream: bool,
    pub max_retries: u32,
    pub retry_delay: f64,
    pub context_size: usize,

    // ── Formatter ──
    pub formatter: Box<dyn Formatter>,

    // ── HTTP client (lazy init) ──
    client: reqwest::Client,
    client_kwargs: HashMap<String, serde_json::Value>,
    extra_body: Option<serde_json::Value>,
}
```

## Constructor

```rust
impl OpenAIChatModel {
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        parameters: Option<OpenAIChatParameters>,
        stream: bool,
        max_retries: u32,
        retry_delay: f64,
        context_size: usize,
        formatter: Option<Box<dyn Formatter>>,
        base_url: Option<String>,
        organization: Option<String>,
        client_kwargs: Option<HashMap<String, serde_json::Value>>,
        extra_body: Option<serde_json::Value>,
    ) -> Self;

    /// Build the API request body for a chat completion call.
    fn build_request_body(
        &self,
        model_name: &str,
        formatted_messages: Vec<serde_json::Value>,
        tools: Option<&[serde_json::Value]>,
        tool_choice: Option<&ToolChoice>,
    ) -> serde_json::Value;
}
```

### Default Values

| Parameter | Default |
|-----------|---------|
| `stream` | `true` |
| `max_retries` | `3` |
| `retry_delay` | `1.0` |
| `context_size` | `128000` |
| `base_url` | `"https://api.openai.com/v1"` |
| `formatter` | `OpenAIChatFormatter::default()` |

## Parameters: OpenAIChatParameters

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OpenAIChatParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    #[serde(default)]
    pub thinking_enable: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    #[serde(default = "default_true")]
    pub parallel_tool_calls: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}
```

## HTTP Request

### Endpoint

```
POST {base_url}/chat/completions
```

### Headers

```
Authorization: Bearer {api_key}
OpenAI-Organization: {organization}   # if present
Content-Type: application/json
```

### Request Body (non-streaming)

```json
{
  "model": "gpt-4.1",
  "messages": [...],
  "stream": false,
  "max_completion_tokens": 4096,
  "temperature": 0.7,
  "top_p": 0.9,
  "tools": [...],
  "tool_choice": "auto",
  "parallel_tool_calls": true
}
```

### Request Body (streaming)

Same as non-streaming, plus:
```json
{
  "stream": true,
  "stream_options": { "include_usage": true }
}
```

### Audio (omni models)

When `parameters.voice` is set:
```json
{
  "audio": { "voice": "alloy", "format": "pcm16" },
  "modalities": ["text", "audio"]
}
```

### extra_body

Any keys in `extra_body` are merged into the top-level request body.

## Retryable Errors

```rust
impl ChatModel for OpenAIChatModel {
    fn retryable_errors(&self) -> &[ModelErrorKind] {
        &[
            ModelErrorKind::ApiConnection,   // connection errors
            ModelErrorKind::ApiTimeout,       // request timeouts
            ModelErrorKind::RateLimit,        // 429
            ModelErrorKind::InternalServer,   // 5xx
        ]
    }
}
```

Concretely, retryable HTTP status codes: `429`, `500`, `502`, `503`, `504`. Plus any connection/timeout errors from `reqwest`.

## Streaming Response Parsing (SSE)

### SSE Format

```
data: {"id":"chatcmpl-...","choices":[{"delta":{"content":"Hello"},"index":0}],...}

data: {"id":"chatcmpl-...","choices":[],"usage":{...}}

data: [DONE]
```

### Chunk Parsing Logic

```rust
async fn parse_stream_response(
    response: reqwest::Response,
    start_time: Instant,
) -> impl Stream<Item = Result<ChatResponse, ModelError>> {
    // 1. Get byte stream from response
    // 2. Buffer lines until "\n\n"
    // 3. For each event:
    //    - Strip "data: " prefix
    //    - If "[DONE]" → finalize
    //    - Parse JSON → extract delta fields → build ChatResponse chunk
    // 4. Emit ChatResponse chunks
}
```

### Delta Field Extraction

| OpenAI Delta Field | ChatResponse Method |
|-------------------|---------------------|
| `choices[0].delta.content` | `append_text` |
| `choices[0].delta.reasoning_content` or `reasoning` | `append_thinking` |
| `choices[0].delta.tool_calls[n].function.{name,arguments}` | `append_tool_call` |
| `choices[0].delta.audio.data` (+ WAV header on first) | `append_data_block` |
| `choices[0].delta.audio.transcript` | `append_text` (merged with delta.content) |
| `usage` (on final chunk) | set on ChatResponse |

### Audio: PCM16 → WAV

First audio chunk gets a streaming WAV header prepended:

```rust
fn build_streaming_wav_header() -> Vec<u8> {
    // 44-byte WAV header:
    // RIFF chunk: "RIFF" + 4-byte size (0xFFFFFFFF) + "WAVE"
    // fmt chunk: "fmt " + 16 + PCM(1) + 1ch + 24000 + 48000 + 2 + 16
    // data chunk: "data" + 4-byte size (0xFFFFFFFF)
}
```

Subsequent audio chunks are raw PCM16 bytes appended directly.

## Non-Streaming Response Parsing

```rust
fn parse_completion_response(
    response: serde_json::Value,  // ChatCompletion object
    start_time: Instant,
    audio_format: &str,
) -> Result<ChatResponse, ModelError> {
    // 1. Extract choices[0].message
    // 2. reasoning_content → ThinkingBlock
    // 3. message.content → TextBlock
    // 4. message.tool_calls → ToolCallBlock(s)
    // 5. message.audio.data/transcript → DataBlock/TextBlock
    // 6. response.usage → ChatUsage
    // 7. Return ChatResponse { is_last: true, ... }
}
```

## Structured Output

For OpenAI models with native structured output support (`response_format`), the override:

```rust
impl OpenAIChatModel {
    async fn _call_api_with_structured_output(...) -> StructuredResponse {
        // Try base impl (tool-calling trick) first.
        // On BadRequestError mentioning "tool_choice":
        //   → retry with tool_choice=ToolChoice { mode: "auto" }
    }
}
```

## Tool Choice Format

| `tool_choice.mode` | API `tool_choice` value |
|-------------------|------------------------|
| `"auto"` | `"auto"` |
| `"none"` | `"none"` |
| `"required"` | `"required"` |
| Specific tool name | `{"type": "function", "function": {"name": "search"}}` |

When `tool_choice.tools` is set, tool schemas are filtered to only the named tools BEFORE sending to the API.

## JSON Schema Flattening

Before sending tool schemas, `$ref` / `$defs` references are inlined:

```rust
fn flatten_json_schema(schema: &mut serde_json::Value) {
    let defs = schema.get("$defs")
        .and_then(|d| d.as_object())
        .cloned()
        .unwrap_or_default();

    // Recursively walk schema properties
    // Replace "$ref": "#/$defs/TypeName" with the actual definition
    // Track visited types to prevent infinite recursion
}
```

## Invariants

1. `api_key` is NEVER logged or serialized (keep as `String`, not exposed via Debug).
2. Streaming chunks with empty content (usage-only carrier chunks) are absorbed by the stream wrapper but NOT yielded to the consumer.
3. The `id` field of ChatResponse comes from `response.id` (API response), NOT auto-generated for OpenAIChatModel.
4. `tool_choice.mode` validation happens in `_validate_tool_choice` before request construction.
5. `extra_body` is merged at the TOP level of the request body — keys there can override any field including `model`, `messages`, etc.
