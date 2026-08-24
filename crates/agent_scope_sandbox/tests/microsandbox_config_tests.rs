#![cfg(feature = "microsandbox")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use agent_scope_sandbox::policy::{memory_bytes_to_mib, validate_microsandbox_policy};
use agent_scope_sandbox::{
    CapabilityReport, CompatibilityLevel, CpuLimit, MicrosandboxConfig, MicrosandboxSession,
    MountAccess, MountOwner, NetworkPolicy, SandboxCapability, SandboxError, SandboxMount,
    SandboxPolicy, SandboxSession,
};

#[test]
fn microsandbox_default_policy_is_no_net() {
    assert_eq!(
        MicrosandboxConfig::default().policy.network,
        NetworkPolicy::Disabled
    );
}

#[test]
fn microsandbox_config_rejects_invalid_session_id() {
    let err = MicrosandboxSession::new(MicrosandboxConfig {
        session_id: Some("bad/id".into()),
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::ValidationError { .. }));
}

#[test]
fn microsandbox_config_rejects_empty_image_and_workdir() {
    let err = MicrosandboxSession::new(MicrosandboxConfig {
        image: " ".into(),
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::ValidationError { .. }));

    let err = MicrosandboxSession::new(MicrosandboxConfig {
        workdir: "relative".into(),
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::ValidationError { .. }));

    let err = MicrosandboxSession::new(MicrosandboxConfig {
        workdir: "/".into(),
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::ValidationError { .. }));
}

#[test]
fn microsandbox_debug_redacts_env_values() {
    let mut env = HashMap::new();
    env.insert("SAFE_TOKEN".to_string(), "super-secret-value".to_string());
    let config = MicrosandboxConfig {
        env,
        ..Default::default()
    };
    let debug = format!("{config:?}");

    assert!(debug.contains("env_keys"));
    assert!(debug.contains("SAFE_TOKEN"));
    assert!(!debug.contains("super-secret-value"));

    let session = MicrosandboxSession::new(config).unwrap();
    let debug = format!("{session:?}");
    assert!(debug.contains("env_keys"));
    assert!(debug.contains("SAFE_TOKEN"));
    assert!(!debug.contains("super-secret-value"));
}

#[test]
fn microsandbox_config_rejects_zero_timeouts_and_reserved_env_keys() {
    let err = MicrosandboxSession::new(MicrosandboxConfig {
        startup_timeout: Duration::ZERO,
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::ValidationError { .. }));

    let mut env = HashMap::new();
    env.insert("MSB_API_KEY".to_string(), "not-secret-in-test".to_string());
    let err = MicrosandboxSession::new(MicrosandboxConfig {
        env,
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::ValidationError { .. }));
}

