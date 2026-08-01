# Model Abstraction

> One-liner: the unified interface for all LLM providers — the `ChatModel` trait defines streaming/non-streaming calls, automatic retry, token counting, and structured output; `Arc<dyn ChatModel>` is the decoupling layer between agents and concrete vendors.

## 1. Overview

This module covers the `agent_scope_model` crate at the abstraction layer (depending on `agent_scope_types`/`agent_scope_message`, and on no concrete provider). It defines "what a model call is"; concrete vendors (e.g., DashScope) implement the trait in separate crates.

**When to use**: calling a model to generate replies; implementing a custom provider for agent integration; consuming streaming responses and accumulating them into a complete result; using structured output (JSON mode).

**Prerequisites**: [Message & Basic Types](./message-types.md) (`Msg`/`ContentBlock`); for provider-specific usage see [DashScope](./dashscope.md).

## 2. Core Concepts & Main Public Types

### 2.1 The `ChatModel` Trait

The unified interface for all chat model providers (`async_trait`):

| Method | Category | Description |
|--------|----------|-------------|
| `model_name() -> &str` | Required | Model identifier (e.g., `"qwen-plus"`) |
| `stream_enabled() -> bool` | Required | Whether streaming is enabled by default |
| `call_api(...) -> Result<ModelCallResult, ModelError>` | Required | Provider-specific API implementation (the only method requiring network code) |
| `max_retries() -> u32` | Optional override | Maximum retry attempts, default `3` |
| `retry_delay() -> f64` | Optional override | Delay between retries in seconds, default `1.0` |
| `retryable_errors() -> &[ModelErrorKind]` | Optional override | Error categories that trigger retries, default empty (no retries) |
| `context_size() -> i64` | Optional override | Context window size in tokens, default `32768` |
| `call(...) -> Result<ModelCallResult, ModelError>` | Default method | **Call entry point**: wraps `call_api` in a retry loop |
| `count_tokens(...) -> usize` | Default method | bytes/4 heuristic (each `DataBlock` ≈ 2000 tokens); providers may override with precise tokenizers |
| `generate_structured_output(...) -> Result<StructuredResponse, ModelError>` | Default method | Structured output (see 4.4) |

**Retry semantics**: `call()` performs at most `max_retries` additional attempts, only when the error's `kind()` matches `retryable_errors()`, with `retry_delay` seconds between attempts; when all fail it returns `ModelError::RetryExhausted { attempts: max_retries + 1, last_error, provider }`.

### 2.2 `ModelCallResult`: Unified Streaming/Non-Streaming Return

```rust
pub enum ModelCallResult {
    Complete(ChatResponse),                                  // Non-streaming: one complete response
    Stream(Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>>),  // Streaming: incremental responses
}
```

When `stream_enabled()` is true, `call()` returns the `Stream` variant; each `ChatResponse` in the stream is a delta, with `is_last` marking the final one.

### 2.3 `ChatResponse`

| Field | Description |
|-------|-------------|
| `content: Vec<ContentBlock>` | Response content blocks (text/thinking/tool_call, etc.) |
| `is_last: bool` | The final chunk of a streaming sequence |
| `id` / `created_at` | Response ID (UUID) and RFC 3339 timestamp |
| `usage: Option<ChatUsage>` | Extended usage stats: `input_tokens`/`output_tokens`/`time`/`cache_creation_input_tokens`/`cache_input_tokens` (extends `message::Usage` with timing and cache stats) |
| `finished_reason: FinishedReason` | `completed` (default) / `interrupted` |
| `metadata` | Provider-specific metadata (e.g., tool-call extras) |

Provider implementations can use the builder methods `append_text`/`append_thinking`/`append_tool_call` to incrementally merge content blocks by `block_id`.

### 2.4 `StreamAccumulator`: O(n) Stream Accumulation

The standard tool for stream consumption: `new()` → call `append_chat_response(&delta)` for each chunk → `build()` to get the merged complete `ChatResponse`. Internally it buffers per block type (text/thinking/tool-call input accumulate separately), avoiding O(n²) string concatenation.

### 2.5 `ToolChoice` & `ModelCard`

- `ToolChoice`: tool selection configuration — `auto()` (default)/`none()`/`required()`/`specific_tool(name)`; `validate()` checks against available tool names, and `call()` fails fast with `ValidationError` on invalid input.
- `ModelCard`/`ModelStatus`: model cards (context window, parameter schema, and other metadata), parsed by `ChatModel::list_models()` from provider-supplied YAML values.
- `Formatter` trait: converts `Msg` lists to provider API message formats (and back); `FormatError` covers conversion failures.

## 3. Quick Example

The standard way to create a model in the shared example library — returns `Arc<DashScopeChatModel>`, ready for agent injection:

<!-- source: examples/common.rs:L34-L36 -->
```rust
pub fn create_model(api_key: &str, model_name: &str) -> Arc<DashScopeChatModel> {
    Arc::new(DashScopeChatModel::new(api_key, model_name))
}
```

