# Data Model: Model API (Feature 003)

**Date**: 2026-07-28 | **Spec**: [spec.md](./spec.md)

## Entity Overview

```
ChatModel (trait)
├── call() → ModelCallResult
│   ├── ChatResponse (non-streaming)
│   └── Stream<ChatResponse> (streaming)
├── count_tokens() → usize
├── generate_structured_output() → StructuredResponse
├── list_models() → Vec<ModelCard>
└── retry state: max_retries, retry_delay

StreamAccumulator (internal)
├── _AccTextBlock      → Vec<String> → TextBlock
├── _AccThinkingBlock  → Vec<String> → ThinkingBlock
├── _AccToolCallBlock  → Vec<String> → ToolCallBlock
├── _AccDataBlock      → AccBase64Source → DataBlock
└── build() → ChatResponse (is_last=true)
```

---

## 1. ChatResponse

The streaming/non-streaming model response.

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `content` | `Vec<ContentBlock>` | yes | `[]` | TextBlock, ThinkingBlock, ToolCallBlock, DataBlock |
| `is_last` | `bool` | yes | — | Whether this is the final accumulated response |
| `id` | `String` | yes (auto) | `uuid v4` | Unique response identifier |
| `created_at` | `String` | yes (auto) | ISO 8601 now | Creation timestamp |
| `type` | `String` | yes (auto) | `"chat_response"` | Discriminant value |
| `usage` | `Option<ChatUsage>` | no | `None` | Token usage stats |
| `finished_reason` | `FinishedReason` | yes | `COMPLETED` | Why the response ended |
| `metadata` | `HashMap<String, JsonValue>` | yes | `{}` | Additional metadata |

**Serialization tag**: `"type": "chat_response"` (fixed literal).

### FinishedReason

| Variant | Serde Value |
|---------|-------------|
| `Completed` | `"completed"` |
| `Interrupted` | `"interrupted"` |

**Note**: Distinct from `agent_scope_types::ReplyFinishedReason` (4 variants). `FinishedReason` is model-call-level; `ReplyFinishedReason` is reply-level (may span multiple model calls).

### Incremental Methods

