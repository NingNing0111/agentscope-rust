//! T012 — 错误映射测试（rig `CompletionError` → `ModelError` 分类 + key 不泄露）。
//!
//! 对照 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §4。
//! `map_completion_error` 内部单测见 `src/error.rs`；本文件覆盖集成层分类面。
//! T029 — 重试语义：429/500（retryable）重试 `max_retries` 次后 `RetryExhausted`，
//! 401（Authentication）不重试直接返回。

use agent_scope_message::factory::user_msg;
use agent_scope_model::ChatModel;
use agent_scope_model::model_error::{ModelError, ModelErrorKind};
use agent_scope_rig::RigChatModel;
use agent_scope_rig::error::map_completion_error;
use rig::completion::CompletionError;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn http_status(status: http::StatusCode, body: &str) -> CompletionError {
    CompletionError::HttpError(rig::core::http_client::Error::InvalidStatusCodeWithMessage(
        status,
        body.to_string(),
    ))
}

#[test]
fn http_401_maps_to_authentication() {
    let err = http_status(
        http::StatusCode::UNAUTHORIZED,
        r#"{"error":{"message":"bad key"}}"#,
    );
    assert_eq!(
        map_completion_error(&err, "openai").kind(),
        Some(ModelErrorKind::Authentication)
    );
}

#[test]
fn http_403_maps_to_authentication() {
    let err = http_status(http::StatusCode::FORBIDDEN, "{}");
    assert_eq!(
        map_completion_error(&err, "openai").kind(),
        Some(ModelErrorKind::Authentication)
    );
}

#[test]
fn http_429_maps_to_rate_limit() {
    let err = http_status(
        http::StatusCode::TOO_MANY_REQUESTS,
        r#"{"error":{"message":"rate limited"}}"#,
    );
    assert_eq!(
        map_completion_error(&err, "openai").kind(),
        Some(ModelErrorKind::RateLimit)
    );
}

#[test]
fn http_500_maps_to_internal_server() {
    let err = http_status(http::StatusCode::INTERNAL_SERVER_ERROR, "boom");
    assert_eq!(
        map_completion_error(&err, "openai").kind(),
        Some(ModelErrorKind::InternalServer)
    );
}

#[test]
fn http_400_maps_to_bad_request() {
    let err = http_status(http::StatusCode::BAD_REQUEST, "bad schema");
    assert_eq!(
        map_completion_error(&err, "openai").kind(),
        Some(ModelErrorKind::BadRequest)
    );
}

// NOTE: `CompletionError::ProviderResponse` 分支无法直接单测——其载荷类型
// `ProviderResponseError` 位于 rig 的 `pub(crate) mod provider_response`，
// 外部 crate 不可命名构造。该分支分类逻辑与 `InvalidStatusCode` 共用
// `classify_status`，已被上列状态测试覆盖。

#[test]
fn connection_error_maps_to_api_connection() {
    let err = CompletionError::HttpError(rig::core::http_client::Error::Instance(
        std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused").into(),
    ));
    assert_eq!(
        map_completion_error(&err, "openai").kind(),
        Some(ModelErrorKind::ApiConnection)
    );
}

#[test]
fn timeout_error_maps_to_connection_retryable() {
    let err = CompletionError::HttpError(rig::core::http_client::Error::Instance(
        std::io::Error::new(std::io::ErrorKind::TimedOut, "request timed out").into(),
    ));
    let mapped = map_completion_error(&err, "openai");
    assert_eq!(mapped.kind(), Some(ModelErrorKind::ApiConnection));
    assert!(mapped.to_string().contains("timed out"));
}

#[test]
fn response_error_maps_to_format_error() {
    let err = CompletionError::ResponseError("invalid response".to_string());
    assert!(matches!(
        map_completion_error(&err, "openai"),
        ModelError::FormatError { .. }
    ));
}

#[test]
fn key_never_leaks_in_error_message() {
    let err = http_status(
        http::StatusCode::UNAUTHORIZED,
        r#"{"error":{"message":"Incorrect API key provided: sk-abc123xyz"}}"#,
    );
    let mapped = map_completion_error(&err, "openai");
    let text = mapped.to_string();
    assert!(
        !text.contains("sk-abc123xyz"),
        "must not leak full key, got: {text}"
    );
}

