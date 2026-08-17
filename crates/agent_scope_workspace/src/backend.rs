//! Workspace backend trait + LocalBackend implementation.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::error::WorkspaceError;

/// Upper bound on the bytes read from a shell command's stdout/stderr, so an
/// unbounded producer (e.g. `yes`) cannot exhaust memory. Reading past this
/// limit truncates the captured output.
const MAX_SHELL_OUTPUT_BYTES: usize = 1_048_576; // 1 MiB

/// Upper bound for a single `read_file` call, so an agent-created giant file
/// cannot be loaded wholly into memory (round-4 M29).
const MAX_READ_FILE_BYTES: usize = 10 * 1024 * 1024; // 10 MiB

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
// ContainedBackend — enforces workdir containment for any inner backend
// ---------------------------------------------------------------------------

/// A backend wrapper that restricts all file/shell operations to a root
/// directory. Absolute paths, `..` traversals, and symlink escapes are
/// rejected.
///
/// Path-only helpers (join, basename, dirname, normpath, is_absolute) are
/// forwarded to the inner backend unchanged.
pub struct ContainedBackend {
    inner: Arc<dyn WorkspaceBackend>,
    root: PathBuf,
}

impl ContainedBackend {
    /// Wrap `inner` so every path operation is confined within `root`.
    #[must_use]
    pub fn new(inner: Arc<dyn WorkspaceBackend>, root: PathBuf) -> Self {
        Self { inner, root }
    }

    /// Canonicalize `path` (joining with root if relative), then check it
    /// starts with `self.root`. Returns the contained path or an error.
    fn contain(&self, path: &str) -> Result<String, WorkspaceError> {
        self.contain_path(path, true)
    }

    /// Same as `contain` but allows paths that don't yet exist by resolving
    /// through the nearest existing ancestor.
    fn contain_for_write(&self, path: &str) -> Result<String, WorkspaceError> {
        self.contain_path(path, false)
    }

    /// Internal implementation.
    fn contain_path(&self, path: &str, must_exist: bool) -> Result<String, WorkspaceError> {
        // 1. Reject paths with `..` components before any resolution
        if path_has_parent_component(path) {
            return Err(WorkspaceError::PathTraversal {
                path: path.to_string(),
            });
        }

        let p = Path::new(path);
        let joined = if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.root.join(p)
        };

        if must_exist {
            // Path must exist (e.g. read, is_dir) — canonicalize whole path
            let canon = joined
                .canonicalize()
                .map_err(|e| WorkspaceError::BackendError {
                    message: format!("path containment canonicalize '{path}': {e}"),
                })?;
            if !canon.starts_with(&self.root) {
                return Err(WorkspaceError::PathTraversal {
                    path: canon.display().to_string(),
                });
            }
            Ok(canon.to_string_lossy().to_string())
        } else {
            // Path may not exist yet (e.g. write, exec_shell cwd) — walk up
            // to the nearest existing ancestor, canonicalize that, then
            // re-join the non-existent tail.
            let mut existing = joined.clone();
            let mut missing: Vec<std::ffi::OsString> = Vec::new();
            while !existing.exists() {
                let Some(name) = existing.file_name() else {
                    break;
                };
                missing.push(name.to_os_string());
                let Some(up) = existing.parent() else {
                    break;
                };
                existing = up.to_path_buf();
            }
            let canon_existing =
                existing
                    .canonicalize()
                    .map_err(|e| WorkspaceError::BackendError {
                        message: format!("path containment canonicalize parent of '{path}': {e}"),
                    })?;
            if !canon_existing.starts_with(&self.root) {
                return Err(WorkspaceError::PathTraversal {
                    path: canon_existing.display().to_string(),
                });
            }
            let mut result = canon_existing.clone();
            for comp in missing.iter().rev() {
                result = result.join(comp);
            }
            // Final symlink check: if the resolved leaf exists as a symlink,
            // resolve and verify containment
            if let Ok(meta) = std::fs::symlink_metadata(&result)
                && meta.file_type().is_symlink()
            {
                let canon = result
                    .canonicalize()
                    .map_err(|e| WorkspaceError::BackendError {
                        message: format!("path containment symlink resolve '{path}': {e}"),
                    })?;
                if !canon.starts_with(&self.root) {
                    return Err(WorkspaceError::PathTraversal {
                        path: canon.display().to_string(),
                    });
                }
                return Ok(canon.to_string_lossy().to_string());
            }
            // Verify no component in the missing tail is a symlink escape
            let start = canon_existing.clone();
            let mut walk = start;
            for comp in missing.iter().rev() {
                walk = walk.join(comp);
                if walk.exists()
                    && let Ok(meta) = std::fs::symlink_metadata(&walk)
                    && meta.file_type().is_symlink()
                {
                    let resolved =
                        walk.canonicalize()
                            .map_err(|e| WorkspaceError::BackendError {
                                message: format!("path containment symlink in path '{path}': {e}"),
                            })?;
                    if !resolved.starts_with(&self.root) {
                        return Err(WorkspaceError::PathTraversal {
                            path: resolved.display().to_string(),
                        });
                    }
                }
            }
            Ok(result.to_string_lossy().to_string())
        }
    }
}

