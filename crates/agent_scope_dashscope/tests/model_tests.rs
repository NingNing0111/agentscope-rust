//! Integration tests for DashScopeChatModel using wiremock HTTP mocking.

use agent_scope_dashscope::DashScopeChatModel;
use agent_scope_message::factory::user_msg;
use agent_scope_model::ChatModel;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_non_streaming_call() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-mock-001",
            "object": "chat.completion",
            "model": "qwen-plus",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! I am Qwen, how can I help?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 8,
                "total_tokens": 20
            }
        })))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen-plus")
        .with_base_url(mock_server.uri())
        .with_stream(false);

    let msg = user_msg("user", "Hi!").unwrap();
    let result = model.call(&[msg], None, None).await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
    if let Ok(agent_scope_model::model_trait::ModelCallResult::Complete(resp)) = result {
        assert!(resp.is_last);
        assert_eq!(resp.id, "chatcmpl-mock-001");
        let text = resp.get_text_content("");
        assert!(text.contains("Hello"), "Expected greeting, got: {text}");
        assert_eq!(resp.usage.as_ref().map(|u| u.input_tokens), Some(12));
    } else {
        panic!("Expected Complete response");
    }
}

#[tokio::test]
async fn test_streaming_call_parses_chunks() {
    let mock_server = MockServer::start().await;

    let sse_body = concat!(
        "data: {\"id\":\"chatcmpl-mock-002\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"chatcmpl-mock-002\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" World\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"chatcmpl-mock-002\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\"},\"finish_reason\":\"stop\"}]}\n\n",
        "data: {\"id\":\"chatcmpl-mock-002\",\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}\n\n",
        "data: [DONE]\n\n"
    );

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen-plus")
        .with_base_url(mock_server.uri())
        .with_stream(true);

    let msg = user_msg("user", "Hi!").unwrap();
    let result = model.call(&[msg], None, None).await;

    assert!(result.is_ok());
    if let Ok(agent_scope_model::model_trait::ModelCallResult::Stream(mut stream)) = result {
        use futures::StreamExt;
        let mut messages: Vec<String> = Vec::new();
        while let Some(Ok(chunk)) = stream.next().await {
            messages.push(chunk.get_text_content(""));
        }
        let combined: String = messages.into_iter().collect::<Vec<_>>().join("");
        assert_eq!(combined, "Hello World");
    } else {
        panic!("Expected Stream response");
    }
}

#[tokio::test]
async fn test_empty_choices_usage_chunk_no_panic() {
    let mock_server = MockServer::start().await;

    // Simulate an SSE response where the final chunk has empty choices + usage
    let sse_body = concat!(
        "data: {\"id\":\"chatcmpl-mock-003\",\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":1,\"total_tokens\":6}}\n\n",
        "data: [DONE]\n\n"
    );

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen-plus")
        .with_base_url(mock_server.uri())
        .with_stream(true);

    let msg = user_msg("user", "Hi!").unwrap();
    let result = model.call(&[msg], None, None).await;

    assert!(result.is_ok());
    if let Ok(agent_scope_model::model_trait::ModelCallResult::Stream(mut stream)) = result {
        use futures::StreamExt;
        let mut usage_found = false;
        while let Some(Ok(chunk)) = stream.next().await {
            if chunk.usage.is_some() {
                usage_found = true;
            }
        }
        assert!(
            usage_found,
            "Expected usage to be found in empty-choices chunk"
        );
    }
}

#[tokio::test]
async fn test_error_response_401_authentication() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": {
                "code": "InvalidApiKey",
                "message": "Invalid API-key provided.",
                "type": "invalid_request_error"
            }
        })))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("bad-key", "qwen-plus")
        .with_base_url(mock_server.uri())
        .with_stream(false);

    let msg = user_msg("user", "Hi!").unwrap();
    let result = model.call(&[msg], None, None).await;

    assert!(result.is_err());
    if let Err(err) = result {
        let error_str = err.to_string();
        assert!(
            error_str.contains("401") || error_str.contains("InvalidApiKey"),
            "Expected 401 or InvalidApiKey, got: {error_str}"
        );
    }
}

#[tokio::test]
async fn test_error_response_429_rate_limit() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
            "error": {
                "code": "Throttling.RateQuota",
                "message": "Rate limit exceeded. Please try again later.",
                "type": "rate_limit_error"
            }
        })))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen-plus")
        .with_base_url(mock_server.uri())
        .with_stream(false);

    let msg = user_msg("user", "Hi!").unwrap();
    let result = model.call(&[msg], None, None).await;

    assert!(result.is_err());
    if let Err(err) = result {
        let error_str = err.to_string();
        assert!(
            error_str.contains("429") || error_str.contains("Throttling"),
            "Expected 429 or Throttling, got: {error_str}"
        );
    }
}

#[tokio::test]
async fn test_error_response_500_internal_server() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen-plus")
        .with_base_url(mock_server.uri())
        .with_stream(false);

    let msg = user_msg("user", "Hi!").unwrap();
    let result = model.call(&[msg], None, None).await;

    assert!(result.is_err());
    if let Err(err) = result {
        assert!(err.to_string().contains("500"), "Expected 500, got: {err}");
    }
}

#[tokio::test]
async fn test_flat_error_format() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "code": "InvalidParameter",
            "message": "Required body invalid, please check the request body format.",
            "request_id": "abc-123"
        })))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen-plus")
        .with_base_url(mock_server.uri())
        .with_stream(false);

    let msg = user_msg("user", "Hi!").unwrap();
    let result = model.call(&[msg], None, None).await;

    assert!(result.is_err());
    if let Err(err) = result {
        assert!(
            err.to_string().contains("InvalidParameter"),
            "Expected InvalidParameter, got: {err}"
        );
    }
}
