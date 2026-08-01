# DashScope Provider

> One-liner: the reference `ChatModel`/`EmbeddingModel` implementation for Alibaba Cloud Model Studio (DashScope) — connects to Qwen-series models via the OpenAI-compatible endpoint, and is currently the only built-in model provider in the repository.

## 1. Overview

This module covers the `agent_scope_dashscope` crate at the provider layer (implementing `ChatModel` from `agent_scope_model` and `EmbeddingModel` from `agent_scope_embedding`). Chat goes through the OpenAI-compatible endpoint `/compatible-mode/v1/chat/completions`; embedding goes through the Text Embedding API.

**When to use**: chatting/reasoning with Qwen-series models (including thinking mode), text vectorization (the embedding stage of RAG pipelines).

**Prerequisites**: [Model Abstraction](./model.md) (trait semantics); for RAG scenarios see [RAG](./rag.md).

## 2. Core Concepts & Main Public Types

### 2.1 `DashScopeChatModel`

A public-field struct — modify fields directly after construction or use the chained methods:

| Member | Description | Default |
|--------|-------------|---------|
| `new(api_key, model_name)` | Constructor: `api_key: impl Into<String>`, `model_name: impl Into<String>` | — |
| `base_url` | OpenAI-compatible endpoint | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| `stream` | Streaming enabled by default | `true` |
| `max_retries` / `retry_delay` | Retry attempts / delay in seconds | `3` / `1.0` |
| `context_size` | Context window in tokens | `131072` |
| `parameters` | Generation parameters (see 2.2) | `DashScopeParameters::default()` |
| `extra_body` | Extra fields merged into every request body | empty |
| `with_base_url(...)` / `with_stream(bool)` | Chained configuration | — |

**Retryable errors** are configured as `ApiConnection`/`ApiTimeout`/`RateLimit`/`InternalServer` — these categories are retried automatically by `ChatModel::call()` (up to `max_retries` times); all others (e.g., 401 authentication, 400 bad request) return immediately.

### 2.2 `DashScopeParameters`

Generation parameters (`Option` fields are omitted from the request body when `None`):

| Parameter | Type | Description |
|-----------|------|-------------|
| `max_tokens` | `Option<u32>` | Maximum tokens to generate |
| `temperature` | `Option<f64>` | Sampling temperature (0–2) |
| `top_p` / `top_k` | `Option<f64>` / `Option<u32>` | Nucleus sampling / Top-K sampling |
| `enable_search` | `bool` (default `false`) | Web search augmentation (DashScope extension) |
| `enable_thinking` | `bool` (default `false`) | Thinking/reasoning mode (streams `reasoning_content` as `ThinkingBlock`s) |
| `thinking_budget` | `Option<u32>` | Thinking token budget; `Some(n)` caps it, `None` means uncapped (only meaningful when thinking is enabled) |
| `repetition_penalty` | `Option<f64>` | Repetition penalty, must be > 0 (checked by `validate()`) |
| `seed` | `Option<u64>` | Random seed ([0, 2³¹-1]) |
| `stop` | `Option<Vec<String>>` | Stop sequences |

**Parameter constraints** (`ParamError`): `repetition_penalty` must be positive; `enable_thinking=true` is incompatible with `tool_choice="required"`; `enable_search` is only supported by certain models.

### 2.3 `DashScopeEmbeddingModel`

Text vectorization (implements `agent_scope_embedding::EmbeddingModel`):

- `new(api_key: String, model_card: EmbeddingModelCard)` — the card carries model name, dimensions, and multimodal support
- `with_cache(cache: Arc<dyn EmbeddingCache>)` — attach a response cache
- `with_base_url(...)` — custom endpoint (default `https://dashscope.aliyuncs.com`)
- `embed()` returns `EmbeddingError::ApiKeyMissing` when the API key is empty (no panic)

### 2.4 Credential Configuration (Verified Facts)

The crate **does not read environment variables itself** — credentials are passed in explicitly by the caller (layered design). The repository examples follow this convention:

1. A root `.env` file contains `API_KEY=sk-...` (ignored by `.gitignore`), loaded at program entry via `dotenv::dotenv().ok();` (`examples/chat.rs:388`);
2. Most examples inject it via clap `#[arg(short = 'k', long, env = "API_KEY")]`;
3. Exception: `examples/chat.rs` only accepts `-k`/`--api-key` explicitly (no `env` attribute, `chat.rs:40`);
4. The examples default to model name `qwen-plus`.

## 3. Quick Example

The standard way to create a chat model (shared example library):

<!-- source: examples/common.rs:L34-L36 -->
```rust
pub fn create_model(api_key: &str, model_name: &str) -> Arc<DashScopeChatModel> {
    Arc::new(DashScopeChatModel::new(api_key, model_name))
}
```