#[test]
fn raw_body_without_json_fields_is_sanitized() {
    // body 非 JSON 或无可提取字段时，不应原样透传敏感明文。
    let err = http_status(http::StatusCode::UNAUTHORIZED, "authorization sk-aaaa bbbb");
    let mapped = map_completion_error(&err, "openai");
    assert!(!mapped.to_string().contains("sk-aaaa"));
}

// ---------------------------------------------------------------------------
// T029 — 重试语义（mock HTTP 状态 → `ChatModel::call` 重试行为）
// ---------------------------------------------------------------------------

/// 指向 wiremock 的 rig-backed OpenAI 模型（固定重试参数，缩短测试时长）。
/// 非流式：`ChatModel::call` 在 `completion()` 返回时即暴露 HTTP 错误，
/// 重试循环（model_trait `call`）在 `call_api` 返回 Err 时立即迭代。
fn mock_model(uri: &str) -> RigChatModel {
    RigChatModel::openai("test-key", "gpt-4o-mini")
        .unwrap()
        .with_stream(false)
        .with_base_url(uri.to_string())
        .with_max_retries(3)
        .with_retry_delay(0.0)
}

/// 注册固定状态码的 mock 并统计请求次数。
async fn mount_status(server: &MockServer, status: u16) {
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(status)
                .set_body_raw(r#"{"error":{"message":"boom"}}"#, "application/json"),
        )
        .mount(server)
        .await;
}

async fn request_count(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .map(|r| r.len())
        .unwrap_or_default()
}

#[tokio::test]
async fn http_429_retries_then_retry_exhausted() {
    // 429 → RateLimit（retryable）：初始 + 3 次重试共 4 次请求后 RetryExhausted。
    let server = MockServer::start().await;
    mount_status(&server, 429).await;
    let model = mock_model(&server.uri());
    let msgs = vec![user_msg("user", "hello").unwrap()];

    let err = match model.call(&msgs, None, None).await {
        Ok(_) => panic!("must fail on 429"),
        Err(e) => e,
    };
    match err {
        ModelError::RetryExhausted { attempts, .. } => {
            assert_eq!(attempts, 4, "initial + 3 retries");
        }
        other => panic!("expected RetryExhausted, got: {other:?}"),
    }
    assert_eq!(request_count(&server).await, 4, "429 must be retried 3x");
}

#[tokio::test]
async fn http_500_retries_then_retry_exhausted() {
    // 500 → InternalServer（retryable）：同样重试 3 次后 RetryExhausted。
    let server = MockServer::start().await;
    mount_status(&server, 500).await;
    let model = mock_model(&server.uri());
    let msgs = vec![user_msg("user", "hello").unwrap()];

    let err = match model.call(&msgs, None, None).await {
        Ok(_) => panic!("must fail on 500"),
        Err(e) => e,
    };
    match err {
        ModelError::RetryExhausted { attempts, .. } => {
            assert_eq!(attempts, 4, "initial + 3 retries");
        }
        other => panic!("expected RetryExhausted, got: {other:?}"),
    }
    assert_eq!(request_count(&server).await, 4, "500 must be retried 3x");
}

#[tokio::test]
async fn http_401_does_not_retry_returns_authentication() {
    // 401 → Authentication（非 retryable）：单次请求直接返回，不重试。
    let server = MockServer::start().await;
    mount_status(&server, 401).await;
    let model = mock_model(&server.uri());
    let msgs = vec![user_msg("user", "hello").unwrap()];

    let err = match model.call(&msgs, None, None).await {
        Ok(_) => panic!("must fail on 401"),
        Err(e) => e,
    };
    assert_eq!(
        err.kind(),
        Some(ModelErrorKind::Authentication),
        "401 must map to Authentication, got: {err:?}"
    );
    assert_eq!(request_count(&server).await, 1, "401 must not be retried");
}
