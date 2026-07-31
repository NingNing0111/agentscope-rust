//! Test helpers for workspace tests.

use tempfile::TempDir;

/// Create a temporary directory for workspace tests.
/// Returns (TempDir, workdir_path).
pub fn temp_workdir() -> (TempDir, String) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let path = dir.path().to_string_lossy().to_string();
    (dir, path)
}
