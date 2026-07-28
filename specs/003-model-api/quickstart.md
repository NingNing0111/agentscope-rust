# Quickstart: Model API (Feature 003)

**Feature**: 003-model-api | **Date**: 2026-07-28

This guide demonstrates how to validate the Model API feature works end-to-end.

## Prerequisites

- Rust toolchain: 1.85+ (2024 edition)
- `agent_scope_types` and `agent_scope_message` crates compiled (Foundation Layer)
- `serde_yaml` for YAML model card loading
- `reqwest` for HTTP (mock server for tests, optional)

## Quick Validation Commands

### 1. Crate Compiles

```bash
# Add to workspace Cargo.toml members: ["crates/*"]
cargo build -p agent_scope_model
```

**Expected**: Compiles without errors. Dependency tree shows only Foundation crates.

### 2. Dependency Topology Check

```bash
cargo tree -p agent_scope_model --no-deps
```

**Expected**: Only `agent_scope_types`, `agent_scope_message`, `agent_scope_utils` (and framework deps like `serde`, `reqwest`, `tokio`).

### 3. Unit Tests

```bash
cargo test -p agent_scope_model
```

**Expected**: All tests pass. Key test modules:
- `response.rs` — ChatResponse append methods
- `usage.rs` — ChatUsage serialization round-trip
- `accumulator.rs` — StreamAccumulator build correctness
- `card.rs` — ModelCard from_yaml with overrides
- `openai/formatter.rs` — Formatter output validation

### 4. ChatResponse Round-Trip

```rust
// tests/model/chat_response_tests.rs
#[test]
fn test_chat_response_serde_round_trip() {
    let mut resp = ChatResponse::default();
    resp.append_text("Hello", None);
    resp.append_text(" World!", None);
    resp.is_last = true;

    let json = serde_json::to_string(&resp).unwrap();
    let parsed: ChatResponse = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.content.len(), 1);
    assert_eq!(parsed.get_text_content(""), "Hello World!");
    assert!(parsed.is_last);
    assert_eq!(parsed.response_type, "chat_response");
}
```

**Expected**: Round-trip preserves all fields.

### 5. StreamAccumulator Build

```rust
// tests/model/stream_accumulator_tests.rs
#[tokio::test]
async fn test_accumulator_text_streaming() {
    let mut acc = StreamAccumulator::new();

    let mut chunk1 = ChatResponse::default();
    chunk1.append_text("Hel", Some("t1"));

    let mut chunk2 = ChatResponse::default();
    chunk2.append_text("lo", Some("t1"));
    chunk2.usage = Some(ChatUsage {
        input_tokens: 10,
        output_tokens: 5,
        time: 1.5,
        ..Default::default()
    });

    acc.append_chat_response(&chunk1);
    acc.append_chat_response(&chunk2);

    let result = acc.build();
    assert!(result.is_last);
    assert_eq!(result.get_text_content(""), "Hello");
    assert_eq!(result.usage.unwrap().output_tokens, 5);
}
```

**Expected**: Two chunks merged → one TextBlock with "Hello", usage from last chunk.

### 6. ModelCard YAML Loading

```bash
# Create test model card
cat > /tmp/test_model.yaml << 'EOF'
name: "test-model-v1"
label: "Test Model"
status: "active"
input_types:
  - "text/plain"
output_types:
  - "text/plain"
context_size: 32768
output_size: 4096
parameter_overrides: {}
EOF
```

```rust
#[test]
fn test_model_card_from_yaml() {
    let param_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "temperature": {"type": "number", "minimum": 0.0, "maximum": 2.0}
        }
    });

    let card = ModelCard::from_yaml(
        "/tmp/test_model.yaml",
        &param_schema,
    ).unwrap();

    assert_eq!(card.name, "test-model-v1");
    assert_eq!(card.context_size, 32768);
    // parameter_schema should have merged temperature with output_size max
    let props = card.parameter_schema["properties"].as_object().unwrap();
    assert!(props.contains_key("temperature"));
}
```

**Expected**: ModelCard loaded with merged parameter schema.

### 7. Formatter Output Validation

```rust
#[tokio::test]
async fn test_openai_formatter_basic_msg() {
    let formatter = OpenAIChatFormatter::default();
    let msg = UserMsg::new("user", vec![TextBlock::new("Hello!")]).unwrap();

    let result = formatter.format(&[msg]).await.unwrap();

    assert_eq!(result.len(), 1);
    let user_msg = &result[0];
    assert_eq!(user_msg["role"], "user");
    // content should be "Hello!" string (not array for single text block)

    // For multimodal, content becomes array
    let img = DataBlock::new(Base64Source {
        data: base64::encode(b"fake_img"),
        media_type: "image/png".into(),
    }.into());
    let multi_msg = UserMsg::new("user", vec![
        TextBlock::new("Describe:"),
        img,
    ]).unwrap();
    let result = formatter.format(&[multi_msg]).await.unwrap();
    let content = &result[0]["content"];
    assert!(content.is_array());
}
```

**Expected**: Formatter produces OpenAI Chat Completions format.

### 8. Mock Model Integration Test

```rust
// tests/model/mock_model_tests.rs
struct MockModel {
    response: ChatResponse,
    stream: bool,
}

#[async_trait]
impl ChatModel for MockModel {
    // ... implement trait ...
    async fn call_api(...) -> ModelCallResult {
        if self.stream {
            ModelCallResult::Stream(Box::pin(stream::once(async { Ok(self.response.clone()) })))
        } else {
            ModelCallResult::Complete(self.response.clone())
        }
    }
}

#[tokio::test]
async fn test_mock_model_non_streaming() {
    let mut resp = ChatResponse::default();
    resp.append_text("Mock response", None);
    let model = MockModel { response: resp, stream: false };

    let result = model.call(&[], None, None).await.unwrap();
    match result {
        ModelCallResult::Complete(r) => assert_eq!(r.get_text_content(""), "Mock response"),
        _ => panic!("Expected Complete"),
    }
}
```

**Expected**: Mock model returns expected response via the trait.

## Compatibility Diff Test

After all implementation is done:

```bash
# Generate Python golden snapshots (in test fixture)
# Using the Python reference implementation's model module:
python generate_model_fixtures.py

# Run Rust diff test
cargo test -p agent_scope_model --test model_diff_tests
```

**Expected**: Rust ChatResponse/ChatUsage/ModelCard serialization matches Python golden snapshots (after timestamp/UUID normalization).

## Validation Checklist

- [ ] `cargo build -p agent_scope_model` passes
- [ ] `cargo test -p agent_scope_model` all tests pass
- [ ] `cargo tree -p agent_scope_model --no-deps` shows only Foundation deps
- [ ] `cargo clippy -p agent_scope_model -- -D warnings` passes
- [ ] `cargo fmt -p agent_scope_model -- --check` passes
- [ ] ChatResponse JSON format matches Python reference
- [ ] ChatUsage JSON format matches Python reference
- [ ] StreamAccumulator O(n) build produces correct merged content
- [ ] ModelCard YAML loading works with overrides and auto-filters
- [ ] OpenAIChatFormatter produces valid OpenAI API format
- [ ] Mock model integration test demonstrates trait usage
