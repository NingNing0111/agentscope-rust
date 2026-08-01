use std::time::Duration;

use agent_scope_sandbox::{
    ExecutionRequest, LocalSandboxConfig, LocalSandboxSession, SandboxPolicy, SandboxSession,
};

#[tokio::test]
async fn audit_history_order_and_output_refs() {
    let policy = SandboxPolicy {
        max_output_bytes: 2,
        ..Default::default()
    };
    let mut session = LocalSandboxSession::new(LocalSandboxConfig {
        policy,
        ..Default::default()
    })
    .unwrap();
    session.initialize().await.unwrap();
    session
        .execute(ExecutionRequest::new(["printf", "abcd"]))
        .await
        .unwrap();
    session
        .execute(ExecutionRequest::new(["sh", "-c", "exit 3"]))
        .await
        .unwrap();
    let history = session.history().await.unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].sequence, 1);
    assert_eq!(history[1].sequence, 2);
    assert!(history[0].stdout_ref.is_some());
    assert_eq!(
        history[1].failure_category.as_deref(),
        Some("non_zero_exit")
    );
}

#[tokio::test]
async fn audit_redacts_sensitive_env_values() {
    let mut session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
    session.initialize().await.unwrap();
    let mut req = ExecutionRequest::new(["printf", "ok"]);
    req.env
        .insert("API_KEY".into(), "super-secret-value".into());
    session.execute(req).await.unwrap();
    let summary = &session.history().await.unwrap()[0].command_summary;
    assert!(summary.contains("API_KEY"));
    assert!(!summary.contains("super-secret-value"));
}

#[tokio::test]
async fn audit_failure_category_timeout() {
    let policy = SandboxPolicy {
        default_timeout: Duration::from_millis(20),
        max_timeout: Duration::from_millis(50),
        ..Default::default()
    };
    let mut session = LocalSandboxSession::new(LocalSandboxConfig {
        policy,
        ..Default::default()
    })
    .unwrap();
    session.initialize().await.unwrap();
    session
        .execute(ExecutionRequest::new(["sh", "-c", "sleep 1"]))
        .await
        .unwrap();
    assert_eq!(
        session.history().await.unwrap()[0]
            .failure_category
            .as_deref(),
        Some("timeout")
    );
}
