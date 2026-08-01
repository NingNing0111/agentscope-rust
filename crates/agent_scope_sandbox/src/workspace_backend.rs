//! Workspace backend adapter for sandbox sessions.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use agent_scope_workspace::{ExecOutput, WorkspaceBackend, WorkspaceError};
use tokio::sync::Mutex;

use crate::error::SandboxError;
use crate::execution::{ExecutionRequest, ExecutionStatus};
use crate::local::LocalSandboxSession;
use crate::session::SandboxSession;

#[derive(Debug, Clone)]
pub struct SandboxWorkspaceBackend {
    session: Arc<Mutex<LocalSandboxSession>>,
    instructions: String,
}

impl SandboxWorkspaceBackend {
    #[must_use]
    pub fn new(session: LocalSandboxSession) -> Self {
        let workspace_root = session.workdir().to_path_buf();
        let instructions = format!(
            "Sandbox workspace rooted at {}. Path traversal and read-only mount writes are refused. Unsupported hard isolation capabilities are reported explicitly.",
            workspace_root.display()
        );
        Self {
            session: Arc::new(Mutex::new(session)),
            instructions,
        }
    }

    #[must_use]
    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    pub async fn initialize(&self) -> Result<(), WorkspaceError> {
        self.session
            .lock()
            .await
            .initialize()
            .await
            .map_err(sandbox_to_workspace)
    }

    pub async fn close(&self) -> Result<(), WorkspaceError> {
        self.session
            .lock()
            .await
            .close()
            .await
            .map_err(sandbox_to_workspace)
    }
}

fn sandbox_to_workspace(err: SandboxError) -> WorkspaceError {
    match err {
        SandboxError::PermissionDenied { path, .. } => WorkspaceError::PathTraversal {
            path: path.unwrap_or_default(),
        },
        other => WorkspaceError::GatewayError {
            message: format!("{}: {other}", other.category()),
        },
    }
}

#[async_trait::async_trait]
impl WorkspaceBackend for SandboxWorkspaceBackend {
    async fn exec_shell(
        &self,
        cmd: &[&str],
        cwd: &str,
        timeout_secs: Option<f64>,
    ) -> Result<ExecOutput, WorkspaceError> {
        let mut session = self.session.lock().await;
        let mut req = ExecutionRequest::new(cmd.iter().copied());
        req.cwd = Some(PathBuf::from(cwd));
        req.timeout = timeout_secs.map(Duration::from_secs_f64);
        let result = session.execute(req).await.map_err(sandbox_to_workspace)?;
        let exit_code = match result.status {
            ExecutionStatus::Exited { code } => code,
            ExecutionStatus::TimedOut => -124,
            _ => -1,
        };
        Ok(ExecOutput {
            stdout: result.stdout.inline,
            stderr: result.stderr.inline,
            exit_code,
        })
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, WorkspaceError> {
        self.session
            .lock()
            .await
            .read_file(path)
            .await
            .map_err(sandbox_to_workspace)
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), WorkspaceError> {
        self.session
            .lock()
            .await
            .write_file(path, data)
            .await
            .map_err(sandbox_to_workspace)
    }

    async fn is_dir(&self, path: &str) -> Result<bool, WorkspaceError> {
        self.session
            .lock()
            .await
            .is_dir(path)
            .await
            .map_err(sandbox_to_workspace)
    }

    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, WorkspaceError> {
        self.session
            .lock()
            .await
            .list_dir(path, recursive)
            .await
            .map_err(sandbox_to_workspace)
    }

    async fn delete_path(&self, path: &str) -> Result<(), WorkspaceError> {
        self.session
            .lock()
            .await
            .delete_path(path)
            .await
            .map_err(sandbox_to_workspace)
    }

    async fn file_exists(&self, path: &str) -> Result<bool, WorkspaceError> {
        match self.session.lock().await.read_file(path).await {
            Ok(_) => Ok(true),
            Err(SandboxError::IoError { .. }) => Ok(false),
            Err(e) => Err(sandbox_to_workspace(e)),
        }
    }

    fn join_path(&self, a: &str, b: &str) -> String {
        Path::new(a).join(b).to_string_lossy().to_string()
    }
    fn basename(&self, path: &str) -> String {
        Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }
    fn dirname(&self, path: &str) -> String {
        Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into())
    }

    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, WorkspaceError> {
        self.session
            .lock()
            .await
            .stat_mtime(path)
            .await
            .map_err(sandbox_to_workspace)
    }

    fn normpath(&self, path: &str) -> String {
        let mut buf = PathBuf::new();
        for component in Path::new(path).components() {
            match component {
                std::path::Component::ParentDir => {
                    buf.pop();
                }
                std::path::Component::CurDir => {}
                c => buf.push(c.as_os_str()),
            }
        }
        buf.to_string_lossy().to_string()
    }
    fn is_absolute(&self, path: &str) -> bool {
        Path::new(path).is_absolute()
    }
}
