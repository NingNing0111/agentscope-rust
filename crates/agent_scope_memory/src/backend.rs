//! Storage backend abstraction — defines the [`Backend`] trait for pluggable filesystem
//! operations and provides a built-in [`LocalBackend`] implementation.

use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::MemoryError;

/// Monotonic counter for unique temp-file names in [`LocalBackend::write_file`].
static TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[async_trait::async_trait]
pub trait Backend: Send + Sync {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, MemoryError>;
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), MemoryError>;
    async fn delete_file(&self, path: &str) -> Result<(), MemoryError>;
    async fn file_exists(&self, path: &str) -> Result<bool, MemoryError>;
    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, MemoryError>;
    fn join_path(&self, a: &str, b: &str) -> String;
    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, MemoryError>;
    fn normpath(&self, path: &str) -> String;
    fn isabs(&self, path: &str) -> bool;
}

#[derive(Debug, Default)]
pub struct LocalBackend;

impl LocalBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Backend for LocalBackend {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, MemoryError> {
        tokio::fs::read(path)
            .await
            .map_err(|err| io_error(path, err))
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), MemoryError> {
        if let Some(parent) = Path::new(path).parent() {
            let parent = parent.to_path_buf();
            tokio::task::spawn_blocking(move || std::fs::create_dir_all(parent))
                .await
                .map_err(|err| MemoryError::IoError {
                    path: path.to_string(),
                    message: err.to_string(),
                })?
                .map_err(|err| io_error(path, err))?;
        }
        // Atomic write: a process crash mid-write must not leave a truncated
        // .md / index file that the frontmatter parser treats as corrupted.
        // Write to a unique temp file, then rename (atomic on POSIX) so readers
        // always see either the old or the new complete file (audit M4). No
        // fsync here — rename-atomicity covers process crashes and keeping this
        // on the hot write path matters (bulk index generation).
        let unique = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp_path = format!("{path}.tmp-{}-{unique}", std::process::id());
        tokio::fs::write(&tmp_path, data)
            .await
            .map_err(|err| io_error(path, err))?;
        if let Err(err) = tokio::fs::rename(&tmp_path, path).await {
            // Don't leave a stray temp file behind on failure.
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(io_error(path, err));
        }
        Ok(())
    }

    async fn delete_file(&self, path: &str) -> Result<(), MemoryError> {
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(io_error(path, err)),
        }
    }

    async fn file_exists(&self, path: &str) -> Result<bool, MemoryError> {
        tokio::fs::try_exists(path)
            .await
            .map_err(|err| io_error(path, err))
    }

    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, MemoryError> {
        if !self.file_exists(path).await? {
            return Ok(Vec::new());
        }
        if recursive {
            let root = PathBuf::from(path);
            tokio::task::spawn_blocking(move || list_recursive(&root))
                .await
                .map_err(|err| MemoryError::IoError {
                    path: path.to_string(),
                    message: err.to_string(),
                })?
                .map_err(|err| io_error(path, err))
        } else {
            let mut entries = tokio::fs::read_dir(path)
                .await
                .map_err(|err| io_error(path, err))?;
            let mut result = Vec::new();
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|err| io_error(path, err))?
            {
                result.push(entry.path().to_string_lossy().into_owned());
            }
            Ok(result)
        }
    }

    fn join_path(&self, a: &str, b: &str) -> String {
        Path::new(a).join(b).to_string_lossy().into_owned()
    }

    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, MemoryError> {
        let metadata = match tokio::fs::metadata(path).await {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(io_error(path, err)),
        };
        let modified = metadata.modified().map_err(|err| io_error(path, err))?;
        let duration = modified
            .duration_since(UNIX_EPOCH)
            .map_err(|err| MemoryError::IoError {
                path: path.to_string(),
                message: err.to_string(),
            })?;
        Ok(Some(duration.as_secs_f64()))
    }

    fn normpath(&self, path: &str) -> String {
        lexical_clean(Path::new(path))
            .to_string_lossy()
            .into_owned()
    }

    fn isabs(&self, path: &str) -> bool {
        Path::new(path).is_absolute()
    }
}

fn io_error(path: &str, err: std::io::Error) -> MemoryError {
    MemoryError::IoError {
        path: path.to_string(),
        message: err.to_string(),
    }
}

fn list_recursive(root: &Path) -> std::io::Result<Vec<String>> {
    let mut result = Vec::new();
    if !root.exists() {
        return Ok(result);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path.clone());
            }
            result.push(path.to_string_lossy().into_owned());
        }
    }
    Ok(result)
}

fn lexical_clean(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_backend_file_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalBackend::new();
        let path = dir.path().join("a/b.txt").to_string_lossy().into_owned();
        backend.write_file(&path, b"hello").await.unwrap();
        assert!(backend.file_exists(&path).await.unwrap());
        assert_eq!(backend.read_file(&path).await.unwrap(), b"hello");
        assert!(backend.stat_mtime(&path).await.unwrap().is_some());
        backend.delete_file(&path).await.unwrap();
        assert!(!backend.file_exists(&path).await.unwrap());
    }

    #[tokio::test]
    async fn list_dir_includes_subdirs_for_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalBackend::new();
        let root = dir.path().to_string_lossy().into_owned();
        backend
            .write_file(&backend.join_path(&root, "sub/file.md"), b"x")
            .await
            .unwrap();
        let recursive = backend.list_dir(&root, true).await.unwrap();
        assert!(recursive.iter().any(|p| p.ends_with("file.md")));
    }

    #[test]
    fn path_helpers_work() {
        let backend = LocalBackend::new();
        assert!(
            backend.join_path("a", "b").ends_with("a/b")
                || backend.join_path("a", "b").ends_with("a\\b")
        );
        assert_eq!(backend.normpath("a/./b/../c"), "a/c");
        assert!(backend.isabs("/tmp") || cfg!(windows));
    }
}
