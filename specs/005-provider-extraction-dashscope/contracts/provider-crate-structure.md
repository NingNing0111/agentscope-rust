# Contract: Provider Crate Structure

**Feature**: 005-provider-extraction-dashscope | **Version**: 0.1.0

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
│   └── parameters.rs   # XxxParameters struct + schemars::JsonSchema derive
└── tests/
    ├── model_tests.rs      # Mock HTTP tests for ChatModel impl
    ├── formatter_tests.rs  # Formatter output validation
    └── parameters_tests.rs # Parameters serde round-trip
```

## 3. Required Public API Surface

Each Provider crate MUST re-export:

```rust
pub mod model;
pub mod formatter;
pub mod parameters;

pub use model::XxxChatModel;
pub use formatter::XxxFormatter;
pub use parameters::XxxParameters;
```

## 4. Dependency Contract

Provider crate MUST:
- Depend on `agent_scope_model` (for `ChatModel`, `ChatResponse`, `ModelError`, `ToolChoice`, `Formatter`, `StreamAccumulator`)
- Depend on `agent_scope_message` (for `Msg`, `ContentBlock`, `DataBlock`)
- Depend on `agent_scope_types` (基础类型)
- NOT depend on `agent_scope_tool`, `agent_scope_agent`, or any non-Foundation crate
- Use `reqwest` 0.12 (stream + json features) as HTTP client
- Use `tokio` 1.x as async runtime
- NOT re-export Foundation types (users import from Foundation crates directly)

## 5. ChatModel Trait Implementation Contract

Each Provider's `XxxChatModel` MUST implement ALL required methods:

| Method | Requirement |
|--------|-------------|
| `model_name()` | Return model identifier string |
| `stream_enabled()` | Return `self.stream` |
| `call_api()` | HTTP POST + response/stream parsing |
| `retryable_errors()` | Return retryable error categories |
| `max_retries()` | Override if provider-specific |
| `retry_delay()` | Override if provider-specific |
| `context_size()` | Return model-specific context window |
| `count_tokens()` | Provider-specific tokenizer or fallback |

## 6. Core Crate Contract (agent_scope_model)

After cleanup, `agent_scope_model` MUST:
- NOT contain any `mod openai` or OpenAI re-exports in `lib.rs`
- NOT depend on `reqwest`, `tokio-stream`, `tokio-util`, `serde_yaml`, `thiserror` in `Cargo.toml`
- Retain `futures` dependency (needed for `Pin<Box<dyn Stream>>` in `ChatModel` trait definition)
- Expose `ModelCard::from_raw(yaml_str: &str)` (replacing `from_yaml(path: &Path)`)
- Retain all other public API unchanged（`ChatModel`, `ChatResponse`, `Formatter`, `ModelError`, etc.）

## 7. Workspace-level Constraints

- Root `Cargo.toml` uses `members = ["crates/*"]` — new crates auto-register
- All crates use `edition = "2024"` (workspace-level)
- All crates use `#![deny(unsafe_code)]`
- `cargo test --workspace` MUST pass without network
