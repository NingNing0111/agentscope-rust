# Contract: ChatModel Trait

**Feature**: 003-model-api | **Version**: 0.1.0

## Trait Definition

```rust
use std::pin::Pin;
use std::path::Path;
use futures::stream::Stream;
use agent_scope_message::Msg;
use agent_scope_types::JsonValue;

/// The result of a model call — either a complete response or a stream.
pub enum ModelCallResult {
    Complete(ChatResponse),
    Stream(Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>>),
}

/// The unified interface for all chat model providers.
pub trait ChatModel: Send + Sync {
    // ── Core Call ──
    /// Call the model with retry logic.
    ///
    /// - `messages`: ordered conversation history
    /// - `tools`: optional list of tool JSON schemas
    /// - `tool_choice`: optional tool selection config
    ///
    /// When `self.stream_enabled()` is true, returns `ModelCallResult::Stream`.
    /// The stream yields incremental `ChatResponse` chunks; the final chunk
    /// has `is_last = true`.  If the stream is dropped / cancelled mid-flight,
    /// the final chunk's `finished_reason` is set to `Interrupted`.
    ///
    /// Retry behaviour: up to `self.max_retries()` additional attempts for
    /// errors matching `self.retryable_errors()`, with `self.retry_delay()`
    /// seconds between attempts.
    async fn call(
        &self,
        messages: &[Msg],
        tools: Option<&[JsonValue]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError>;

    // ── Configuration ──
    fn stream_enabled(&self) -> bool;
    fn max_retries(&self) -> u32;        // default: 3
    fn retry_delay(&self) -> f64;         // seconds, default: 1.0
    fn model_name(&self) -> &str;
    fn context_size(&self) -> usize;      // default: 32768

    /// Returns the error categories that should trigger an automatic retry.
    /// Default implementation returns an empty slice (no retries).
    fn retryable_errors(&self) -> &[ModelErrorKind] {
        &[]
    }

    // ── Token Counting ──
    /// Estimate the token count for the given messages and tools.
    /// Default: byte-length / 4 heuristic, each DataBlock adds 2000.
    async fn count_tokens(
        &self,
        messages: &[Msg],
        tools: Option<&[JsonValue]>,
    ) -> usize;

    // ── Structured Output ──
    /// Generate a structured (JSON Schema-constrained) output.
    /// Implemented via forced tool-calling by default; providers with native
    /// structured output support SHOULD override.
    async fn generate_structured_output(
        &self,
        messages: &[Msg],
        structured_model: &JsonValue,  // JSON Schema dict
    ) -> Result<StructuredResponse, ModelError>;

    // ── Model Discovery (static-like method) ──
    /// Scan the `_models/` YAML directory for model cards.
    /// `custom_yaml_dir` overrides the default directory (next to provider source).
    fn list_models(
        custom_yaml_dir: Option<&Path>,
        parameter_schema: &JsonValue,  // JSON Schema of Parameters struct
    ) -> Result<Vec<ModelCard>, ModelError>;

    // ── Validation ──
    /// Validate tool_choice against the available tools list.
    fn validate_tool_choice(
        tool_choice: Option<&ToolChoice>,
        tools: Option<&[JsonValue]>,
    ) -> Result<(), ModelError>;

    // ── Internal (not part of public contract) ──
    #[doc(hidden)]
    async fn _call_api(
        &self,
        model_name: &str,
        messages: &[Msg],
        tools: Option<&[JsonValue]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError>;
}
```

## Usage Examples

### Non-streaming call

```rust
let model: Arc<dyn ChatModel> = ...;
let msgs: Vec<Msg> = ...;

match model.call(&msgs, None, None).await? {
    ModelCallResult::Complete(resp) => {
        assert!(resp.is_last);
        println!("tokens: {:?}", resp.usage);
        println!("text: {:?}", resp.get_text_content("\n"));
    }
    ModelCallResult::Stream(_) => unreachable!(),
}
```

### Streaming call

```rust
let model: Arc<dyn ChatModel> = ...;
if let ModelCallResult::Stream(mut stream) = model.call(&msgs, None, None).await? {
    let mut accumulator = StreamAccumulator::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if chunk.is_last {
            let final_response = chunk;
            break;
        }
        accumulator.append_chat_response(&chunk);
    }
    let complete = accumulator.build();  // is_last=true, all content merged
}
```

### Structured output

```rust
use schemars::JsonSchema;

#[derive(JsonSchema)]
struct WeatherOutput {
    city: String,
    temperature: f64,
    condition: String,
}

let schema = schemars::schema_for!(WeatherOutput);
let result = model.generate_structured_output(&msgs, &serde_json::to_value(&schema)?).await?;
let weather: WeatherOutput = serde_json::from_value(result.content)?;
```

## Invariants

1. `call()` MUST NOT panic — all errors returned via `Result`.
2. `call()` with `stream_enabled()=true` MUST return `ModelCallResult::Stream`.
3. `call()` with `stream_enabled()=false` MUST return `ModelCallResult::Complete`.
4. `generate_structured_output()` MUST NOT be called with empty messages (returns `ValidationError`).
5. `validate_tool_choice()` MUST be called before forwarding `tool_choice` to the API.
6. When the stream returned by `call()` is dropped, the inflight HTTP request SHOULD be aborted.
