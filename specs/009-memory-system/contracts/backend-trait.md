# Contract: Backend Trait

**Feature**: 009-memory-system  
**Contract Type**: Rust trait interface  
**Stability**: New (may evolve)

## Trait Definition

```rust
#[async_trait::async_trait]
pub trait Backend: Send + Sync {
    /// Read entire file as bytes.
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, MemoryError>;

    /// Write bytes to file (create or overwrite). Creates parent directories.
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), MemoryError>;

    /// Delete a file. No-op if file does not exist.
    async fn delete_file(&self, path: &str) -> Result<(), MemoryError>;

    /// Check if a file exists at path.
    async fn file_exists(&self, path: &str) -> Result<bool, MemoryError>;

    /// List entries in a directory, optionally recursive.
    /// Returns fully qualified paths.
    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, MemoryError>;

    /// Join two path components with the backend's separator.
    fn join_path(&self, a: &str, b: &str) -> String;

    /// Get file modification time as Unix timestamp (seconds since epoch).
    /// Returns None if mtime is unavailable.
    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, MemoryError>;

    /// Normalize a path (resolve `.` and `..`).
    fn normpath(&self, path: &str) -> String;

    /// Check if a path is absolute.
    fn isabs(&self, path: &str) -> bool;
}
```

## LocalBackend

```rust
pub struct LocalBackend;

impl LocalBackend {
    pub fn new() -> Self;
}
```

Implementation uses `tokio::fs`:
- `read_file` → `tokio::fs::read`
- `write_file` → `tokio::fs::write` (parent dirs created via `std::fs::create_dir_all` spawn_blocking)
- `delete_file` → `tokio::fs::remove_file`
- `file_exists` → `tokio::fs::try_exists`
- `list_dir` → recursive `tokio::fs::read_dir` if `recursive=true`, else non-recursive
- `join_path` → `std::path::Path::join`
- `stat_mtime` → `tokio::fs::metadata` → `.modified()?.duration_since(UNIX_EPOCH)`
- `normpath` → `std::path::Path::canonicalize` equivalent (lexical normalization via `path-clean` or manual `.`/`..` resolution)
- `isabs` → `std::path::Path::is_absolute`

## Error Handling

All backend methods return `MemoryError::IoError { path, message }` on I/O failures. The `path` field captures the failing path for debugging. The `message` field contains the OS error string.

## Concurrency

`LocalBackend` methods are inherently thread-safe (tokio::fs operations). No internal locking needed — the OS filesystem provides consistency.

## Future Remote Backends

The `Backend` trait is designed for future implementations:
- `S3Backend` — object storage via `rusoto`/`aws-sdk`
- `MCPBackend` — sandboxed file access via MCP protocol
- `MemoryBackend` — in-memory for testing
