# Contract: WorkspaceBackend Trait

**Feature**: 012-workspace-management | **Status**: Draft

## Interface

```rust
/// Abstract filesystem and process-I/O backend for Workspace.
///
/// Implementations: `LocalBackend` (local filesystem), future Docker/E2B/K8s.
#[async_trait::async_trait]
pub trait WorkspaceBackend: Send + Sync {
    /// Execute a shell command. `cmd` is the argv array.
    /// `cwd` is the working directory for the command (absolute path).
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
    /// When `recursive` is true, walks all subdirectories.
    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, WorkspaceError>;

    /// Delete a file or recursively delete a directory.
    /// Idempotent: no error if path does not exist.
    async fn delete_path(&self, path: &str) -> Result<(), WorkspaceError>;

    /// Check whether a file or directory exists.
    async fn file_exists(&self, path: &str) -> Result<bool, WorkspaceError>;

    /// Join two path components. Synchronous (pure string operation).
    fn join_path(&self, a: &str, b: &str) -> String;

    /// Return the last component of a path (filename or dirname).
    fn basename(&self, path: &str) -> String;

    /// Return the parent directory of a path.
    fn dirname(&self, path: &str) -> String;

    /// Get modification time as Unix timestamp (seconds since epoch).
    /// Returns `None` if the file does not exist.
    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, WorkspaceError>;

    /// Normalize a path (remove `.` and resolve `..`).
    fn normpath(&self, path: &str) -> String;

    /// Check if a path is absolute.
    fn is_absolute(&self, path: &str) -> bool;
}

/// Result of a shell command execution.
#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

impl ExecOutput {
    /// True if exit_code == 0.
    pub fn ok(&self) -> bool {
        self.exit_code == 0
    }
}
```

## Contract Guarantees

| Guarantee | Detail |
|-----------|--------|
| Thread safety | `Send + Sync` in trait bound |
| No panics | All I/O errors returned as `WorkspaceError::BackendError` |
| Parent creation | `write_file` auto-creates parent directories |
| Idempotent delete | `delete_path` returns `Ok(())` for non-existent paths |
| Idempotent dir list | `list_dir` with non-existent path returns empty `Vec` |
| Path format | All path arguments and return values are absolute or workspace-relative |
| Timeout support | `exec_shell` accepts optional timeout; kills process on timeout |

## Cross-reference

- Python: `BaseBackend` in `agentscope/src/agentscope/tool/_builtin/_backend.py`
- Existing: `agent_scope_memory::Backend` (partial subset, no shell/delete/is_dir/basename/dirname)
