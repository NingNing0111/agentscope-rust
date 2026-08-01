use std::time::Duration;

use agent_scope_sandbox::{
    CapabilityReport, ExecutionRequest, ExecutionStatus, LocalSandboxConfig, LocalSandboxSession,
    SandboxPolicy, SandboxSession,
};

#[tokio::test]
async fn sandbox_session_execute_success_and_nonzero() {
    let mut session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
    session.initialize().await.unwrap();

    let result = session
        .execute(ExecutionRequest::new(["printf", "hello"]))
        .await
        .unwrap();
    assert_eq!(result.status, ExecutionStatus::Exited { code: 0 });
    assert_eq!(result.stdout.inline, b"hello");
    assert_eq!(session.history().await.unwrap()[0].sequence, 1);

    let result = session
        .execute(ExecutionRequest::new(["sh", "-c", "exit 7"]))
        .await
        .unwrap();
    assert_eq!(result.status, ExecutionStatus::Exited { code: 7 });
    assert_eq!(result.exit_code, Some(7));
}

#[tokio::test]
async fn sandbox_session_lifecycle_close_rejects_operations() {
    let mut session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
    session.initialize().await.unwrap();
    session.initialize().await.unwrap();
    session.close().await.unwrap();
    session.close().await.unwrap();
    assert!(
        session
            .execute(ExecutionRequest::new(["printf", "x"]))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn sandbox_session_serde_roundtrips() {
    let policy = SandboxPolicy::default();
    let json = serde_json::to_string(&policy).unwrap();
    let decoded: SandboxPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.max_output_bytes, policy.max_output_bytes);

    let report = CapabilityReport::local_process();
    let json = serde_json::to_string(&report).unwrap();
    let decoded: CapabilityReport = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.backend_name, "local-process");
}

#[tokio::test]
async fn sandbox_execution_status_timeout() {
    let policy = SandboxPolicy {
        default_timeout: Duration::from_millis(50),
        max_timeout: Duration::from_secs(1),
        ..Default::default()
    };
    let mut session = LocalSandboxSession::new(LocalSandboxConfig {
        policy,
        ..Default::default()
    })
    .unwrap();
    session.initialize().await.unwrap();
    let result = session
        .execute(ExecutionRequest::new(["sh", "-c", "sleep 1"]))
        .await
        .unwrap();
    assert_eq!(result.status, ExecutionStatus::TimedOut);
}