fn path_has_parent_component(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|c| matches!(c, Component::ParentDir))
}

#[async_trait::async_trait]
impl WorkspaceBackend for ContainedBackend {
    async fn exec_shell(
        &self,
        cmd: &[&str],
        cwd: &str,
        timeout_secs: Option<f64>,
    ) -> Result<ExecOutput, WorkspaceError> {
        // cwd must exist (we're about to run a command there)
        let contained_cwd = self.contain(cwd)?;
        self.inner
            .exec_shell(cmd, &contained_cwd, timeout_secs)
            .await
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, WorkspaceError> {
        let contained = self.contain(path)?;
        self.inner.read_file(&contained).await
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), WorkspaceError> {
        let contained = self.contain_for_write(path)?;
        self.inner.write_file(&contained, data).await
    }

    async fn is_dir(&self, path: &str) -> Result<bool, WorkspaceError> {
        // is_dir must work for non-existent paths (returns false)
        let contained = self.contain_for_write(path)?;
        self.inner.is_dir(&contained).await
    }

    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, WorkspaceError> {
        // list_dir on a non-existent path should return empty vec
        let contained = self.contain_for_write(path)?;
        self.inner.list_dir(&contained, recursive).await
    }

    async fn delete_path(&self, path: &str) -> Result<(), WorkspaceError> {
        // Check containment first — if the caller tries to escape, reject.
        // If the path doesn't exist but is within root, the inner backend
        // handles idempotency. Use contain (must_exist=false implicitly via
        // contain_for_write since we permit deleting non-existent paths).
        let contained = self.contain_for_write(path)?;
        self.inner.delete_path(&contained).await
    }

    async fn file_exists(&self, path: &str) -> Result<bool, WorkspaceError> {
        // file_exists must work for non-existent paths (returns false).
        let contained = self.contain_for_write(path)?;
        self.inner.file_exists(&contained).await
    }

    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, WorkspaceError> {
        let contained = self.contain_for_write(path)?;
        self.inner.stat_mtime(&contained).await
    }

    // Pure path helpers — forwarded unchanged
    fn join_path(&self, a: &str, b: &str) -> String {
        self.inner.join_path(a, b)
    }
    fn basename(&self, path: &str) -> String {
        self.inner.basename(path)
    }
    fn dirname(&self, path: &str) -> String {
        self.inner.dirname(path)
    }
    fn normpath(&self, path: &str) -> String {
        self.inner.normpath(path)
    }
    fn is_absolute(&self, path: &str) -> bool {
        self.inner.is_absolute(path)
    }
}

// ---------------------------------------------------------------------------
// Process group helpers (defect 2 fix)
// ---------------------------------------------------------------------------

/// Kill the process group led by `child_pid`. The child is spawned with
/// `setsid()` so it becomes the session leader; signalling the negative pid
/// reaps the direct child *and* any grandchildren that inherited its
/// stdout/stderr pipes (e.g. `sh -c 'sleep 1000 &'`), which would otherwise
/// survive the direct child, keep the pipes open, and leak.
/// Best-effort: a missing/unknown pid or a group that no longer exists is
/// silently ignored.
#[cfg(unix)]
fn kill_process_group(child_pid: Option<u32>) {
    if let Some(pid) = child_pid {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;
        let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_process_group(_child_pid: Option<u32>) {}

// ---------------------------------------------------------------------------
// LocalBackend
// ---------------------------------------------------------------------------

/// Filesystem backend that delegates to `tokio::fs` and `tokio::process`.
///
/// NOTE: `LocalBackend` itself does **not** enforce workdir containment.
/// For a contained execution environment use `ContainedBackend` wrapping a
/// `LocalBackend` — this is what `LocalWorkspace` does internally so that
/// all `WorkspaceBase::get_backend()` callers automatically benefit from
/// path isolation.
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

        let mut command = tokio::process::Command::new(cmd[0]);
        command
            .args(&cmd[1..])
            .current_dir(cwd)
            .env_clear()
            .env(
                "PATH",
                std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".into()),
            )
            .env("HOME", cwd)
            .env("TMPDIR", std::env::temp_dir())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        // Put the child in its own process group so we can later reap
        // grandchildren via killpg (defect 2 fix).
        #[cfg(unix)]
        {
            command.process_group(0);
        }

        let mut child = command.spawn().map_err(|e| WorkspaceError::BackendError {
            message: format!("failed to spawn command '{}': {e}", cmd[0]),
        })?;

        let child_pid = child.id();

        // Spawn stdout/stderr read tasks. Reads are bounded so an unbounded
        // producer cannot exhaust memory.
        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();
        let stdout_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            if let Some(s) = stdout_pipe {
                use tokio::io::AsyncReadExt;
                s.take((MAX_SHELL_OUTPUT_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .await?;
            }
            Ok::<Vec<u8>, std::io::Error>(bytes)
        });
        let stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            if let Some(s) = stderr_pipe {
                use tokio::io::AsyncReadExt;
                s.take((MAX_SHELL_OUTPUT_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .await?;
            }
            Ok::<Vec<u8>, std::io::Error>(bytes)
        });

        // Bundle reads into a single future so we never hold `&mut JoinHandle`
        // across a `tokio::select!` boundary (which can cause the dreaded
        // "JoinHandle polled after completion" panic when a branch wins).
        // The inner function flattens errors to Option<Vec<u8>> for simplicity.
        let mut reads_done = tokio::spawn(async move {
            let out = match stdout_task.await {
                Ok(Ok(data)) => data,
                _ => Vec::new(),
            };
            let err = match stderr_task.await {
                Ok(Ok(data)) => data,
                _ => Vec::new(),
            };
            (out, err)
        });

        // Build optional timeout future.
        let explicit_timeout = timeout_secs.filter(|s| *s > 0.0);

        // RACE: child.wait() vs reads-completion vs timeout.
        //
        // This fixes two defect-2 problems:
        //   (a) `yes` overflow: reads finish at 1 MiB cap, child blocks
        //       writing to full pipe — child.wait() hangs forever.
        //   (b) Timeout: kill the entire session, not just the direct child.
        let exit_code: i32;
        let stdout: Vec<u8>;
        let stderr: Vec<u8>;

        tokio::select! {
            // Case 1: Child exited on its own (normal completion)
            status = child.wait() => {
                exit_code = status
                    .map_err(|e| WorkspaceError::BackendError {
                        message: format!("failed to wait on command '{}': {e}", cmd[0]),
                    })?
                    .code()
                    .unwrap_or(-1);
                // Child has exited, so its pipes are closed. Join reads with a
                // generous timeout (reads should complete quickly now).
                let read_timeout = explicit_timeout
                    .map(std::time::Duration::from_secs_f64)
                    .unwrap_or(std::time::Duration::from_secs(300));
                let read_result = tokio::time::timeout(read_timeout, &mut reads_done).await;
                match read_result {
                    Ok(Ok((out, err))) => {
                        stdout = out;
                        stderr = err;
                    }
                    _ => {
                        // Reads timed out after child exit — unusual, capture nothing
                        reads_done.abort();
                        stdout = Vec::new();
                        stderr = Vec::new();
                    }
                }
            }

            // Case 2: Explicit timeout fired
            _ = async {
                if let Some(secs) = explicit_timeout {
                    tokio::time::sleep(std::time::Duration::from_secs_f64(secs)).await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                kill_process_group(child_pid);
                let _ = child.kill().await;
                let _ = child.wait().await;
                exit_code = 124;
                reads_done.abort();
                stdout = Vec::new();
                stderr = Vec::new();
            }

            // Case 3: Reads completed before child exited (output overflow)
            read_result = &mut reads_done => {
                // Reads finished — get the output
                let (out, err) = match read_result {
                    Ok((out, err)) => (out, err),
                    Err(_) => (Vec::new(), Vec::new()),
                };
                stdout = out;
                stderr = err;

                // Did the child also exit?
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Child exited normally (reads just caught up)
                        exit_code = status.code().unwrap_or(-1);
                    }
                    Ok(None) => {
                        // Child still running, blocked on pipe — overflow kill
                        kill_process_group(child_pid);
                        let _ = child.kill().await;
                        if let Ok(status) = child.wait().await {
                            exit_code = status.code().unwrap_or(-1);
                        } else {
                            exit_code = -1;
                        }
                    }
                    Err(_) => {
                        exit_code = -1;
                    }
                }
            }
        }

        Ok(ExecOutput {
            stdout,
            stderr,
            exit_code,
        })
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, WorkspaceError> {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| WorkspaceError::BackendError {
                message: format!("read_file '{path}': {e}"),
            })?;
        let meta = file
            .metadata()
            .await
            .map_err(|e| WorkspaceError::BackendError {
                message: format!("read_file '{path}': {e}"),
            })?;
        // Cap the read so an agent-created giant file cannot be loaded wholly
        // into memory (OOM) — mirrors the sandbox `read_file` cap (round-4 M29).
        if meta.len() > MAX_READ_FILE_BYTES as u64 {
            return Err(WorkspaceError::BackendError {
                message: format!(
                    "read_file '{path}': {} bytes exceeds the {} byte cap",
                    meta.len(),
                    MAX_READ_FILE_BYTES
                ),
            });
        }
        use tokio::io::AsyncReadExt;
        let mut buf = Vec::with_capacity(meta.len() as usize);
        file.take((MAX_READ_FILE_BYTES + 1) as u64)
            .read_to_end(&mut buf)
            .await
            .map_err(|e| WorkspaceError::BackendError {
                message: format!("read_file '{path}': {e}"),
            })?;
        Ok(buf)
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
        // Normalize the separator to `/` so the canonical form is identical on
        // every platform (Windows PathBuf renders `\`). Mirrors the memory
        // backend contract; `\` is a valid filename byte on Unix so only
        // rewrite on Windows.
        let display = buf.to_string_lossy();
        #[cfg(windows)]
        {
            display.replace('\\', "/")
        }
        #[cfg(not(windows))]
        {
            display.into_owned()
        }
    }

    fn is_absolute(&self, path: &str) -> bool {
        let p = Path::new(path);
        // A leading root (`/foo`) is absolute on every platform. On Windows
        // `Path::is_absolute()` additionally requires a drive/UNC prefix, so a
        // POSIX-style `/nested/...` path would otherwise read as relative —
        // same normalization the sandbox resolver applies.
        p.is_absolute() || matches!(p.components().next(), Some(Component::RootDir))
    }
}