| Method | Signature | Behavior |
|--------|-----------|----------|
| `append_text` | `(text: &str, block_id: Option<&str>)` | Match TextBlock by id → append text, or create new |
| `append_thinking` | `(thinking: &str, block_id: Option<&str>, extra: HashMap<String, JsonValue>)` | Match → append thinking + merge extras, or create new |
| `append_tool_call` | `(block_id: &str, name: &str, input: &str, extra: HashMap<String, JsonValue>)` | Match by id → append input + merge extras, or create new |
| `append_data_block` | `(block_id: &str, data: &[u8], media_type: &str, name: Option<&str>)` | Match by id → decode-concat-re-encode for audio/*, replace for others |
| `append_chat_response` | `(other: &ChatResponse)` | Merge by block_id, append new blocks, update usage |

### Content Validation (NO runtime parse of ToolCall input)

- ToolCallBlock.input stored as raw JSON string
- No JSON validity check at ChatResponse level
- Validation happens at higher layers (tool execution)

### Concrete Type of `content`

| Content Block Type | tag value |
|--------------------|-----------|
| `TextBlock` | `"text"` |
| `ThinkingBlock` | `"thinking"` |
| `ToolCallBlock` | `"tool_call"` |
| `DataBlock` | `"data"` |

(These types are defined in `agent_scope_message` crate)

---

## 2. StructuredResponse

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `content` | `serde_json::Value` (object) | yes | — | Structured output data |
| `id` | `String` | yes (auto) | uuid v4 | Response identifier |
| `created_at` | `String` | yes (auto) | ISO 8601 now | Creation timestamp |
| `type` | `String` | yes (auto) | `"structured_response"` | Fixed discriminant |
| `usage` | `Option<ChatUsage>` | no | `None` | Token usage |
| `finished_reason` | `FinishedReason` | yes | `COMPLETED` | End reason |
| `metadata` | `HashMap<String, JsonValue>` | yes | `{}` | Additional metadata |

**Serialization tag**: `"type": "structured_response"`.

---

## 3. ChatUsage

Extended token statistics.

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `input_tokens` | `i64` | yes | — | Prompt token count |
| `output_tokens` | `i64` | yes | — | Completion token count |
| `time` | `f64` | yes | — | Wall clock time in seconds |
| `cache_creation_input_tokens` | `i64` | yes | `0` | Tokens used to create prompt cache |
| `cache_input_tokens` | `i64` | yes | `0` | Tokens read from prompt cache |
| `type` | `String` | yes (auto) | `"chat"` | Fixed discriminant |
| `metadata` | `Option<HashMap<String, JsonValue>>` | no | `None` | Additional metadata |

**Differences from `agent_scope_message::Usage`** (which only has `input_tokens` + `output_tokens`):
- Added `time` (request duration)
- Added `cache_creation_input_tokens` and `cache_input_tokens` (prompt caching)
- Added `type` discriminant and optional `metadata`

---

## 4. ChatModel (trait)

```rust
pub trait ChatModel: Send + Sync {
    type Parameters: serde::Serialize + Clone + Send + Sync + 'static;

    // Core call entry point (implements retry + cancel logic)
    async fn call(
        &self,
        messages: &[Msg],
        tools: Option<&[serde_json::Value]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError>;

    // Provider-specific API implementation (abstract)
    async fn call_api(
        &self,
        model_name: &str,
        messages: &[Msg],
        tools: Option<&[serde_json::Value]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError>;

    // Retry configuration
    fn max_retries(&self) -> u32;
    fn retry_delay(&self) -> f64;  // seconds
    fn retryable_errors(&self) -> &[ModelErrorKind];  // error categories that trigger retry

    // Streaming mode
    fn stream_enabled(&self) -> bool;

    // Token counting (byte/4 heuristic by default)
    async fn count_tokens(
        &self,
        messages: &[Msg],
        tools: Option<&[serde_json::Value]>,
    ) -> usize;

    // Structured output generation
    async fn generate_structured_output(
        &self,
        messages: &[Msg],
        structured_model: &serde_json::Value,  // JSON Schema dict
    ) -> Result<StructuredResponse, ModelError>;

    // Model discovery (class method equivalent — static)
    fn list_models(
        custom_yaml_dir: Option<&Path>,
    ) -> Result<Vec<ModelCard>, ModelError>;

    // Tool choice validation
    fn validate_tool_choice(
        tool_choice: Option<&ToolChoice>,
        tools: Option<&[serde_json::Value]>,
    ) -> Result<(), ModelError>;
}
```

### ModelCallResult

```rust
pub enum ModelCallResult {
    Complete(ChatResponse),        // Non-streaming response
    Stream(Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>>),  // Streaming
}
```

### Trait Object Usage

```rust
// Consumer code:
Arc<dyn ChatModel<Parameters = ???>>
```

**Problem**: `ChatModel` has an associated type `Parameters`, which makes `dyn ChatModel` impossible without specifying the type.

**Solutions considered and selected**:
- **Option A (Selected)**: Use an erased `dyn Any` parameter pattern — `call()` takes `&dyn Any` as parameter arg, concrete implementation downcasts internally.
- **Option B**: Remove `Parameters` from trait, make each provider's constructor handle parameters directly.
- **Option C**: Use `Arc<dyn ChatModel>` only after wrapping in an erased facade type.

**Decision**: This will be resolved in implementation phase. The spec's core type definitions (ChatResponse, ChatUsage, etc.) are independent of this question.

---

## 5. ToolChoice

Self-contained in model crate. No dependency on tool crate.

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `mode` | `String` | yes | — | `"auto"`, `"none"`, `"required"`, or a specific tool name |
| `tools` | `Option<Vec<String>>` | no | `None` | Tool name filter list |

**Validation logic** (in `_validate_tool_choice`):
1. If `tools` is `Some(list)`, validate each name exists in available tools
2. If `mode` not in `{"auto", "none", "required"}`, treat as a tool name and validate it exists in available tools (using `tools` list or full available tools)

---

## 6. Formatter (trait)

```rust
pub trait Formatter: Send + Sync {
    // Return supported media type patterns (from input_types, excluding text/plain and application/x-thinking)
    fn supported_input_media_types(&self) -> &[String];

    // Convert Msg list to API-format JSON list
    async fn format(
        &self,
        msgs: &[Msg],
    ) -> Result<Vec<serde_json::Value>, FormatError>;

    // Separate multimodal data from tool results; return (text_repr, promoted_blocks)
    fn convert_tool_result_to_string(
        &self,
        output: &ToolOutput,  // String or Vec<DataBlock|TextBlock>
    ) -> (String, Vec<ContentBlock>);

    // Group messages: (group_type, msgs) pairs
    fn group_messages(
        &self,
        msgs: &[Msg],
    ) -> Vec<(MessageGroup, Vec<&Msg>)>;
}

pub enum MessageGroup {
    ToolSequence,
    AgentMessage,
}
```

### FormatError

```rust
pub enum FormatError {
    InvalidMessage(String),
    UnsupportedMediaType(String),
    Io(std::io::Error),
    Base64Decode(base64::DecodeError),
}
```

---

## 7. ModelCard

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `type` | `String` | yes (auto) | `"chat_model"` | Fixed discriminant |
| `name` | `String` | yes | — | Model name (e.g., `gpt-4.1`) |
| `label` | `String` | yes | — | Frontend display name |
| `status` | `ModelStatus` | yes | — | active / deprecated / sunset |
| `deprecated_at` | `Option<DateTime<Utc>>` | no | `None` | Deprecation timestamp |
| `input_types` | `Vec<String>` | yes | `["text/plain"]` | Supported input MIME patterns |
| `output_types` | `Vec<String>` | yes | `["text/plain"]` | Supported output MIME patterns |
| `context_size` | `i64` | yes | — | Token context window size (>0) |
| `output_size` | `i64` | yes | — | Max output tokens (>0) |
| `parameter_schema` | `serde_json::Value` | yes | — | JSON Schema for parameters |
| `parameters_overrides` | `HashMap<String, serde_json::Value>` | yes | `{}` | Parameter override configs |

### ModelStatus

| Variant | Description |
|---------|-------------|
| `Active` | Currently available |
| `Deprecated` | Will be removed in future |
| `Sunset` | No longer available |

### from_yaml Logic

1. Load YAML file → `serde_yaml::from_reader()`
2. Get base parameter JSON Schema from `Parameters` struct via `schemars::JsonSchema`
3. Apply auto-filters:
   - If `output_types` doesn't contain `"application/x-thinking"` → remove `thinking_enable`, `thinking_budget`
   - If `output_types` doesn't contain `audio/*` → remove `voice`
4. Apply `parameter_overrides`:
   - `null` → remove parameter
   - `{"hidden": true}` → remove parameter
   - Otherwise → merge override values into parameter schema
5. Set `max_tokens.maximum` from `output_size`
6. Build `ModelCard` with final schema

---

## 8. StreamAccumulator (internal)

Structure for efficient O(n) accumulation of streaming deltas.

### Internal Accumulator Types

```rust
struct AccTextBlock {
    text: Vec<String>,        // delta fragments
    id: String,
    created_at: String,
    // ... other TextBlock fields copied from first delta
}

struct AccThinkingBlock {
    thinking: Vec<String>,    // delta fragments
    extras: HashMap<String, JsonValue>,  // provider fields (last non-None wins)
    // ...
}

struct AccToolCallBlock {
    input: Vec<String>,       // JSON fragment list
    name: String,             // from first non-empty name
    // ...
}

struct AccBase64Source {
    data: Vec<Vec<u8>>,       // raw byte fragments
    media_type: String,
}

struct AccDataBlock {
    source: AccDataSource,
    name: Option<String>,
    // ...
}

enum AccDataSource {
    Audio(AccBase64Source),   // streamable → acc then encode
    Other(DataSource),        // non-streamable → latest wins
}
```

### accumulator_append logic

| Delta block type | Accumulator match | Action |
|-----------------|-------------------|--------|
| TextBlock | Existing `AccTextBlock` by id | Push `delta.text` to fragment list |
| ThinkingBlock | Existing `AccThinkingBlock` by id | Push `delta.thinking` to list, merge extras |
| ToolCallBlock | Existing `AccToolCallBlock` by id | Push `delta.input` to list, update name if empty |
| DataBlock (audio/*) | Existing `AccDataBlock` with `Audio` source, matching media_type | Append decoded bytes to `data` list |
| DataBlock (other) | Existing or new `AccDataBlock` with `Other` source | Replace source entirely |
| Type mismatch | Different block type for same id | Warn log, drop old accumulator, seed new one |

### build()

```rust
fn build(self) -> ChatResponse {
    let content: Vec<ContentBlock> = self.blocks.into_iter().map(|(_, acc)| {
        match acc {
            AccBlock::Text(a)   => TextBlock { text: a.text.concat(), .. }.into(),
            AccBlock::Thinking(a) => ThinkingBlock { thinking: a.thinking.concat(), .. }.into(),
            AccBlock::ToolCall(a) => ToolCallBlock { input: a.input.concat(), .. }.into(),
            AccBlock::Data(a)   => match a.source {
                AccDataSource::Audio(audio) => DataBlock {
                    source: Base64Source {
                        data: base64::encode(&audio.data.concat()),
                        media_type: audio.media_type,
                    }.into(),
                    ..
                }.into(),
                AccDataSource::Other(src) => DataBlock { source: src, .. }.into(),
            },
        }
    }).collect();

    ChatResponse {
        content,
        is_last: true,
        id: self.id.unwrap_or_default(),
        usage: self.usage,
        finished_reason: self.finished_reason,
        ..Default::default()
    }
}
```

---

## 9. ModelError

```rust
pub enum ModelError {
    ApiError {
        status: u16,
        message: String,
        provider: String,
    },
    RetryExhausted {
        attempts: u32,
        last_error: Box<ModelError>,
        provider: String,
    },
    Cancelled,
    ValidationError {
        field: String,
        message: String,
    },
    SerializationError {
        context: String,
        source: serde_json::Error,
    },
    FormatError {
        context: String,
        source: FormatError,
    },
    StructuredOutputError {
        reason: String,
    },
    UnsupportedFeature {
        feature: String,
        provider: String,
    },
    ConfigError {
        message: String,
    },
}
```

## State Transitions

### ToolCall during streaming (handled by Event layer, not Model layer)

Model layer only produces `ToolCallBlock` with raw input; state management is handled by `agent_scope_event`'s `append_event`.

### Retry Machine

```
call() called
    │
    ▼
[attempt = 0]
    │
    ▼
_call_api() ──success──▶ return response
    │
    ▼ (error)
is retryable? ──no──▶ propagate error
    │ yes
    ▼
attempt < max_retries? ──no──▶ return RetryExhausted
    │ yes
    ▼
sleep(retry_delay)
    │
    ▼
attempt += 1 → loop
```
