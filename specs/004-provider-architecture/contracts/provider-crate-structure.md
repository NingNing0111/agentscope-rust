# Contract: Provider Crate Structure

**Feature**: 004-provider-architecture | **Version**: 0.1.0

## 1. Crate Naming & Registration

```
crate name: agent_scope_<provider>
directory:  crates/agent_scope_<provider>/
workspace:   auto-registered via "crates/*" glob in root Cargo.toml
```

## 2. Standard Crate Layout

```
crates/agent_scope_<provider>/
├── Cargo.toml          # deps: agent_scope_model, reqwest, tokio-stream, serde, ...
├── src/
│   ├── lib.rs          # pub mod model/formatter/parameters; re-exports
│   ├── model.rs        # XxxChatModel struct + ChatModel trait impl
│   ├── formatter.rs    # XxxFormatter struct + Formatter trait impl
│   ├── parameters.rs   # XxxParameters struct + schemars::JsonSchema derive
│   └── _models/        # Optional: YAML model cards for list_models()
└── tests/
    ├── model_tests.rs      # Mock HTTP tests for ChatModel impl
    ├── formatter_tests.rs  # Formatter output validation
    └── parameters_tests.rs # Parameters serde round-trip
```

## 3. Required Public API Surface

Each Provider crate MUST re-export:

```rust
pub use model::XxxChatModel;
pub use formatter::XxxFormatter;
pub use parameters::XxxParameters;
```

## 4. Dependency Contract

Provider crate MUST:
- Depend on `agent_scope_model` (for `ChatModel`, `ChatResponse`, `ModelError`, `ToolChoice`, `Formatter`, `StreamAccumulator`)
- Depend on `agent_scope_message` (for `Msg`, `ContentBlock`, `DataBlock`, etc.)
- NOT depend on `agent_scope_tool` or any other non-Foundation crate
- Use `reqwest` 0.12 (stream + json features) as HTTP client
- Use `tokio` 1.x as async runtime
- NOT re-export Foundation types (users import from Foundation crates directly)

## 5. ChatModel Trait Implementation Contract

Each Provider's `XxxChatModel` MUST implement ALL required methods:

| Method | Requirement |
|--------|-------------|
| `model_name()` | Return the model identifier string |
| `stream_enabled()` | Return `self.stream` |
| `call_api()` | HTTP POST + response parsing |
| `retryable_errors()` | Return retryable error categories |
| `max_retries()` | Override if provider-specific |
| `retry_delay()` | Override if provider-specific |
| `context_size()` | Override with model-specific value |
| `count_tokens()` | Override with provider-specific tokenizer or fallback to byte/4 |
