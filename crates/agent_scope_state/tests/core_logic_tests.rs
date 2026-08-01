use agent_scope_state::{PermissionRule, SessionError, SessionImpl, SessionStatus};

#[test]
fn permission_rule_preserves_arbitrary_flattened_fields() {
    let rule: PermissionRule = serde_json::from_str(
        r#"{"kind":"allow","tool":"search","max_calls":3,"nested":{"scope":"read"}}"#,
    )
    .unwrap();

    assert_eq!(rule.extras["kind"], serde_json::json!("allow"));
    assert_eq!(rule.extras["tool"], serde_json::json!("search"));
    assert_eq!(rule.extras["max_calls"], serde_json::json!(3));
    assert_eq!(rule.extras["nested"]["scope"], serde_json::json!("read"));

    let json = serde_json::to_value(&rule).unwrap();
    assert_eq!(json["kind"], serde_json::json!("allow"));
    assert_eq!(json["nested"]["scope"], serde_json::json!("read"));
}

#[test]
fn session_error_display_messages_are_stable() {
    let cases = [
        (
            SessionError::Closed {
                session_id: "s1".into(),
            },
            "Session 's1' is closed",
        ),
        (
            SessionError::AlreadyExists {
                session_id: "s1".into(),
            },
            "Session 's1' already exists",
        ),
        (
            SessionError::NotFound {
                session_id: "s1".into(),
            },
            "Session 's1' not found",
        ),
        (
            SessionError::InvalidTrimConfig {
                reason: "too small".into(),
            },
            "Invalid trim configuration: too small",
        ),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

#[tokio::test]
async fn session_child_token_is_cancelled_on_close() {
    use agent_scope_state::Session;

    let mut session = SessionImpl::with_session_id("cancel-me".into());
    let child = session.cancel_token();

    assert_eq!(session.status(), SessionStatus::Active);
    assert!(!child.is_cancelled());

    session.close().await.unwrap();
    session.close().await.unwrap();

    assert_eq!(session.status(), SessionStatus::Closed);
    assert!(child.is_cancelled());
}
