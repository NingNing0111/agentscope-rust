use std::error::Error;

use agent_scope_model::{ModelError, ModelErrorKind};

#[test]
fn display_text_for_representative_model_errors_is_stable() {
    let cases = [
        (
            ModelError::ApiError {
                status: 429,
                message: "slow down".into(),
                provider: "dashscope".into(),
            },
            "[dashscope] API error 429: slow down",
        ),
        (ModelError::Cancelled, "Operation cancelled"),
        (
            ModelError::ValidationError {
                field: "messages".into(),
                message: "empty".into(),
            },
            "Validation error on 'messages': empty",
        ),
        (
            ModelError::StructuredOutputError {
                reason: "schema mismatch".into(),
            },
            "Structured output error: schema mismatch",
        ),
        (
            ModelError::UnsupportedFeature {
                feature: "thinking".into(),
                provider: "mock".into(),
            },
            "[mock] Unsupported feature: thinking",
        ),
        (
            ModelError::ConfigError {
                message: "missing api key".into(),
            },
            "Config error: missing api key",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none(), "unexpected source for {expected}");
    }
}

#[test]
fn source_and_from_for_serialization_errors_are_stable() {
    let source = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    let error = ModelError::from(source);

    assert!(
        error
            .to_string()
            .starts_with("Serialization error in json: ")
    );
    assert!(error.source().is_some());
    assert_eq!(error.kind(), None);
}

#[test]
fn api_error_kind_mapping_is_stable() {
    let cases = [
        (401, Some(ModelErrorKind::Authentication)),
        (403, Some(ModelErrorKind::Authentication)),
        (429, Some(ModelErrorKind::RateLimit)),
        (400, Some(ModelErrorKind::BadRequest)),
        (422, Some(ModelErrorKind::BadRequest)),
        (500, Some(ModelErrorKind::InternalServer)),
        (599, Some(ModelErrorKind::InternalServer)),
        (418, Some(ModelErrorKind::ApiConnection)),
    ];

    for (status, expected) in cases {
        let error = ModelError::ApiError {
            status,
            message: "msg".into(),
            provider: "provider".into(),
        };
        assert_eq!(error.kind(), expected, "status {status}");
    }
}
