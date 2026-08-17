//! RigError 映射 — rig `CompletionError` → agent_scope `ModelError` 全分类。
//!
//! 契约见 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §4。
//! 分类依据 `ModelError.kind()`（[`agent_scope_model::model_error::ModelErrorKind`]）：
//! 401/403→Authentication、429→RateLimit、5xx→InternalServer、4xx→BadRequest、
//! 连接/超时→ApiError{status:0}（kind() 归 ApiConnection，retryable）。
//!
//! **安全约束**（宪法第九/十四条）：错误消息不泄露 API key——provider 原始 body
//! 只提取 `error.message` 字段，fallback 截断至 512 字符，绝不原样透传。

use rig::completion::CompletionError;

use agent_scope_model::FormatError;
use agent_scope_model::model_error::ModelError;

/// 把 rig `CompletionError` 映射为 agent_scope `ModelError`。
///
/// `provider` 用于错误消息标识（`openai`/`anthropic`/`deepseek`）。
///
/// # 映射表
///
/// | rig 错误 | `ModelError` |
/// |----------|--------------|
/// | `HttpError(InvalidStatusCode[WithMessage])` | `ApiError{status}`（按 status 分类 kind） |
/// | `HttpError(Instance/Protocol/…)`（连接/超时） | `ApiError{status:0}`（kind()=ApiConnection，retryable） |
/// | `ProviderResponse{status}` | 按 status 分类 |
/// | `ProviderResponse{status:None}` | `ApiError{status:0}`（提取 body 消息） |
/// | `ProviderError(String)` | `ApiError{status:0}`（诊断信息 sanitize） |
/// | `ResponseError(String)` | `FormatError{context:"rig:response"}` |
/// | `JsonError` | `SerializationError{context:"rig:json"}` |
/// | `UrlError` | `ConfigError` |
/// | `RequestError` | `FormatError{context:"rig:request"}` |
///
/// rig 0.41.0 `CompletionError` 带 `#[non_exhaustive]`，必须保留 `_ =>` 兜底。
pub fn map_completion_error(err: &CompletionError, provider: impl AsRef<str>) -> ModelError {
    let provider = provider.as_ref().to_string();

    match err {
        // ── HTTP 状态错误 ──────────────────────────────────────────────
        CompletionError::HttpError(e) => match e {
            rig::core::http_client::Error::InvalidStatusCode(status) => {
                classify_status(status.as_u16(), "", &provider)
            }
            rig::core::http_client::Error::InvalidStatusCodeWithMessage(status, body) => {
                classify_status(status.as_u16(), body, &provider)
            }
            // 连接 / 超时 / 协议错误。rig 把非成功 HTTP 响应建模为
            // InvalidStatusCode[WithMessage]；其余（Instance=reqwest 错误、
            // Protocol、StreamEnded 等）视为传输层失败。ModelError 框架无
            // 独立 ApiTimeout 变体（kind() 只看 status，status:0 → ApiConnection，
            // 在 retryable_errors 内），连接与超时统一归 ApiConnection 类。
            other => {
                let msg = other.to_string();
                if is_timeout_like(&msg) {
                    ModelError::ApiError {
                        status: 0,
                        message: "request timed out".to_string(),
                        provider,
                    }
                } else {
                    ModelError::ApiError {
                        status: 0,
                        message: sanitize_message(&msg),
                        provider,
                    }
                }
            }
        },
        // ── Provider 原始响应（status + body 均保留）─────────────────
        CompletionError::ProviderResponse(r) => match r.status {
            Some(status) => classify_status(status.as_u16(), &r.body, &provider),
            None => ModelError::ApiError {
                status: 0,
                message: extract_error_message(&r.body),
                provider,
            },
        },
        // ── Provider 诊断（无 status）────────────────────────────────
        CompletionError::ProviderError(s) => ModelError::ApiError {
            status: 0,
            message: sanitize_message(s),
            provider,
        },
        // ── 响应解析失败 ─────────────────────────────────────────────
        CompletionError::ResponseError(s) => ModelError::FormatError {
            context: "rig:response".to_string(),
            source: FormatError::InvalidMessage(sanitize_message(s)),
        },
        CompletionError::JsonError(e) => ModelError::SerializationError {
            context: "rig:json".to_string(),
            // serde_json::Error 不实现 Clone，无法从 `&Error` 复制；以同消息
            // 经 `Error::io` 重建（该构造器为 #[doc(hidden)] 但公开可调）。
            source: serde_json::Error::io(std::io::Error::other(e.to_string())),
        },
        // ── URL / 请求构建失败 ───────────────────────────────────────
        CompletionError::UrlError(e) => ModelError::ConfigError {
            message: format!("invalid URL: {e}"),
        },
        CompletionError::RequestError(e) => ModelError::FormatError {
            context: "rig:request".to_string(),
            source: FormatError::InvalidMessage(e.to_string()),
        },
        // ── 兜底（#[non_exhaustive]）────────────────────────────────
        _ => ModelError::ApiError {
            status: 0,
            message: sanitize_message(&err.to_string()),
            provider,
        },
    }
}