#[test]
fn microsandbox_config_rejects_missing_and_sensitive_host_mounts() {
    let missing = MicrosandboxSession::new(MicrosandboxConfig {
        mounts: vec![SandboxMount {
            mount_id: "missing".into(),
            host_path: PathBuf::from("/definitely/not/a/real/agentscope/path"),
            sandbox_path: PathBuf::from("/mnt/missing"),
            access: MountAccess::ReadOnly,
            persist: false,
            owner: MountOwner::User,
        }],
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(missing, SandboxError::ValidationError { .. }));

    let dir = tempfile::tempdir().unwrap();
    let sensitive = dir.path().join(".ssh");
    std::fs::create_dir(&sensitive).unwrap();
    let err = MicrosandboxSession::new(MicrosandboxConfig {
        mounts: vec![SandboxMount {
            mount_id: "ssh".into(),
            host_path: sensitive,
            sandbox_path: PathBuf::from("/mnt/ssh"),
            access: MountAccess::ReadOnly,
            persist: false,
            owner: MountOwner::User,
        }],
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::PermissionDenied { .. }));
}

#[test]
#[cfg(unix)]
fn microsandbox_config_rejects_symlink_to_sensitive_host_mount() {
    let dir = tempfile::tempdir().unwrap();
    let sensitive = dir.path().join(".ssh");
    std::fs::create_dir(&sensitive).unwrap();
    let link = dir.path().join("safe-looking");
    std::os::unix::fs::symlink(&sensitive, &link).unwrap();

    let err = MicrosandboxSession::new(MicrosandboxConfig {
        mounts: vec![SandboxMount {
            mount_id: "linked".into(),
            host_path: link,
            sandbox_path: PathBuf::from("/mnt/data"),
            access: MountAccess::ReadOnly,
            persist: false,
            owner: MountOwner::User,
        }],
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::PermissionDenied { .. }));
}

#[test]
fn microsandbox_config_accepts_existing_non_sensitive_mount() {
    let dir = tempfile::tempdir().unwrap();
    let session = MicrosandboxSession::new(MicrosandboxConfig {
        mounts: vec![SandboxMount {
            mount_id: "data".into(),
            host_path: dir.path().to_path_buf(),
            sandbox_path: PathBuf::from("/mnt/data"),
            access: MountAccess::ReadOnly,
            persist: false,
            owner: MountOwner::User,
        }],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(session.state(), agent_scope_sandbox::SandboxState::Created);
}

#[test]
fn microsandbox_policy_accepts_disabled_and_unrestricted_network() {
    validate_microsandbox_policy(&SandboxPolicy {
        network: NetworkPolicy::Disabled,
        ..Default::default()
    })
    .unwrap();

    validate_microsandbox_policy(&SandboxPolicy {
        network: NetworkPolicy::Unrestricted,
        ..Default::default()
    })
    .unwrap();
}

#[test]
fn microsandbox_policy_rejects_non_equivalent_network_and_resource_limits() {
    let err = validate_microsandbox_policy(&SandboxPolicy {
        network: NetworkPolicy::LoopbackOnly,
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::UnsupportedFeature { .. }));

    let err = validate_microsandbox_policy(&SandboxPolicy {
        network: NetworkPolicy::Allowlist {
            hosts: vec!["example.com".into()],
        },
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::UnsupportedFeature { .. }));

    let err = validate_microsandbox_policy(&SandboxPolicy {
        cpu_limit: Some(CpuLimit { cpu_shares: 1024 }),
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::UnsupportedFeature { .. }));

    let err = validate_microsandbox_policy(&SandboxPolicy {
        process_limit: Some(32),
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, SandboxError::UnsupportedFeature { .. }));
}

#[test]
fn microsandbox_memory_bytes_round_up_to_mib() {
    assert_eq!(memory_bytes_to_mib(1).unwrap(), 1);
    assert_eq!(memory_bytes_to_mib(1024 * 1024).unwrap(), 1);
    assert_eq!(memory_bytes_to_mib(1024 * 1024 + 1).unwrap(), 2);
    assert!(matches!(
        memory_bytes_to_mib(0).unwrap_err(),
        SandboxError::ValidationError { .. }
    ));
}

#[test]
fn microsandbox_capability_report_declares_hard_isolation_without_local_fallback() {
    let report = CapabilityReport::microsandbox();
    assert_eq!(report.backend_name, "microsandbox");
    assert_eq!(report.compatibility_level, CompatibilityLevel::L4);
    assert!(
        report
            .supported
            .contains(&SandboxCapability::HardwareIsolation)
    );
    assert!(
        report
            .supported
            .contains(&SandboxCapability::MicrosandboxRuntime)
    );
    assert!(
        report
            .unsupported
            .iter()
            .any(|u| u.capability == "network_allowlist")
    );
    assert!(
        report
            .known_deviations
            .iter()
            .any(|d| d.contains("MSB_API_KEY"))
    );
}

#[tokio::test]
async fn microsandbox_new_session_reports_capability_before_runtime_start() {
    let session = MicrosandboxSession::new(MicrosandboxConfig::default()).unwrap();
    let report = session.capability_report().await.unwrap();
    assert_eq!(report.backend_name, "microsandbox");
}