The thinking-mode variant (enables reasoning output and forces streaming) is at `examples/common.rs` L43 (`create_model_with_thinking`); see [Event & Streaming](./event-streaming.md) for how the agent side consumes model streams.

## 4. Usage Patterns

### 4.1 Non-Streaming Call

```rust
let result = model.call(&messages, None, None).await?;
if let ModelCallResult::Complete(resp) = result {
    for block in &resp.content { /* read ContentBlocks */ }
    if let Some(u) = &resp.usage {
        println!("in={} out={}", u.input_tokens, u.output_tokens);
    }
}
```

### 4.2 Streaming Call + Accumulation

```rust
let result = model.call(&messages, None, None).await?;
if let ModelCallResult::Stream(mut stream) = result {
    let mut acc = StreamAccumulator::new();
    while let Some(chunk) = stream.next().await {
        let delta = chunk?;                 // Result<ChatResponse, ModelError>
        acc.append_chat_response(&delta);   // O(n) accumulation
        // or render incremental blocks in delta.content immediately
    }
    let full = acc.build();                 // merged complete response
}
```

### 4.3 Implementing a Custom Provider

Implement the three required `ChatModel` methods (`model_name`/`stream_enabled`/`call_api`), override retry configuration as needed, then inject as a trait object:

```rust
let model: Arc<dyn ChatModel> = Arc::new(MyProvider::new(...));
// Agent constructors accept Arc<dyn ChatModel>, decoupled from vendors
```

`call_api` returns `ModelError`; wrapping HTTP errors in `ModelError::ApiError { status, message, provider }` automatically gets `kind()` classification (401/403→`Authentication`, 429→`RateLimit`, 400/422→`BadRequest`, 5xx→`InternalServer`) for `retryable_errors()` matching.

### 4.4 Structured Output

`generate_structured_output(messages, &json_schema)` uses a tool-calling bypass by default: it injects a tool named `generate_structured_output`, forces `ToolChoice::required()`, and parses JSON from the tool-call input, with json_repair as a fallback. Note: **the default implementation does not support structured output over streaming calls** (returns `StructuredOutputError`).

### 4.5 Timeout & Cancellation

There is no built-in timeout at the trait level — wrap `call()` in `tokio::time::timeout` at the call site, or configure timeouts on the provider's HTTP client. Cancellation is implemented at the agent layer via `CancellationToken`; a cancelled call ends with `ModelError::Cancelled` (see [Agent System](./agent.md)).

## 5. Errors & Unsupported Capabilities

| Error variant | Trigger condition |
|---------------|-------------------|
| `ModelError::ApiError { status, message, provider }` | Provider returns an HTTP error; `kind()` classifies by status code |
| `ModelError::RetryExhausted { attempts, last_error, provider }` | Retries exhausted (`attempts` = `max_retries` + 1) |
| `ModelError::Cancelled` | Call cancelled (CancellationToken) |
| `ModelError::ValidationError { field, message }` | Argument validation failed (e.g., `tool_choice` names a non-existent tool, or messages is empty for structured output) |
| `ModelError::SerializationError` / `FormatError` | JSON serialization failure / Formatter message-format conversion failure |
| `ModelError::StructuredOutputError { reason }` | Structured output parsing failed, or the default implementation is used over streaming |
| `ModelError::UnsupportedFeature { feature, provider }` | Provider does not support the requested capability (Constitution Article V: explicit refusal instead of fake compatibility) |
| `ModelError::ConfigError { message }` | Configuration error (e.g., missing credentials) |

**Unsupported capabilities**: determined by each provider and returned explicitly as `UnsupportedFeature`; no fixed list is defined at this abstraction layer.

## 6. Compatibility

- **Compatibility level**: **L1** (field-by-field compatibility of core types and serialization protocol, 9 entries); **L2** (behaviorally equivalent trait call semantics/retry/counting, 34 entries)
- **Authoritative source**: `specs/001-compatibility-baseline/capability-matrix.json`
- **Known deviations**:
  - The matrix `status` field is currently `NOT_ANALYZED` for all entries (not backfilled after Features 001-017). Levels on this page are cross-verified against matrix `target_level` (model category: L1×9/L2×34) + `specs/003-model-api`, `specs/005-provider-extraction` + actual code state.
  - The default `count_tokens` is a bytes/4 heuristic (Python relies on precise tokenizers like tiktoken); providers may override — an explicit approximation deviation.
  - The default structured-output implementation is a tool-calling bypass and does not support streaming; this differs from Python's native JSON mode path.
  - `ChatUsage` extends the Python side with `time` and prompt-cache statistics fields.
- **Unsupported capabilities**: streaming structured output (default implementation, returns `StructuredOutputError`); anything else is declared explicitly by providers via `UnsupportedFeature`.

## 7. See Also

- [DashScope Provider](./dashscope.md) — the reference implementation of this trait
- [Event & Streaming](./event-streaming.md) — model-stream to agent-event conversion
- [Agent System](./agent.md) — the consumer of `Arc<dyn ChatModel>`
- [Message & Basic Types](./message-types.md) — the block types in `ChatResponse.content`
