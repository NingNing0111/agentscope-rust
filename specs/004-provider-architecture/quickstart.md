# Quickstart: Provider Architecture & DashScope (Feature 004)

**Feature**: 004-provider-architecture | **Date**: 2026-07-28

## Prerequisites

- Rust toolchain: 1.85+ (2024 edition)
- Feature 003 (agent_scope_model) implemented and tested
- `cargo build -p agent_scope_model` passes (current state)

## Quick Validation Commands

### 1. Verify Core Crate Purity (after split)

```bash
# agent_scope_model MUST NOT depend on reqwest
cargo tree -p agent_scope_model --no-dedupe | grep -q reqwest && echo "FAIL" || echo "PASS"

# Core tests still pass
cargo test -p agent_scope_model
```

**Expected**: "PASS" and all 56 core tests pass.

### 2. OpenAI Crate Compiles (after extraction)

```bash
cargo build -p agent_scope_openai
cargo test -p agent_scope_openai
```

**Expected**: Compiles and all 10 original OpenAI tests pass.

### 3. DashScope Crate Compiles

```bash
cargo build -p agent_scope_dashscope
cargo test -p agent_scope_dashscope
```

**Expected**: Compiles and all mock HTTP tests pass (at least 10 tests).

### 4. Mock HTTP Test Pattern (DashScope example)

```rust
// tests/model_tests.rs in agent_scope_dashscope
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};

#[tokio::test]
async fn test_dashscope_non_streaming_call() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/compatible-mode/v1/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({
                "choices": [{
                    "message": {"role": "assistant", "content": "Hello from Qwen!"},
                    "finish_reason": "stop"
                }],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            })))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen-plus")
        .with_base_url(mock_server.uri());

    let result = model.call(&[/* Msg */], None, None).await.unwrap();
    match result {
        ModelCallResult::Complete(resp) => {
            assert_eq!(resp.get_text_content(""), "Hello from Qwen!");
        }
        _ => panic!("Expected Complete"),
    }
}
```

### 5. Integration Test: Provider Agnostic Usage

```rust
// Application code using any Provider via trait object
use agent_scope_model::ChatModel;
use agent_scope_dashscope::DashScopeChatModel;
use std::sync::Arc;

let model: Arc<dyn ChatModel> = Arc::new(
    DashScopeChatModel::new("sk-xxx", "qwen-plus")
);

match model.call(&messages, None, None).await? {
    ModelCallResult::Complete(resp) => println!("{}", resp.get_text_content("")),
    ModelCallResult::Stream(stream) => { /* consume stream */ }
}
```

### 6. Dependency Topology Check (overall)

```bash
# Verify no Provider crate pollutes core
cargo tree -p agent_scope_model --no-dedupe | grep -E "openai|dashscope" && echo "FAIL" || echo "PASS"

# Verify Provider crates only depend on Foundation
cargo tree -p agent_scope_dashscope --no-dedupe | grep -E "agent_scope_tool|agent_scope_agent" && echo "FAIL" || echo "PASS"
```

**Expected**: Both commands output "PASS".

## Validation Checklist

- [ ] `cargo build -p agent_scope_model` passes (no reqwest in deps)
- [ ] `cargo test -p agent_scope_model` — 56 core tests pass
- [ ] `cargo build -p agent_scope_openai` — independent compilation
- [ ] `cargo test -p agent_scope_openai` — 10 tests pass
- [ ] `cargo build -p agent_scope_dashscope` — independent compilation
- [ ] `cargo test -p agent_scope_dashscope` — ≥10 mock tests pass
- [ ] `cargo clippy --workspace -- -D warnings` — all crates clean
- [ ] `cargo fmt --all -- --check` — all crates formatted
- [ ] `cargo tree -p agent_scope_model` shows zero Provider deps
- [ ] `cargo tree -p agent_scope_dashscope` shows only Foundation deps
- [ ] Any `Arc<dyn ChatModel>` can be constructed from any Provider
