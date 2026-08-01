use std::time::Duration;

use sha2::{Digest, Sha256};

use agent_scope_sandbox::{
    CapabilityReport, ExecutionRequest, ExecutionStatus, LocalSandboxConfig, LocalSandboxSession,
    NetworkPolicy, ResourceLimitHit, SandboxError, SandboxPolicy, SandboxSession,
};

#[tokio::test]
async fn sandbox_output_limit_writes_full_ref() {
    let policy = SandboxPolicy {
        max_output_bytes: 4,
        ..Default::default()
    };
    let mut session = LocalSandboxSession::new(LocalSandboxConfig {
        policy,
        ..Default::default()
    })
    .unwrap();
    session.initialize().await.unwrap();
    let result = session
        .execute(ExecutionRequest::new(["printf", "abcdefgh"]))
        .await
        .unwrap();
    assert_eq!(result.stdout.inline, b"abcd");
    assert!(result.stdout.truncated);
    let output_ref = result.stdout.full_ref.as_ref().unwrap();
    assert!(output_ref.bytes >= 8);
    let full = tokio::fs::read(&output_ref.path).await.unwrap();
    let expected_sha = format!("{:x}", Sha256::digest(&full));
    assert_eq!(output_ref.sha256, expected_sha);
    assert!(
        result
            .resource_hits
            .contains(&ResourceLimitHit::OutputTruncated)
    );
}

#[tokio::test]
async fn sandbox_execution_status_timeout_policy() {
    let policy = SandboxPolicy {
        default_timeout: Duration::from_millis(30),
        max_timeout: Duration::from_millis(100),
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

#[tokio::test]
async fn sandbox_capability_report_lists_unsupported() {
    let report = CapabilityReport::local_process();
    assert!(
        report
            .unsupported
            .iter()
            .any(|u| u.capability == "network_policy")
    );
    assert!(report.supported.len() >= 5);
}

#[tokio::test]
async fn sandbox_default_policy_is_explicitly_unrestricted_network() {
    assert_eq!(
        SandboxPolicy::default().network,
        NetworkPolicy::Unrestricted
    );
}

#[tokio::test]
async fn sandbox_policy_disabled_network_is_unsupported() {
    let policy = SandboxPolicy {
        network: NetworkPolicy::Disabled,
        ..Default::default()
    };
    let err = LocalSandboxSession::new(LocalSandboxConfig {
        policy,
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::UnsupportedFeature { .. }));
}

#[tokio::test]
async fn sandbox_policy_unsupported_resource_requests_fail() {
    let policy = SandboxPolicy {
        memory_limit_bytes: Some(1024),
        ..Default::default()
    };
    let err = LocalSandboxSession::new(LocalSandboxConfig {
        policy,
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::UnsupportedFeature { .. }));

    let policy = SandboxPolicy {
        network: NetworkPolicy::LoopbackOnly,
        ..Default::default()
    };
    let err = LocalSandboxSession::new(LocalSandboxConfig {
        policy,
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::UnsupportedFeature { .. }));
}
