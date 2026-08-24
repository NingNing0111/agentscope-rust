#![cfg(feature = "microsandbox")]

use std::path::PathBuf;
use std::time::Duration;

use agent_scope_sandbox::{
    ExecutionRequest, ExecutionStatus, MicrosandboxConfig, MicrosandboxSession, MountAccess,
    MountOwner, NetworkPolicy, SandboxError, SandboxMount, SandboxPolicy, SandboxSession,
};

fn runtime_tests_enabled() -> bool {
    std::env::var("AGENTSCOPE_RUN_MICROSANDBOX_TESTS").as_deref() == Ok("1")
}

fn runtime_config() -> (tempfile::TempDir, MicrosandboxConfig) {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("input.txt"), b"mounted workspace").unwrap();

    let config = MicrosandboxConfig {
        session_id: None,
        image: std::env::var("AGENTSCOPE_MICROSANDBOX_IMAGE").unwrap_or_else(|_| "python".into()),
        workdir: "/workspace".into(),
        mounts: vec![SandboxMount {
            mount_id: "workspace".into(),
            host_path: workspace.path().to_path_buf(),
            sandbox_path: PathBuf::from("/workspace"),
            access: MountAccess::ReadWrite,
            persist: false,
            owner: MountOwner::Workspace,
        }],
        policy: SandboxPolicy {
            network: NetworkPolicy::Disabled,
            default_timeout: Duration::from_secs(10),
            max_timeout: Duration::from_secs(30),
            max_output_bytes: 16,
            ..Default::default()
        },
        startup_timeout: Duration::from_secs(120),
        stop_timeout: Duration::from_secs(30),
        ..Default::default()
    };

    (workspace, config)
}

#[tokio::test]
#[ignore = "requires microsandbox runtime; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1 to run"]
async fn microsandbox_runtime_exec_and_file_roundtrip() {
    if !runtime_tests_enabled() {
        eprintln!("skipping real microsandbox test; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1");
        return;
    }

    let (_workspace, config) = runtime_config();
    let mut session = MicrosandboxSession::new(config).unwrap();
    session.initialize().await.unwrap();

    session.write_file("note.txt", b"hello").await.unwrap();
    assert_eq!(session.read_file("note.txt").await.unwrap(), b"hello");
    assert_eq!(
        session.read_file("/workspace/input.txt").await.unwrap(),
        b"mounted workspace"
    );

    let result = session
        .execute(ExecutionRequest {
            argv: vec![
                "python".into(),
                "-c".into(),
                "from pathlib import Path; print(Path.cwd()); print(Path('input.txt').read_text())"
                    .into(),
            ],
            cwd: None,
            env: Default::default(),
            timeout: Some(Duration::from_secs(10)),
            stdin: None,
        })
        .await
        .unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.truncated);
    assert!(result.stdout.full_ref.is_some());
    assert_eq!(
        String::from_utf8_lossy(&result.stdout.inline),
        "/workspace\nmount"
    );

    session.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires microsandbox runtime; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1 to run"]
async fn microsandbox_runtime_timeout_is_recorded() {
    if !runtime_tests_enabled() {
        eprintln!("skipping real microsandbox test; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1");
        return;
    }

    let (_workspace, config) = runtime_config();
    let mut session = MicrosandboxSession::new(config).unwrap();
    session.initialize().await.unwrap();

    let result = session
        .execute(ExecutionRequest {
            argv: vec![
                "python".into(),
                "-c".into(),
                "import time; time.sleep(5)".into(),
            ],
            cwd: None,
            env: Default::default(),
            timeout: Some(Duration::from_millis(100)),
            stdin: None,
        })
        .await
        .unwrap();

    assert!(
        result
            .resource_hits
            .iter()
            .any(|hit| matches!(hit, agent_scope_sandbox::ResourceLimitHit::Timeout))
    );
    assert!(
        session
            .history()
            .await
            .unwrap()
            .iter()
            .any(|record| record.execution_id == result.execution_id)
    );

    session.cleanup().await.unwrap();
}

#[tokio::test]
#[ignore = "requires microsandbox runtime; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1 to run"]
async fn microsandbox_runtime_nonzero_exit_is_not_backend_error() {
    if !runtime_tests_enabled() {
        eprintln!("skipping real microsandbox test; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1");
        return;
    }

    let (_workspace, config) = runtime_config();
    let mut session = MicrosandboxSession::new(config).unwrap();
    session.initialize().await.unwrap();

    let result = session
        .execute(ExecutionRequest {
            argv: vec![
                "python".into(),
                "-c".into(),
                "import sys; sys.exit(7)".into(),
            ],
            cwd: None,
            env: Default::default(),
            timeout: Some(Duration::from_secs(10)),
            stdin: None,
        })
        .await
        .unwrap();

    assert_eq!(result.exit_code, Some(7));
    assert!(matches!(result.status, ExecutionStatus::Exited { code: 7 }));

    session.cleanup().await.unwrap();
}

#[tokio::test]
#[ignore = "requires microsandbox runtime; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1 to run"]
async fn microsandbox_runtime_disabled_network_blocks_external_request() {
    if !runtime_tests_enabled() {
        eprintln!("skipping real microsandbox test; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1");
        return;
    }

    let (_workspace, config) = runtime_config();
    let mut session = MicrosandboxSession::new(config).unwrap();
    session.initialize().await.unwrap();

    let result = session
        .execute(ExecutionRequest {
            argv: vec![
                "python".into(),
                "-c".into(),
                "import urllib.request; urllib.request.urlopen('https://example.com', timeout=2)"
                    .into(),
            ],
            cwd: None,
            env: Default::default(),
            timeout: Some(Duration::from_secs(10)),
            stdin: None,
        })
        .await
        .unwrap();

    assert_ne!(result.exit_code, Some(0));

    session.cleanup().await.unwrap();
}

#[tokio::test]
#[ignore = "requires microsandbox runtime; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1 to run"]
async fn microsandbox_runtime_readonly_mount_rejects_writes() {
    if !runtime_tests_enabled() {
        eprintln!("skipping real microsandbox test; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("input.txt"), b"mounted").unwrap();
    let (_workspace, mut config) = runtime_config();
    config.mounts.push(SandboxMount {
        mount_id: "readonly-data".into(),
        host_path: dir.path().to_path_buf(),
        sandbox_path: PathBuf::from("/mnt/data"),
        access: MountAccess::ReadOnly,
        persist: false,
        owner: MountOwner::User,
    });

    let mut session = MicrosandboxSession::new(config).unwrap();
    session.initialize().await.unwrap();

    assert_eq!(
        session.read_file("/mnt/data/input.txt").await.unwrap(),
        b"mounted"
    );
    let err = session
        .write_file("/mnt/data/output.txt", b"should fail")
        .await
        .unwrap_err();
    assert!(matches!(err, SandboxError::PermissionDenied { .. }));

    session.cleanup().await.unwrap();
}

#[tokio::test]
#[ignore = "requires microsandbox runtime; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1 to run"]
async fn microsandbox_runtime_cleanup_is_idempotent() {
    if !runtime_tests_enabled() {
        eprintln!("skipping real microsandbox test; set AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1");
        return;
    }

    let (_workspace, config) = runtime_config();
    let mut session = MicrosandboxSession::new(config).unwrap();
    session.initialize().await.unwrap();

    session.cleanup().await.unwrap();
    session.cleanup().await.unwrap();
}
