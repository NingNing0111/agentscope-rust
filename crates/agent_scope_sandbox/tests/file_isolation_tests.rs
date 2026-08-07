use std::fs;

#[cfg(unix)]
use std::os::unix::fs::symlink;

use agent_scope_sandbox::{
    LocalSandboxConfig, LocalSandboxSession, MountAccess, MountOwner, SandboxMount, SandboxSession,
};

#[tokio::test]
async fn sandbox_file_isolation_write_read_delete_list() {
    let mut session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
    session.initialize().await.unwrap();
    session
        .write_file("notes/result.txt", b"hello")
        .await
        .unwrap();
    assert_eq!(
        session.read_file("notes/result.txt").await.unwrap(),
        b"hello"
    );
    let entries = session.list_dir("notes", true).await.unwrap();
    assert!(entries.iter().any(|e| e.ends_with("result.txt")));
    session.delete_path("notes/result.txt").await.unwrap();
    assert!(session.read_file("notes/result.txt").await.is_err());
}

#[tokio::test]
async fn sandbox_delete_path_refuses_root() {
    // `delete_path("/")` resolves to the sandbox root; deleting it would
    // recursively wipe the entire sandbox, so it must be refused (audit S9).
    let mut session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
    session.initialize().await.unwrap();
    session
        .write_file("notes/result.txt", b"hello")
        .await
        .unwrap();
    assert!(session.delete_path("/").await.is_err());
    // The sandbox is still usable afterwards.
    assert_eq!(
        session.read_file("notes/result.txt").await.unwrap(),
        b"hello"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn sandbox_path_policy_rejects_traversal_and_symlink_escape() {
    let outside = tempfile::tempdir().unwrap();
    let mut session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
    session.initialize().await.unwrap();
    assert!(session.write_file("../outside.txt", b"bad").await.is_err());

    let link = session.workdir().join("escape");
    symlink(outside.path(), &link).unwrap();
    assert!(session.read_file("escape/secret.txt").await.is_err());
}

#[tokio::test]
async fn sandbox_path_policy_readonly_mount_denies_writes() {
    let root = tempfile::tempdir().unwrap();
    let ro = root.path().join("work/ro");
    fs::create_dir_all(&ro).unwrap();
    let mount = SandboxMount {
        mount_id: "ro".into(),
        host_path: ro.clone(),
        sandbox_path: "ro".into(),
        access: MountAccess::ReadOnly,
        persist: true,
        owner: MountOwner::User,
    };
    let mut session = LocalSandboxSession::new(LocalSandboxConfig {
        root_dir: Some(root.path().to_path_buf()),
        workdir: Some(root.path().join("work")),
        mounts: vec![mount],
        ..Default::default()
    })
    .unwrap();
    session.initialize().await.unwrap();
    session
        .write_file("ro/file.txt", b"seed")
        .await
        .unwrap_err();
    fs::write(ro.join("file.txt"), b"seed").unwrap();
    assert!(session.delete_path("ro/file.txt").await.is_err());
}
