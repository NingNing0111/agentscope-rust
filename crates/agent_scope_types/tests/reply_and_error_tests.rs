//! Integration tests for ReplyFinishedReason and ErrorType/ErrorInfo
//! serialization round-trip and edge cases.

use agent_scope_types::{ErrorInfo, ErrorType, ReplyFinishedReason};

// ── T100: ReplyFinishedReason serialization ──────────────────────────

#[test]
fn test_reply_finished_reason_all_variants_snake_case() {
    let cases = vec![
        (ReplyFinishedReason::Completed, "completed"),
        (ReplyFinishedReason::Interrupted, "interrupted"),
        (ReplyFinishedReason::ExceedMaxIters, "exceed_max_iters"),
        (ReplyFinishedReason::Error, "error"),
    ];
    assert_eq!(cases.len(), 4);
    for (variant, expected) in cases {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!(r#""{}""#, expected));
    }
}

#[test]
fn test_reply_finished_reason_deserialization() {
    let source = r#""completed""#;
    let r: ReplyFinishedReason = serde_json::from_str(source).unwrap();
    assert_eq!(r, ReplyFinishedReason::Completed);

    let source = r#""exceed_max_iters""#;
    let r: ReplyFinishedReason = serde_json::from_str(source).unwrap();
    assert_eq!(r, ReplyFinishedReason::ExceedMaxIters);
}

// ── T100: ErrorType and ErrorInfo serialization ──────────────────────

#[test]
fn test_error_type_all_8_variants_snake_case() {
    let cases = vec![
        (ErrorType::Authentication, "authentication"),
        (ErrorType::Permission, "permission"),
        (ErrorType::RateLimit, "rate_limit"),
        (ErrorType::InvalidRequest, "invalid_request"),
        (ErrorType::Upstream, "upstream"),
        (ErrorType::Connection, "connection"),
        (ErrorType::Internal, "internal"),
        (ErrorType::Unknown, "unknown"),
    ];
    assert_eq!(cases.len(), 8);
    for (variant, expected) in cases {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!(r#""{}""#, expected));
    }
}

#[test]
fn test_error_info_json_structure_matches_spec() {
    let info = ErrorInfo {
        error_type: ErrorType::RateLimit,
        message: "Too many requests".to_string(),
    };
    let json = serde_json::to_string(&info).unwrap();
    // Per spec: {"type": "rate_limit", "message": "Too many requests"}
    assert!(json.contains(r#""type":"rate_limit""#));
    assert!(json.contains(r#""message":"Too many requests""#));
}

#[test]
fn test_error_info_default_type_is_unknown() {
    let json = r#"{"message": "something broke"}"#;
    let info: ErrorInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.error_type, ErrorType::Unknown);
    assert_eq!(info.message, "something broke");
}

#[test]
fn test_error_info_roundtrip_all_variants() {
    for error_type in &[
        ErrorType::Authentication,
        ErrorType::Permission,
        ErrorType::RateLimit,
        ErrorType::InvalidRequest,
        ErrorType::Upstream,
        ErrorType::Connection,
        ErrorType::Internal,
        ErrorType::Unknown,
    ] {
        let info = ErrorInfo {
            error_type: error_type.clone(),
            message: format!("error: {:?}", error_type),
        };
        let json = serde_json::to_string(&info).unwrap();
        let restored: ErrorInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.error_type, *error_type);
        assert_eq!(restored.message, info.message);
    }
}

#[test]
fn test_error_info_with_extra_fields_ignored() {
    let json = r#"{"type":"internal","message":"fail","extra_field":123}"#;
    let info: ErrorInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.error_type, ErrorType::Internal);
    assert_eq!(info.message, "fail");
}

// ── Embedding type ───────────────────────────────────────────────────

#[test]
fn test_embedding_type_alias() {
    let emb: agent_scope_types::Embedding = vec![0.1, 0.2, 0.3];
    assert_eq!(emb.len(), 3);
    assert!((emb[1] - 0.2).abs() < f64::EPSILON);
}

// ── JsonValue type ───────────────────────────────────────────────────

#[test]
fn test_json_value_type_alias() {
    use agent_scope_types::JsonValue;
    let val: JsonValue = serde_json::json!({"key": "value"});
    assert_eq!(val["key"], "value");
}