/// 按 HTTP status 分类为 `ApiError`。
///
/// `kind()` 自动完成分类：401/403→Auth、429→RateLimit、5xx→InternalServer、
/// 4xx（含 400/422）→BadRequest。
fn classify_status(status: u16, body: &str, provider: &str) -> ModelError {
    ModelError::ApiError {
        status,
        message: extract_error_message(body),
        provider: provider.to_string(),
    }
}

/// 从 provider 错误 body 提取可读消息（不泄露 key）。
///
/// 支持 OpenAI 兼容的 `{"error":{"message":…}}` 与扁平 `{"message":…}`；
/// 解析失败或字段缺失时截断 body 至 512 字符。
fn extract_error_message(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            // error.message 本身可能含完整 key（如 OpenAI
            // "Incorrect API key provided: sk-…"），仍需打码。
            return redact_api_keys(msg);
        }
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return redact_api_keys(msg);
        }
        // 防止 key 泄露：不返回整个 body 的 JSON 原样，仅提取已知字段。
        if v.get("error").is_some() || v.get("message").is_some() {
            return "provider returned an error".to_string();
        }
    }
    sanitize_message(body)
}

/// 截断并清理错误消息，防止泄露敏感信息（API key 等）。
fn sanitize_message(msg: &str) -> String {
    let trimmed = msg.trim();
    if trimmed.is_empty() {
        "unknown error".to_string()
    } else {
        redact_api_keys(trimmed).chars().take(512).collect()
    }
}

