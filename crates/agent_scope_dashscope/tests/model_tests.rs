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

/// Structured output on a *streaming-configured* model must still work: the
/// DashScope override forces `stream: false` in the request body instead of
/// letting the trait default error out on `ModelCallResult::Stream`.
#[tokio::test]
async fn structured_output_works_on_streaming_model() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-mock-so",
            "object": "chat.completion",
            "model": "qwen-plus",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "generate_structured_output",
                            "arguments": r#"{"selected_files":["auth.md"]}"#
                        }
                    }]
                },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 5, "completion_tokens": 3 }
        })))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen-plus")
        .with_base_url(mock_server.uri())
        .with_stream(true);

    let msg = user_msg("user", "which memory is relevant?").unwrap();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "selected_files": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["selected_files"]
    });

    let result = model.generate_structured_output(&[msg], &schema).await;
    assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
    let response = result.unwrap();
    assert_eq!(response.content["selected_files"][0], "auth.md");

    // Even though the model is configured streaming, the structured-output
    // request must have forced `stream: false` and dropped the stale
    // `stream_options` (DashScope requires the two to be set together).
    let requests = mock_server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(
        requests
            .last()
            .expect("expected at least one request")
            .body
            .as_ref(),
    )
    .unwrap();
    assert_eq!(body["stream"], false, "stream must be forced off");
    assert!(
        body.get("stream_options").is_none(),
        "stream_options must be dropped when stream is false: {body}"
    );
    // Thinking mode (explicit or server-default, e.g. qwen3) rejects
    // tool_choice="required"/object, so structured output must always send
    // "auto" regardless of the model name.
    assert_eq!(
        body["tool_choice"], "auto",
        "structured output must use tool_choice=\"auto\": {body}"
    );
}

/// A malformed tool-call argument from the model must fall back to JSON repair
/// and still produce a structured response (mirrors the trait default).
#[tokio::test]
async fn structured_output_repairs_malformed_json() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-mock-so2",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call-2",
                        "type": "function",
                        "function": {
                            "name": "generate_structured_output",
                            "arguments": r#"{"selected_files":["auth.md"]"#
                        }
                    }]
                },
                "finish_reason": "stop"
            }]
        })))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen-plus")
        .with_base_url(mock_server.uri())
        .with_stream(true);

    let msg = user_msg("user", "pick a memory").unwrap();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "selected_files": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["selected_files"]
    });

    let result = model.generate_structured_output(&[msg], &schema).await;
    assert!(
        result.is_ok(),
        "Expected repair to succeed, got {:?}",
        result.err()
    );
    let response = result.unwrap();
    assert_eq!(response.content["selected_files"][0], "auth.md");
}

/// A qwen3-series model with `enable_thinking` left at its default (`false`)
/// reproduces the server-default thinking case: DashScope rejects
/// `tool_choice="required"`/object in thinking mode, so structured output must
/// send `"auto"` even when the local `enable_thinking` flag is off.
#[tokio::test]
async fn structured_output_uses_auto_tool_choice_when_thinking_off() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-mock-so3",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call-3",
                        "type": "function",
                        "function": {
                            "name": "generate_structured_output",
                            "arguments": r#"{"selected_files":["b.md"]}"#
                        }
                    }]
                },
                "finish_reason": "stop"
            }]
        })))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen3")
        .with_base_url(mock_server.uri())
        .with_stream(true);

    let msg = user_msg("user", "which memory is relevant?").unwrap();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "selected_files": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["selected_files"]
    });

    let result = model.generate_structured_output(&[msg], &schema).await;
    assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
    let response = result.unwrap();
    assert_eq!(response.content["selected_files"][0], "b.md");

    // The exact bug scenario: model `qwen3` with default enable_thinking=false
    // (server-default thinking) must still send tool_choice="auto".
    let requests = mock_server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(
        requests
            .last()
            .expect("expected at least one request")
            .body
            .as_ref(),
    )
    .unwrap();
    assert_eq!(
        body["tool_choice"], "auto",
        "qwen3 with enable_thinking off must still use tool_choice=\"auto\": {body}"
    );
    assert!(
        body.get("enable_thinking").is_none(),
        "enable_thinking must not be sent when unset: {body}"
    );
}

/// In `auto` mode the model may respond with plain-text JSON instead of a tool
/// call; `generate_structured_output` must fall back to parsing the text.
#[tokio::test]
async fn structured_output_parses_plain_text_json() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-mock-so4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": r#"{"selected_files":["c.md"]}"#
                },
                "finish_reason": "stop"
            }]
        })))
        .mount(&mock_server)
        .await;

    let model = DashScopeChatModel::new("test-key", "qwen3")
        .with_base_url(mock_server.uri())
        .with_stream(true);

    let msg = user_msg("user", "which memory is relevant?").unwrap();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "selected_files": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["selected_files"]
    });

    let result = model.generate_structured_output(&[msg], &schema).await;
    assert!(
        result.is_ok(),
        "Expected text fallback to succeed, got {:?}",
        result.err()
    );
    let response = result.unwrap();
    assert_eq!(response.content["selected_files"][0], "c.md");
}
