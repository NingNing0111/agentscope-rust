//! T015 — OpenAI 后端 wiremock 集成测试。
//!
//! mock HTTP 回放固定 OpenAI-compatible 响应，验证：
//! - 请求体形状（model/messages/tools/tool_choice → rig `CompletionRequest`）；
//! - 非流式响应解析（`ChatResponse` 文本）；
//! - 流式 SSE chunk 拼接；
//! - 错误分类（401→Authentication、429→RateLimit、500→InternalServer）。
//!
//! 对照 `specs/034-rig-llm-integration/contracts/provider-adapter.md` §2/§4。

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use agent_scope_message::factory::user_msg;
use agent_scope_model::model_error::{ModelError, ModelErrorKind};
use agent_scope_model::model_trait::{ChatModel, ModelCallResult};
use agent_scope_model::tool_choice::ToolChoice;
use agent_scope_rig::RigChatModel;

/// 标准非流式 OpenAI 补全响应。
fn openai_completion_body(text: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "chatcmpl-mock-001",
        "object": "chat.completion",
        "created": 1710000000,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": text },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 12,
            "completion_tokens": 8,
            "total_tokens": 20
        }
    })
}

/// OpenAI 标准 SSE 流式响应（两文本 chunk + finish + usage + [DONE]）。
fn openai_sse_body() -> String {
    concat!(
        "data: {\"id\":\"chatcmpl-mock-002\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"chatcmpl-mock-002\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" World\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"chatcmpl-mock-002\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: {\"id\":\"chatcmpl-mock-002\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4o-mini\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}\n\n",
        "data: [DONE]\n\n"
    )
    .to_string()
}

/// 组装 wiremock 环境 + `RigChatModel`（OpenAI，指向 mock）。
async fn setup_model(stream: bool) -> (MockServer, RigChatModel) {
    let mock_server = MockServer::start().await;
    let model = RigChatModel::openai("test-key", "gpt-4o-mini")
        .unwrap()
        .with_base_url(mock_server.uri())
        .with_stream(stream);
    (mock_server, model)
}

fn weather_tool() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get weather for a city",
            "parameters": {
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }
        }
    })
}

/// 解包错误（`ModelCallResult` 无 Debug，`expect_err` 不可用）。
fn unwrap_err(result: Result<ModelCallResult, ModelError>) -> ModelError {
    match result {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

#[tokio::test]
async fn request_body_shape_and_non_streaming_response() {
    let (mock_server, model) = setup_model(false).await;
    let model = model.with_max_retries(0);

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(openai_completion_body("Hello! I am GPT.")),
        )
        .mount(&mock_server)
        .await;

    let msg = user_msg("user", "Hi!").unwrap();
    let tools = vec![weather_tool()];
    let tool_choice = ToolChoice::required();
    let result = model.call(&[msg], Some(&tools), Some(&tool_choice)).await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
    if let Ok(ModelCallResult::Complete(resp)) = result {
        assert!(resp.is_last);
        // rig 0.41 的 OpenAI Chat Completions 硬编码 message_id=None（仅
        // Responses API 填充，见 rig-core completion/mod.rs:1215）。D5 原则：
        // 不捏造随机 id → 应为空串。
        assert!(
            resp.id.is_empty(),
            "Chat Completions has no msg_ id, got {:?}",
            resp.id
        );
        let text = resp.get_text_content("");
        assert!(text.contains("Hello"), "Expected greeting, got: {text}");
        assert_eq!(resp.usage.as_ref().map(|u| u.input_tokens), Some(12));
    } else {
        panic!("Expected Complete response");
    }

    // ── 请求体形状断言 ────────────────────────────────────────────────
    let requests = mock_server
        .received_requests()
        .await
        .expect("request recorded");
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("json body");
    assert_eq!(body["model"], "gpt-4o-mini");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "Hi!");
    assert_eq!(body["tools"][0]["function"]["name"], "get_weather");
    // ToolChoice::required → OpenAI "required"。
    assert_eq!(body["tool_choice"], "required");
}

#[tokio::test]
async fn streaming_chunks_are_concatenated() {
    let (mock_server, model) = setup_model(true).await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(openai_sse_body(), "text/event-stream"),
        )
        .mount(&mock_server)
        .await;

    let msg = user_msg("user", "Hi!").unwrap();
    let result = model.call(&[msg], None, None).await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
    if let Ok(ModelCallResult::Stream(stream)) = result {
        use futures::StreamExt;
        let mut text = String::new();
        let mut is_last_seen = false;
        let mut chunks = 0usize;
        futures::pin_mut!(stream);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.expect("chunk ok");
            if chunk.is_last {
                is_last_seen = true;
            }
            text.push_str(&chunk.get_text_content(""));
            chunks += 1;
        }
        assert_eq!(text, "Hello World", "chunks must be concatenated in order");
        assert!(chunks > 1, "must emit multiple streaming chunks");
        assert!(is_last_seen, "final chunk must mark is_last");
    } else {
        panic!("Expected Stream response");
    }
}

#[tokio::test]
async fn error_401_maps_to_authentication() {
    let (mock_server, model) = setup_model(false).await;
    let model = model.with_max_retries(0);

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": { "message": "Invalid API key", "type": "invalid_request_error" }
        })))
        .mount(&mock_server)
        .await;

    let msg = user_msg("user", "Hi!").unwrap();
    let err = unwrap_err(model.call(&[msg], None, None).await);
    assert_eq!(err.kind(), Some(ModelErrorKind::Authentication));
}

#[tokio::test]
async fn error_429_maps_to_rate_limit() {
    let (mock_server, model) = setup_model(false).await;
    let model = model.with_max_retries(0);

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
            "error": { "message": "Rate limit exceeded", "type": "rate_limit_error" }
        })))
        .mount(&mock_server)
        .await;

    let msg = user_msg("user", "Hi!").unwrap();
    let err = unwrap_err(model.call(&[msg], None, None).await);
    // RateLimit 是 retryable → 重试耗尽后 RetryExhausted，last_error 为 RateLimit。
    match err {
        ModelError::RetryExhausted { last_error, .. } => {
            assert_eq!(last_error.kind(), Some(ModelErrorKind::RateLimit));
        }
        other => panic!("expected RetryExhausted, got {other}"),
    }
}

#[tokio::test]
async fn error_500_maps_to_internal_server() {
    let (mock_server, model) = setup_model(false).await;
    let model = model.with_max_retries(0);

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "error": { "message": "Internal server error", "type": "server_error" }
        })))
        .mount(&mock_server)
        .await;

    let msg = user_msg("user", "Hi!").unwrap();
    let err = unwrap_err(model.call(&[msg], None, None).await);
    match err {
        ModelError::RetryExhausted { last_error, .. } => {
            assert_eq!(last_error.kind(), Some(ModelErrorKind::InternalServer));
        }
        other => panic!("expected RetryExhausted, got {other}"),
    }
}
