//! Workspace backend trait + LocalBackend implementation.

use std::path::{Path, PathBuf};

use crate::error::WorkspaceError;

// ---------------------------------------------------------------------------
// ExecOutput
// ---------------------------------------------------------------------------

/// Result of a shell command execution.
#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

impl ExecOutput {
    /// True if exit_code == 0.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.exit_code == 0
    }
}

// ---------------------------------------------------------------------------
// WorkspaceBackend trait
// ---------------------------------------------------------------------------

/// Abstract filesystem and process-I/O backend for Workspace.
///
/// Implementations: `LocalBackend` (local filesystem), future Docker/E2B/K8s.
#[async_trait::async_trait]
pub trait WorkspaceBackend: Send + Sync {
    /// Execute a shell command. `cmd` is the argv array.
    async fn exec_shell(
        &self,
        cmd: &[&str],
        cwd: &str,
        timeout_secs: Option<f64>,
    ) -> Result<ExecOutput, WorkspaceError>;

    /// Read entire file contents as raw bytes.
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, WorkspaceError>;

    /// Write data to a file, creating parent directories automatically.
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), WorkspaceError>;

    /// Check whether `path` is a directory.
    async fn is_dir(&self, path: &str) -> Result<bool, WorkspaceError>;

    /// List directory entries. Returns full paths.
    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, WorkspaceError>;

    /// Delete a file or recursively delete a directory.
    /// Idempotent: no error if path does not exist.
    async fn delete_path(&self, path: &str) -> Result<(), WorkspaceError>;

    /// Check whether a file or directory exists.
    async fn file_exists(&self, path: &str) -> Result<bool, WorkspaceError>;

    /// Join two path components (synchronous, pure string operation).
    fn join_path(&self, a: &str, b: &str) -> String;

    /// Return the last component of a path.
    fn basename(&self, path: &str) -> String;

    /// Return the parent directory of a path.
    fn dirname(&self, path: &str) -> String;

    /// Get modification time as Unix timestamp (seconds since epoch).
    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, WorkspaceError>;

    /// Normalize a path (remove `.` and resolve `..`).
    fn normpath(&self, path: &str) -> String;

    /// Check if a path is absolute.
    fn is_absolute(&self, path: &str) -> bool;
}

// ---------------------------------------------------------------------------
// LocalBackend
// ---------------------------------------------------------------------------

/// Filesystem backend that delegates to `tokio::fs` and `tokio::process`.
#[derive(Debug, Clone)]
pub struct LocalBackend;

impl LocalBackend {
    /// Create a new LocalBackend.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl WorkspaceBackend for LocalBackend {
    async fn exec_shell(
        &self,
        cmd: &[&str],
        cwd: &str,
        _timeout_secs: Option<f64>,
    ) -> Result<ExecOutput, WorkspaceError> {
        if cmd.is_empty() {
            return Err(WorkspaceError::BackendError {
                message: "exec_shell: cmd must not be empty".into(),
            });
        }
        let child = tokio::process::Command::new(cmd[0])
            .args(&cmd[1..])
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| WorkspaceError::BackendError {
                message: format!("failed to spawn command '{}': {e}", cmd[0]),
            })?;

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| WorkspaceError::BackendError {
                message: format!("failed to wait on command '{}': {e}", cmd[0]),
            })?;

        Ok(ExecOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, WorkspaceError> {
        tokio::fs::read(path)
            .await
            .map_err(|e| WorkspaceError::BackendError {
                message: format!("read_file '{path}': {e}"),
            })
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), WorkspaceError> {
        if let Some(parent) = Path::new(path).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| WorkspaceError::BackendError {
                    message: format!("write_file create_dir_all '{path}': {e}"),
                })?;
        }
        tokio::fs::write(path, data)
            .await
            .map_err(|e| WorkspaceError::BackendError {
                message: format!("write_file '{path}': {e}"),
            })
    }

    async fn is_dir(&self, path: &str) -> Result<bool, WorkspaceError> {
        Ok(Path::new(path).is_dir())
    }

    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, WorkspaceError> {
        fn collect_entries(path: &Path, recursive: bool) -> Result<Vec<String>, WorkspaceError> {
            let mut entries = Vec::new();
            let dir = std::fs::read_dir(path).map_err(|e| WorkspaceError::BackendError {
                message: format!("list_dir '{path:?}': {e}"),
            })?;
            for entry in dir {
                let entry = entry.map_err(|e| WorkspaceError::BackendError {
                    message: format!("list_dir entry '{path:?}': {e}"),
                })?;
                let full = entry.path().to_string_lossy().to_string();
                entries.push(full.clone());
                if recursive && entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    entries.extend(collect_entries(&entry.path(), true)?);
                }
            }
            Ok(entries)
        }

        let p = Path::new(path);
        if !p.is_dir() {
            return Ok(Vec::new());
        }
        collect_entries(p, recursive)
    }

    async fn delete_path(&self, path: &str) -> Result<(), WorkspaceError> {
        let p = Path::new(path);
        if !p.exists() {
            return Ok(());
        }
        if p.is_dir() {
            tokio::fs::remove_dir_all(path)
                .await
                .map_err(|e| WorkspaceError::BackendError {
                    message: format!("delete_path rmdir '{path}': {e}"),
                })?;
        } else {
            tokio::fs::remove_file(path)
                .await
                .map_err(|e| WorkspaceError::BackendError {
                    message: format!("delete_path rm '{path}': {e}"),
                })?;
        }
        Ok(())
    }

    async fn file_exists(&self, path: &str) -> Result<bool, WorkspaceError> {
        Ok(Path::new(path).exists())
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
        let metadata =
            tokio::fs::metadata(path)
                .await
                .map_err(|e| WorkspaceError::BackendError {
                    message: format!("stat_mtime '{path}': {e}"),
                })?;
        let modified = metadata
            .modified()
            .map_err(|e| WorkspaceError::BackendError {
                message: format!("stat_mtime modified '{path}': {e}"),
            })?;
        Ok(Some(
            modified
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        ))
    }

    fn normpath(&self, path: &str) -> String {
        let p = Path::new(path);
        let mut buf = PathBuf::new();
        for component in p.components() {
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
