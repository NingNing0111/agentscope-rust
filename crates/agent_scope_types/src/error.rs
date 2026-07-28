use serde::{Deserialize, Serialize};

/// Classification of a fatal error that terminated a reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    /// 401 — credential missing or wrong.
    Authentication,
    /// 403 — authenticated but not allowed.
    Permission,
    /// 429 — rate/quota exceeded.
    RateLimit,
    /// 400 / 422 — malformed request.
    InvalidRequest,
    /// 5xx — an upstream service failed.
    Upstream,
    /// Network error / timeout — no HTTP status available.
    Connection,
    /// Framework bug or otherwise unexpected exception.
    Internal,
    /// Fallback when no better classification is possible.
    Unknown,
}

/// Structured, UI-facing description of a fatal reply error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Stable classification key; the frontend localizes off it.
    #[serde(default = "default_error_type", rename = "type")]
    pub error_type: ErrorType,
    /// Short, sanitized, human-readable description.
    pub message: String,
}

fn default_error_type() -> ErrorType {
    ErrorType::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ErrorType::Authentication).unwrap(),
            r#""authentication""#
        );
        assert_eq!(
            serde_json::to_string(&ErrorType::Permission).unwrap(),
            r#""permission""#
        );
        assert_eq!(
            serde_json::to_string(&ErrorType::RateLimit).unwrap(),
            r#""rate_limit""#
        );
        assert_eq!(
            serde_json::to_string(&ErrorType::InvalidRequest).unwrap(),
            r#""invalid_request""#
        );
        assert_eq!(
            serde_json::to_string(&ErrorType::Upstream).unwrap(),
            r#""upstream""#
        );
        assert_eq!(
            serde_json::to_string(&ErrorType::Connection).unwrap(),
            r#""connection""#
        );
        assert_eq!(
            serde_json::to_string(&ErrorType::Internal).unwrap(),
            r#""internal""#
        );
        assert_eq!(
            serde_json::to_string(&ErrorType::Unknown).unwrap(),
            r#""unknown""#
        );
    }

    #[test]
    fn test_error_info_default_type() {
        let info = ErrorInfo {
            error_type: ErrorType::Unknown,
            message: "test".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains(r#""type":"unknown""#));
        assert!(json.contains(r#""message":"test""#));
    }

    #[test]
    fn test_error_info_json_roundtrip() {
        let info = ErrorInfo {
            error_type: ErrorType::RateLimit,
            message: "Too many requests".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let restored: ErrorInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.error_type, ErrorType::RateLimit);
        assert_eq!(restored.message, "Too many requests");
    }

    #[test]
    fn test_error_info_deserialization_default() {
        // JSON without type should default to Unknown
        let json = r#"{"message": "something went wrong"}"#;
        let info: ErrorInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.error_type, ErrorType::Unknown);
        assert_eq!(info.message, "something went wrong");
    }
}
