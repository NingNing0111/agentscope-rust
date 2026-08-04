//! Workspace backend trait + LocalBackend implementation.

use std::path::{Path, PathBuf};

use crate::error::WorkspaceError;

/// Upper bound on the bytes read from a shell command's stdout/stderr, so an
/// unbounded producer (e.g. `yes`) cannot exhaust memory. Reading past this
/// limit truncates the captured output.
const MAX_SHELL_OUTPUT_BYTES: usize = 1_048_576; // 1 MiB

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
        timeout_secs: Option<f64>,
    ) -> Result<ExecOutput, WorkspaceError> {
        if cmd.is_empty() {
            return Err(WorkspaceError::BackendError {
                message: "exec_shell: cmd must not be empty".into(),
            });
        }
        let mut child = tokio::process::Command::new(cmd[0])
            .args(&cmd[1..])
            .current_dir(cwd)
            // Do not leak the host environment (API keys etc.) into commands the
            // agent can run, like the sandbox backend. Inherit PATH so commands
            // that need host tooling (/usr/local/bin, brew, node) still resolve
            // — clearing it outright breaks legitimate agent workflows.
            .env_clear()
            .env(
                "PATH",
                std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".into()),
            )
            .env("HOME", cwd)
            .env("TMPDIR", std::env::temp_dir())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| WorkspaceError::BackendError {
                message: format!("failed to spawn command '{}': {e}", cmd[0]),
            })?;

        // Read stdout/stderr from dedicated tasks so we can wait on the child
        // with a timeout and still collect the pipes afterwards. Reads are
        // bounded so an unbounded producer cannot exhaust memory.
        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();
        let mut stdout_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            if let Some(s) = stdout_pipe {
                use tokio::io::AsyncReadExt;
                s.take((MAX_SHELL_OUTPUT_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .await?;
            }
            Ok::<Vec<u8>, std::io::Error>(bytes)
        });
        let mut stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            if let Some(s) = stderr_pipe {
                use tokio::io::AsyncReadExt;
                s.take((MAX_SHELL_OUTPUT_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .await?;
            }
            Ok::<Vec<u8>, std::io::Error>(bytes)
        });

        let wait_fut = child.wait();
        let mut exit_code = match timeout_secs {
            Some(secs) if secs > 0.0 => {
                match tokio::time::timeout(std::time::Duration::from_secs_f64(secs), wait_fut)
                    .await
                {
                    Ok(res) => res
                        .map_err(|e| WorkspaceError::BackendError {
                            message: format!("failed to wait on command '{}': {e}", cmd[0]),
                        })?
                        .code()
                        .unwrap_or(-1),
                    Err(_) => {
                        // Honor the timeout like the sandbox backend does:
                        // kill the child and report a timeout exit code.
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        124 // matches the `timeout` command
                    }
                }
            }
            _ => wait_fut
                .await
                .map_err(|e| WorkspaceError::BackendError {
                    message: format!("failed to wait on command '{}': {e}", cmd[0]),
                })?
                .code()
                .unwrap_or(-1),
        };

        // A grandchild that inherited the pipe keeps it open, so the read task
        // would never reach EOF. Bound the joins with the same deadline so a
        // command like `sh -c 'sleep 1000 &'` cannot hang the caller. When no
        // timeout was requested, use a generous backstop (5 minutes) instead of
        // an arbitrary 30s that would kill legitimate long-running reads.
        let read_timeout = timeout_secs
            .filter(|s| *s > 0.0)
            .map(std::time::Duration::from_secs_f64)
            .unwrap_or(std::time::Duration::from_secs(300));
        // `tokio::select!` with a `&mut` handle (rather than `timeout`) keeps the
        // JoinHandle alive so the timeout branch can abort the detached read
        // task instead of letting it hold the pipe forever (audit S3).
        let stdout = tokio::select! {
            joined = &mut stdout_task => joined
                .map_err(|e| WorkspaceError::BackendError {
                    message: format!("stdout task join failed: {e}"),
                })?
                .map_err(|e| WorkspaceError::BackendError {
                    message: format!("stdout read failed: {e}"),
                })?,
            _ = tokio::time::sleep(read_timeout) => {
                let _ = child.kill().await;
                exit_code = 124;
                stdout_task.abort();
                Vec::new()
            }
        };
        let stderr = tokio::select! {
            joined = &mut stderr_task => joined
                .map_err(|e| WorkspaceError::BackendError {
                    message: format!("stderr task join failed: {e}"),
                })?
                .map_err(|e| WorkspaceError::BackendError {
                    message: format!("stderr read failed: {e}"),
                })?,
            _ = tokio::time::sleep(read_timeout) => {
                let _ = child.kill().await;
                exit_code = 124;
                stderr_task.abort();
                Vec::new()
            }
        };

        Ok(ExecOutput {
            stdout,
            stderr,
            exit_code,
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
        // A missing path is `None`, not an error — otherwise `list_skills`
        // fails outright when the skills directory has been deleted/reset
        // (audit S12).
        let metadata = match tokio::fs::metadata(path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(WorkspaceError::BackendError {
                    message: format!("stat_mtime '{path}': {e}"),
                });
            }
        };
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