The thinking-mode variant (`enable_thinking = true` + optional budget + forced streaming) is at `examples/common.rs` L43 (`create_model_with_thinking`); full embedding usage is in `examples/rag_test.rs` (`run_ingest_test` at L196).

## 4. Usage Patterns

### 4.1 Configuring Generation Parameters

`DashScopeChatModel` fields are public — modify `parameters` directly:

```rust
let mut model = DashScopeChatModel::new(api_key, "qwen-plus");
model.parameters.temperature = Some(0.7);
model.parameters.max_tokens = Some(2048);
model.parameters.validate()?; // optional: validate constraints before sending
```

### 4.2 Thinking (Reasoning) Mode

```rust
model.parameters.enable_thinking = true;
model.parameters.thinking_budget = Some(8192); // or None for uncapped
model.stream = true; // thinking content arrives as streaming ThinkingBlock deltas
```

Note: thinking mode is incompatible with `ToolChoice::required()` (`ParamError::ThinkingNotCompatibleWithRequired`).

### 4.3 Custom Endpoints & Extra Request Fields

Use `with_base_url` for regional endpoints/proxies/mock tests; `extra_body` merges arbitrary vendor extensions:

```rust
let model = DashScopeChatModel::new(api_key, "qwen-plus")
    .with_base_url("https://your-proxy.example.com/v1")
    .with_stream(false);
// model.extra_body.insert("vl_high_resolution_images".into(), json!(true));
```

### 4.4 Embedding Vectorization

```rust
let card = EmbeddingModelCard { name: "text-embedding-v4".into(), dimensions: 1024, .. };
let emb = DashScopeEmbeddingModel::new(api_key, card).with_cache(cache);
let resp = emb.embed(vec![EmbeddingInput::Text("hello".into())]).await?;
```

Embedding CLI conventions are in `examples/rag_test.rs` L35-L49 (`--embedding-model` defaults to env var `EMBEDDING_MODEL`, `--embedding-dims` to `EMBEDDING_DIMS`).

## 5. Errors & Unsupported Capabilities

| Error | Trigger condition |
|-------|-------------------|
| `ModelError::ApiError { status, .. }` | DashScope returns an HTTP error (401 invalid credentials, 429 rate limit, 5xx service failure, etc.; auto-classified by status code) |
| `ModelError::RetryExhausted` | Retryable errors exhausted (after 4 attempts by default) |
| `ModelError::ValidationError` | Pre-flight validation failures such as illegal `tool_choice` |
| `EmbeddingError::ApiKeyMissing` | Embedding called with an empty API key (returned at call time, no panic) |
| `ParamError::RepetitionPenaltyMustBePositive` | `repetition_penalty <= 0` |
| `ParamError::ThinkingNotCompatibleWithRequired` | Thinking mode combined with forced tool choice |
| `ParamError::EnableSearchNotSupported(model)` | The current model does not support web search |

**Unsupported capabilities**: per the `ChatModel` contract, unimplemented capabilities return `ModelError::UnsupportedFeature { feature, provider }` explicitly — no fake compatibility.

**FAQ**:

- *Empty/invalid API key*: the chat side gets a 401 from the server (`ApiError`, `kind()=Authentication`, not retried); the embedding side fails fast locally with `ApiKeyMissing`.
- *Where do credentials come from?*: the crate does not read env vars — load `.env` with `dotenv` at application entry and pass the key via clap/explicit arguments (see 2.4).

## 6. Compatibility

- **Compatibility level**: **L2** (behaviorally equivalent: OpenAI-compatible endpoint call semantics, parameter mapping, and error classification match the Python DashScope implementation; credential management LL1-L2)
- **Authoritative source**: `specs/001-compatibility-baseline/capability-matrix.json`
- **Known deviations**:
  - The matrix `status` field is currently `NOT_ANALYZED` for all entries (not backfilled after Features 001-017). Levels on this page are cross-verified against matrix `target_level` (credential category) + `specs/004-provider-architecture`, `specs/005-provider-extraction` + actual code state.
  - The crate does not read environment variables (some Python implementations auto-read `DASHSCOPE_API_KEY`) — credentials must be passed explicitly; a deliberate layered-design difference.
  - The embedding error type is `EmbeddingError::ApiKeyMissing`; its message mentions `DASHSCOPE_API_KEY`, but the actual credential source is up to the caller.
- **Unsupported capabilities**: unimplemented provider capabilities return `ModelError::UnsupportedFeature` explicitly (e.g., multimodal-input validation for specific models: `DataBlock` inputs are rejected when `supports_multimodal` is false).

## 7. See Also

- [Model Abstraction](./model.md) — the `ChatModel` trait and error classification
- [RAG](./rag.md) — the consumer of embedding models
- [Agent System](./agent.md) — the primary caller of models
