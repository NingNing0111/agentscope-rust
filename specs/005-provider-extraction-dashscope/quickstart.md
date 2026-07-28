# Quickstart: Provider 剥离与 DashScope (Feature 005)

**Feature**: 005-provider-extraction-dashscope | **Date**: 2026-07-29

## Prerequisites

- Rust toolchain: 1.85+ (2024 edition)
- Feature 003 (`agent_scope_model`) implemented and tested
- `cargo build -p agent_scope_model` passes (current state, before changes)

## Quick Validation Commands

### 1. Core Crate Purity — No reqwest

```bash
# agent_scope_model MUST NOT depend on reqwest after cleanup
cargo tree -p agent_scope_model --no-dedupe | grep -q reqwest && echo "FAIL: reqwest in core" || echo "PASS"
```

**Expected**: `PASS`

### 2. Core Crate Purity — No Provider Code

```bash
# agent_scope_model MUST NOT have openai module
cargo doc -p agent_scope_model --no-deps 2>&1 | grep -i openai > /dev/null && echo "FAIL: OpenAI ref in core" || echo "PASS"
```

**Expected**: `PASS`

### 3. Core Tests Still Pass

```bash
cargo test -p agent_scope_model
```

**Expected**: All non-OpenAI tests pass. `formatter_integration.rs` should be gone.

### 4. DashScope Crate Compiles

```bash
cargo build -p agent_scope_dashscope
cargo test -p agent_scope_dashscope
```

**Expected**: Compiles and all mock HTTP tests pass (≥10 tests).

### 5. Workspace-wide Clippy & Fmt

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

**Expected**: Both pass without errors.

### 6. DashScope Non-Streaming Mock Test (example pattern)

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
                    "message": {"role": "assistant", "content": "你好！"},
                    "finish_reason": "stop"
                }],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
            })))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen-plus")
        .with_base_url(mock_server.uri());

    let result = model.call(&[/* Msg */], None, None).await.unwrap();
    // verify ChatResponse
}
```

### 7. Dependency Topology Verification

```bash
# Core crate MUST NOT depend on any Provider crate
cargo tree -p agent_scope_model --no-dedupe | grep -iE "openai|dashscope" && echo "FAIL" || echo "PASS"

# DashScope crate MUST only depend on Foundation + reqwest
cargo tree -p agent_scope_dashscope --no-dedupe | grep -E "agent_scope_tool|agent_scope_agent" && echo "FAIL" || echo "PASS"
```

**Expected**: Both commands output `PASS`.

## Validation Checklist

- [ ] `cargo build -p agent_scope_model` passes (no reqwest in deps)
- [ ] `cargo tree -p agent_scope_model` shows zero `reqwest`/`openai`/`dashscope`
- [ ] `cargo test -p agent_scope_model` — core tests pass (no OpenAI-specific failures)
- [ ] `agent_scope_model/src/openai/` directory no longer exists
- [ ] `agent_scope_model/src/lib.rs` has no `pub mod openai` or OpenAI re-exports
- [ ] `cargo build -p agent_scope_dashscope` — independent compilation
- [ ] `cargo test -p agent_scope_dashscope` — ≥10 mock tests pass
- [ ] `cargo clippy --workspace -- -D warnings` — all crates clean
- [ ] `cargo fmt --all -- --check` — all crates formatted
- [ ] `cargo tree -p agent_scope_dashscope` shows only Foundation deps + reqwest
- [ ] `Arc<dyn ChatModel>` can be constructed from `DashScopeChatModel`
