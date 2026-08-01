use agent_scope_types::{ErrorInfo, ErrorType, ReplyFinishedReason};

#[test]
fn error_info_defaults_unknown_type_from_minimal_json() {
    let info: ErrorInfo = serde_json::from_str(r#"{"message":"boom"}"#).unwrap();

    assert_eq!(info.error_type, ErrorType::Unknown);
    assert_eq!(info.message, "boom");
}

#[test]
fn error_type_rejects_unknown_wire_value() {
    let err = serde_json::from_str::<ErrorType>(r#""not_a_real_type""#).unwrap_err();

    assert!(err.to_string().contains("unknown variant"));
}

#[test]
fn reply_finished_reason_uses_stable_snake_case_values() {
    let cases = [
        (ReplyFinishedReason::Completed, "completed"),
        (ReplyFinishedReason::Interrupted, "interrupted"),
        (ReplyFinishedReason::ExceedMaxIters, "exceed_max_iters"),
        (ReplyFinishedReason::Error, "error"),
    ];

    for (reason, wire) in cases {
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, format!(r#""{wire}""#));
        assert_eq!(
            serde_json::from_str::<ReplyFinishedReason>(&json).unwrap(),
            reason
        );
    }
}