/// 把形如 `sk-` 开头的 API key token 打码为 `sk-***`。
///
/// OpenAI/DashScope/Anthropic key 常以 `sk-` 前缀（如 `sk-abc123`、
/// `sk-ant-api03-…`）；用文本扫描避免引入正则依赖。token 字符集取
/// `[A-Za-z0-9_-]`，超出即视为 key 结束。误伤 `sk-` 子串（如 "task-"）的
/// 代价远低于泄露 key，故保守打码。
fn redact_api_keys(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut rest = msg;
    loop {
        match rest.find("sk-") {
            Some(pos) => {
                out.push_str(&rest[..pos]);
                let after = &rest[pos + 3..];
                let token_len = after
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                    .unwrap_or(after.len());
                out.push_str("sk-***");
                rest = &after[token_len..];
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }
    out
}

/// 启发式判断传输错误消息是否为超时。
///
/// rig 把 reqwest 超时错误包装在 `http_client::Error::Instance` 中；其 Display
/// 文本通常含 "timed out"/"timeout"。用文本启发式替代 downcast（避免为
/// reqwest 类型加显式依赖）。
fn is_timeout_like(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("timed out") || lower.contains("timeout")
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_model::model_error::ModelErrorKind;

    #[test]
    fn http_401_maps_to_authentication() {
        let err = CompletionError::HttpError(
            rig::core::http_client::Error::InvalidStatusCodeWithMessage(
                http::StatusCode::UNAUTHORIZED,
                r#"{"error":{"message":"Invalid API key"}}"#.to_string(),
            ),
        );
        let mapped = map_completion_error(&err, "openai");
        assert_eq!(mapped.kind(), Some(ModelErrorKind::Authentication));
        assert!(!mapped.to_string().contains("sk-"), "must not leak key");
    }

    #[test]
    fn http_429_maps_to_rate_limit() {
        let err = CompletionError::HttpError(
            rig::core::http_client::Error::InvalidStatusCodeWithMessage(
                http::StatusCode::TOO_MANY_REQUESTS,
                r#"{"error":{"message":"rate limited"}}"#.to_string(),
            ),
        );
        assert_eq!(
            map_completion_error(&err, "openai").kind(),
            Some(ModelErrorKind::RateLimit)
        );
    }

    #[test]
    fn http_500_maps_to_internal_server() {
        let err = CompletionError::HttpError(rig::core::http_client::Error::InvalidStatusCode(
            http::StatusCode::INTERNAL_SERVER_ERROR,
        ));
        assert_eq!(
            map_completion_error(&err, "openai").kind(),
            Some(ModelErrorKind::InternalServer)
        );
    }

    #[test]
    fn http_400_maps_to_bad_request() {
        let err = CompletionError::HttpError(
            rig::core::http_client::Error::InvalidStatusCodeWithMessage(
                http::StatusCode::BAD_REQUEST,
                r#"{"error":{"message":"bad schema"}}"#.to_string(),
            ),
        );
        assert_eq!(
            map_completion_error(&err, "openai").kind(),
            Some(ModelErrorKind::BadRequest)
        );
    }

    // NOTE: `CompletionError::ProviderResponse` 分支无法直接单测——其载荷类型
    // `ProviderResponseError` 位于 rig 的 `pub(crate) mod provider_response`，
    // 外部 crate 不可命名构造。该分支的分类逻辑复用 `classify_status`，
    // 已被 `InvalidStatusCode` 各状态测试覆盖。

    #[test]
    fn provider_error_string_maps_to_api_error() {
        let err = CompletionError::ProviderError("model not found".to_string());
        let mapped = map_completion_error(&err, "openai");
        assert!(matches!(mapped, ModelError::ApiError { status: 0, .. }));
    }

    #[test]
    fn response_error_maps_to_format_error() {
        let err = CompletionError::ResponseError("invalid json in response".to_string());
        let mapped = map_completion_error(&err, "openai");
        assert!(matches!(mapped, ModelError::FormatError { .. }));
        assert_eq!(mapped.kind(), None);
    }

    #[test]
    fn json_error_maps_to_serialization_error() {
        let err =
            CompletionError::JsonError(serde_json::from_str::<serde_json::Value>("{").unwrap_err());
        let mapped = map_completion_error(&err, "openai");
        assert!(matches!(mapped, ModelError::SerializationError { .. }));
    }

    #[test]
    fn url_error_maps_to_config_error() {
        let err = CompletionError::UrlError(url::ParseError::Overflow);
        let mapped = map_completion_error(&err, "openai");
        assert!(matches!(mapped, ModelError::ConfigError { .. }));
    }

    #[test]
    fn connection_error_maps_to_api_connection() {
        let err = CompletionError::HttpError(rig::core::http_client::Error::Instance(
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused").into(),
        ));
        let mapped = map_completion_error(&err, "openai");
        assert_eq!(mapped.kind(), Some(ModelErrorKind::ApiConnection));
    }

    #[test]
    fn timeout_error_classified_as_connection_retryable() {
        // reqwest 超时消息含 "timed out"；框架无独立 ApiTimeout 变体，
        // 统一归 status:0（kind()=ApiConnection，retryable）。
        let err = CompletionError::HttpError(rig::core::http_client::Error::Instance(
            std::io::Error::new(std::io::ErrorKind::TimedOut, "operation timed out").into(),
        ));
        let mapped = map_completion_error(&err, "openai");
        assert_eq!(mapped.kind(), Some(ModelErrorKind::ApiConnection));
        assert!(mapped.to_string().contains("timed out"));
    }

    #[test]
    fn key_never_leaks_in_error_message() {
        // body 含 sk- 明文时，只提取 error.message（不含 key）。
        let err = CompletionError::HttpError(
            rig::core::http_client::Error::InvalidStatusCodeWithMessage(
                http::StatusCode::UNAUTHORIZED,
                r#"{"error":{"message":"Incorrect API key provided: sk-abc123xyz"}}"#.to_string(),
            ),
        );
        let mapped = map_completion_error(&err, "openai");
        let text = mapped.to_string();
        assert!(
            !text.contains("sk-abc123xyz"),
            "must not leak full key in message"
        );
    }
}
