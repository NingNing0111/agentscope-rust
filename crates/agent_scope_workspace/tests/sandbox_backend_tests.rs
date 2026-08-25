use agent_scope_sandbox::{
    CapabilityReport, ExecutionRecord, ExecutionRequest, ExecutionResult, LocalSandboxConfig,
    LocalSandboxSession, SandboxError, SandboxPolicy, SandboxSession, SandboxState,
    SandboxWorkspaceBackend,
};
use agent_scope_workspace::{WorkspaceBackend, WorkspaceError};

#[derive(Debug)]
struct PermissionDeniedSession {
    state: SandboxState,
    policy: SandboxPolicy,
}

impl Default for PermissionDeniedSession {
    fn default() -> Self {
        Self {
            state: SandboxState::Ready,
            policy: SandboxPolicy::default(),
        }
    }
}

#[async_trait::async_trait]
impl SandboxSession for PermissionDeniedSession {
    fn session_id(&self) -> &str {
        "permission-denied-session"
    }

    fn state(&self) -> SandboxState {
        self.state
    }

    fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    async fn initialize(&mut self) -> Result<(), SandboxError> {
        self.state = SandboxState::Ready;
        Ok(())
    }

    async fn execute(
        &mut self,
        _request: ExecutionRequest,
    ) -> Result<ExecutionResult, SandboxError> {
        Err(SandboxError::PermissionDenied {
            path: Some("/readonly/output.txt".into()),
            operation: "execute".into(),
        })
    }

    async fn read_file(&self, _path: &str) -> Result<Vec<u8>, SandboxError> {
        Err(SandboxError::PermissionDenied {
            path: Some("/readonly/input.txt".into()),
            operation: "read_file".into(),
        })
    }

    async fn write_file(&mut self, _path: &str, _data: &[u8]) -> Result<(), SandboxError> {
        Err(SandboxError::PermissionDenied {
            path: Some("/readonly/output.txt".into()),
            operation: "write_file".into(),
        })
    }

    async fn delete_path(&mut self, _path: &str) -> Result<(), SandboxError> {
        Err(SandboxError::PermissionDenied {
            path: Some("/readonly/output.txt".into()),
            operation: "delete_path".into(),
        })
    }

    async fn is_dir(&self, _path: &str) -> Result<bool, SandboxError> {
        Err(SandboxError::PermissionDenied {
            path: Some("/readonly".into()),
            operation: "is_dir".into(),
        })
    }

    async fn path_exists(&self, _path: &str) -> Result<bool, SandboxError> {
        Err(SandboxError::PermissionDenied {
            path: Some("/readonly/input.txt".into()),
            operation: "path_exists".into(),
        })
    }

    async fn stat_mtime(&self, _path: &str) -> Result<Option<f64>, SandboxError> {
        Err(SandboxError::PermissionDenied {
            path: Some("/readonly/input.txt".into()),
            operation: "stat_mtime".into(),
        })
    }

    async fn list_dir(&self, _path: &str, _recursive: bool) -> Result<Vec<String>, SandboxError> {
        Err(SandboxError::PermissionDenied {
            path: Some("/readonly".into()),
            operation: "list_dir".into(),
        })
    }

    async fn history(&self) -> Result<Vec<ExecutionRecord>, SandboxError> {
        Ok(Vec::new())
    }

    async fn capability_report(&self) -> Result<CapabilityReport, SandboxError> {
        Ok(CapabilityReport::local_process())
    }

    async fn close(&mut self) -> Result<(), SandboxError> {
        self.state = SandboxState::Closed;
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), SandboxError> {
        self.close().await
    }
}

async fn backend() -> SandboxWorkspaceBackend {
    let session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
    let backend = SandboxWorkspaceBackend::new(session);
    backend.initialize().await.unwrap();
    backend
}

#[tokio::test]
async fn sandbox_backend_from_session_constructor_uses_trait_object() {
    let session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
    let backend = SandboxWorkspaceBackend::from_session(session);
    backend.initialize().await.unwrap();
    backend
        .write_file("constructor.txt", b"from_session")
        .await
        .unwrap();
    assert_eq!(
        backend.read_file("constructor.txt").await.unwrap(),
        b"from_session"
    );
    backend.close().await.unwrap();
}

#[tokio::test]
async fn sandbox_backend_from_boxed_session_constructor_uses_trait_object() {
    let session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
    let boxed: Box<dyn SandboxSession> = Box::new(session);
    let backend = SandboxWorkspaceBackend::from_boxed_session(boxed);
    backend.initialize().await.unwrap();
    let output = backend
        .exec_shell(&["printf", "boxed"], ".", None)
        .await
        .unwrap();
    assert!(output.ok());
    assert_eq!(output.stdout, b"boxed");
    backend.close().await.unwrap();
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
async fn sandbox_backend_permission_denied_maps_to_path_traversal() {
    let backend = SandboxWorkspaceBackend::from_session(PermissionDeniedSession::default());

    let err = backend.write_file("output.txt", b"x").await.unwrap_err();
    assert!(matches!(
        err,
        WorkspaceError::PathTraversal { ref path }
            if path == "/readonly/output.txt"
    ));

    let err = backend.file_exists("input.txt").await.unwrap_err();
    assert!(matches!(
        err,
        WorkspaceError::PathTraversal { ref path }
            if path == "/readonly/input.txt"
    ));
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
    // join_path is platform-native (Windows renders `\`); accept either separator.
    assert!(
        backend.join_path("a", "b").ends_with("a/b")
            || backend.join_path("a", "b").ends_with("a\\b")
    );
}
