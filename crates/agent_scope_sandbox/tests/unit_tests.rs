use std::path::PathBuf;
use std::time::Duration;

use agent_scope_sandbox::execution::failure_category;
use agent_scope_sandbox::path::SandboxPathResolver;
use agent_scope_sandbox::{
    CpuLimit, ExecutionRequest, ExecutionStatus, MountAccess, MountOwner, NetworkPolicy,
    SandboxError, SandboxMount, SandboxPolicy, redacted_command_summary,
};

#[test]
fn execution_request_new_sets_safe_defaults() {
    let req = ExecutionRequest::new(["sh", "-c", "printf ok"]);

    assert_eq!(req.argv, vec!["sh", "-c", "printf ok"]);
    assert!(req.cwd.is_none());
    assert!(req.env.is_empty());
    assert!(req.timeout.is_none());
    assert!(req.stdin.is_none());
}

#[test]
fn redacted_command_summary_sorts_env_keys_without_values() {
    let mut req = ExecutionRequest::new(["cmd"]);
    req.env.insert("TOKEN".into(), "secret".into());
    req.env.insert("API_KEY".into(), "also-secret".into());

    let summary = redacted_command_summary(&req);

    assert_eq!(summary, "cmd env=[API_KEY,TOKEN]");
    assert!(!summary.contains("secret"));
}

#[test]
fn failure_category_covers_all_statuses() {
    assert_eq!(failure_category(&ExecutionStatus::Exited { code: 0 }), None);
    assert_eq!(
        failure_category(&ExecutionStatus::Exited { code: 2 }).as_deref(),
        Some("non_zero_exit")
    );
    assert_eq!(
        failure_category(&ExecutionStatus::TimedOut).as_deref(),
        Some("timeout")
    );
    assert_eq!(
        failure_category(&ExecutionStatus::PermissionDenied).as_deref(),
        Some("permission_denied")
    );
    assert_eq!(
        failure_category(&ExecutionStatus::UnsupportedFeature).as_deref(),
        Some("unsupported_feature")
    );
    assert_eq!(
        failure_category(&ExecutionStatus::SandboxError).as_deref(),
        Some("sandbox_error")
    );
    assert_eq!(
        failure_category(&ExecutionStatus::Cancelled).as_deref(),
        Some("cancelled")
    );
}

#[test]
fn execution_request_timeout_serializes_as_seconds() {
    let mut req = ExecutionRequest::new(["sleep", "1"]);
    req.timeout = Some(Duration::from_millis(1500));

    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains(r#""timeout":1.5"#));

    let restored: ExecutionRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.timeout, Some(Duration::from_millis(1500)));
}

#[test]
fn sandbox_policy_validate_rejects_invalid_bounds() {
    let policy = SandboxPolicy {
        default_timeout: Duration::from_secs(60),
        max_timeout: Duration::from_secs(30),
        ..Default::default()
    };
    assert!(matches!(
        policy.validate(),
        Err(SandboxError::ValidationError { .. })
    ));

    let policy = SandboxPolicy {
        max_output_bytes: 0,
        ..Default::default()
    };
    assert!(matches!(
        policy.validate(),
        Err(SandboxError::ValidationError { .. })
    ));
}

#[test]
fn unsupported_features_lists_resource_and_network_requests() {
    let policy = SandboxPolicy {
        cpu_limit: Some(CpuLimit { cpu_shares: 256 }),
        memory_limit_bytes: Some(1024),
        process_limit: Some(4),
        network: NetworkPolicy::Allowlist {
            hosts: vec!["example.com".into()],
        },
        ..Default::default()
    };

    let names: Vec<_> = policy
        .requested_unsupported_features()
        .into_iter()
        .map(|(name, _)| name)
        .collect();

    assert_eq!(
        names,
        vec![
            "cpu_limit",
            "memory_limit",
            "process_limit",
            "network_policy"
        ]
    );
}

#[test]
fn mount_validate_normalizes_relative_paths_under_workdir() {
    let root = tempfile::tempdir().unwrap();
    let mut mount = SandboxMount {
        mount_id: "m1".into(),
        host_path: root.path().join("host"),
        sandbox_path: PathBuf::from("cache"),
        access: MountAccess::ReadWrite,
        persist: false,
        owner: MountOwner::Session,
    };

    mount.validate(root.path()).unwrap();

    assert_eq!(mount.sandbox_path, root.path().join("work/cache"));
}

#[test]
fn mount_validate_rejects_escape_and_empty_id() {
    let root = tempfile::tempdir().unwrap();
    let mut empty_id = SandboxMount {
        mount_id: String::new(),
        host_path: root.path().join("host"),
        sandbox_path: PathBuf::from("cache"),
        access: MountAccess::ReadOnly,
        persist: false,
        owner: MountOwner::User,
    };
    assert!(matches!(
        empty_id.validate(root.path()),
        Err(SandboxError::ValidationError { .. })
    ));

    let mut escape = SandboxMount {
        mount_id: "escape".into(),
        host_path: root.path().join("host"),
        sandbox_path: PathBuf::from("/tmp/outside"),
        access: MountAccess::ReadOnly,
        persist: false,
        owner: MountOwner::User,
    };
    assert!(matches!(
        escape.validate(root.path()),
        Err(SandboxError::PermissionDenied { .. })
    ));
}

#[test]
fn path_resolver_rejects_parent_traversal_and_resolves_absolute_inside_root() {
    let root = tempfile::tempdir().unwrap();
    let workdir = root.path().join("work");
    let resolver = SandboxPathResolver::new(root.path().to_path_buf(), workdir.clone()).unwrap();

    assert!(matches!(
        resolver.resolve("../escape", None, false, "write"),
        Err(SandboxError::PermissionDenied { .. })
    ));

    let resolved = resolver
        .resolve("/nested/file.txt", None, false, "write")
        .unwrap();
    assert_eq!(
        resolved,
        root.path().canonicalize().unwrap().join("nested/file.txt")
    );
}
