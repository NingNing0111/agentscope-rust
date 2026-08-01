use agent_scope_sandbox::{LocalSandboxConfig, LocalSandboxSession, SandboxWorkspaceBackend};
use agent_scope_workspace::WorkspaceBackend;

async fn backend() -> SandboxWorkspaceBackend {
    let session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
    let backend = SandboxWorkspaceBackend::new(session);
    backend.initialize().await.unwrap();
    backend
}

#[tokio::test]
async fn sandbox_backend_exec_read_write() {
    let backend = backend().await;
    backend.write_file("hello.txt", b"world").await.unwrap();
    assert_eq!(backend.read_file("hello.txt").await.unwrap(), b"world");
    let output = backend
        .exec_shell(&["printf", "ok"], ".", None)
        .await
        .unwrap();
    assert!(output.ok());
    assert_eq!(output.stdout, b"ok");
}

#[tokio::test]
async fn sandbox_backend_file_ops() {
    let backend = backend().await;
    backend.write_file("dir/file.txt", b"x").await.unwrap();
    assert!(backend.file_exists("dir/file.txt").await.unwrap());
    assert!(backend.is_dir("dir").await.unwrap());
    assert!(backend.stat_mtime("dir/file.txt").await.unwrap().is_some());
    let entries = backend.list_dir("dir", true).await.unwrap();
    assert!(entries.iter().any(|e| e.ends_with("file.txt")));
    backend.delete_path("dir/file.txt").await.unwrap();
    assert!(!backend.file_exists("dir/file.txt").await.unwrap());
}

#[tokio::test]
async fn sandbox_backend_path_traversal_rejected() {
    let backend = backend().await;
    assert!(backend.write_file("../escape.txt", b"bad").await.is_err());
}

#[tokio::test]
async fn sandbox_backend_absolute_host_metadata_does_not_leak() {
    let backend = backend().await;
    assert!(!backend.is_dir("/etc").await.unwrap());
    assert!(backend.stat_mtime("/etc/passwd").await.unwrap().is_none());
}
#[tokio::test]
async fn sandbox_backend_reset_close_cleanup() {
    let backend = backend().await;
    backend.write_file("x.txt", b"x").await.unwrap();
    backend.close().await.unwrap();
    assert!(backend.read_file("x.txt").await.is_err());
}

#[tokio::test]
async fn sandbox_backend_path_helpers() {
    let backend = backend().await;
    assert_eq!(backend.basename("/a/b.txt"), "b.txt");
    assert_eq!(backend.dirname("/a/b.txt"), "/a");
    assert_eq!(backend.normpath("/a/./b/../c"), "/a/c");
    assert!(backend.is_absolute("/a"));
    assert!(backend.join_path("a", "b").ends_with("a/b"));
}
