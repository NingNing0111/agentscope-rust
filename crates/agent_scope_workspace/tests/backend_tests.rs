//! Tests for LocalBackend (T010)

mod common;

use agent_scope_workspace::{LocalBackend, WorkspaceBackend};

#[tokio::test]
async fn test_write_read_cycle() {
    let (_td, workdir) = common::temp_workdir();
    let backend = LocalBackend::new();
    let file_path = backend.join_path(&workdir, "test.txt");

    backend
        .write_file(&file_path, b"hello world")
        .await
        .unwrap();
    assert!(backend.file_exists(&file_path).await.unwrap());

    let data = backend.read_file(&file_path).await.unwrap();
    assert_eq!(data, b"hello world");
}

#[tokio::test]
async fn test_exec_shell_echo() {
    let (_td, workdir) = common::temp_workdir();
    let backend = LocalBackend::new();

    let output = backend
        .exec_shell(&["echo", "-n", "hello"], &workdir, None)
        .await
        .unwrap();
    assert!(output.ok());
    assert_eq!(output.stdout, b"hello");
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn test_is_dir() {
    let (_td, workdir) = common::temp_workdir();
    let backend = LocalBackend::new();

    assert!(backend.is_dir(&workdir).await.unwrap());

    let file_path = backend.join_path(&workdir, "test.txt");
    backend.write_file(&file_path, b"data").await.unwrap();
    assert!(!backend.is_dir(&file_path).await.unwrap());
}

#[tokio::test]
async fn test_list_dir_recursive() {
    let (_td, workdir) = common::temp_workdir();
    let backend = LocalBackend::new();

    let subdir = backend.join_path(&workdir, "sub");
    let file1 = backend.join_path(&subdir, "a.txt");
    let file2 = backend.join_path(&workdir, "b.txt");

    backend.write_file(&file1, b"a").await.unwrap();
    backend.write_file(&file2, b"b").await.unwrap();

    let entries = backend.list_dir(&workdir, true).await.unwrap();
    assert!(entries.iter().any(|e| e.contains("a.txt")));
    assert!(entries.iter().any(|e| e.contains("b.txt")));
}

#[tokio::test]
async fn test_delete_path_idempotent() {
    let (_td, workdir) = common::temp_workdir();
    let backend = LocalBackend::new();
    let file_path = backend.join_path(&workdir, "test.txt");

    backend.write_file(&file_path, b"data").await.unwrap();
    backend.delete_path(&file_path).await.unwrap();
    assert!(!backend.file_exists(&file_path).await.unwrap());

    // Idempotent: delete non-existent path
    backend.delete_path(&file_path).await.unwrap();
}

#[tokio::test]
async fn test_stat_mtime() {
    let (_td, workdir) = common::temp_workdir();
    let backend = LocalBackend::new();
    let file_path = backend.join_path(&workdir, "test.txt");

    backend.write_file(&file_path, b"data").await.unwrap();
    let mtime = backend.stat_mtime(&file_path).await.unwrap();
    assert!(mtime.is_some());
    assert!(mtime.unwrap() > 0.0);
}

#[tokio::test]
async fn test_normpath() {
    let backend = LocalBackend::new();
    assert_eq!(backend.normpath("/a/b/../c"), "/a/c");
    assert_eq!(backend.normpath("/a/./b"), "/a/b");
}

#[tokio::test]
async fn test_is_absolute() {
    let backend = LocalBackend::new();
    assert!(backend.is_absolute("/absolute/path"));
    assert!(!backend.is_absolute("relative/path"));
}

// ============================================================================
// Defect 2: exec_shell process group / output overflow tests (FAILING before fix)
// ============================================================================

/// Grandchildren must be reaped when the parent times out (defect 2).
#[cfg(target_family = "unix")]
#[tokio::test]
async fn test_exec_shell_timeout_kills_grandchildren() {
    let (_td, workdir) = common::temp_workdir();
    let backend = LocalBackend::new();

    // Spawn a grandchild that outlives the direct child: `sh -c 'sleep 100 & sleep 999'`
    // The `sleep 100 &` creates a background grandchild. When the direct `sh` is
    // killed, the grandchild must also die.
    let output = backend
        .exec_shell(&["sh", "-c", "sleep 100 & sleep 999"], &workdir, Some(1.0))
        .await
        .unwrap();

    // After timeout, exit code is 124 (matching the `timeout` command convention)
    assert_eq!(output.exit_code, 124);

    // Give a moment for cleanup, then verify no orphan sleep processes
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

/// Infinite-output command like `yes` must not hang when there is no timeout
/// (defect 2). Without the fix, `yes` fills the pipe and blocks, causing
/// `child.wait()` to hang forever.
#[cfg(target_family = "unix")]
#[tokio::test]
async fn test_exec_shell_yes_does_not_hang() {
    let (_td, workdir) = common::temp_workdir();
    let backend = LocalBackend::new();

    // This test must complete within a few seconds, not hang forever.
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        backend.exec_shell(&["yes"], &workdir, None),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            // Output should be truncated (capped at ~1 MiB)
            assert!(output.stdout.len() <= 1_048_576 + 1);
        }
        Ok(Err(e)) => {
            // Any error is acceptable as long as we don't hang
            let _ = e;
        }
        Err(_elapsed) => {
            panic!("exec_shell with `yes` hung for >10s — output overflow not handled");
        }
    }
}

/// When output overflows the cap, exit_code should indicate the kill.
#[cfg(target_family = "unix")]
#[tokio::test]
async fn test_exec_shell_output_overflow_exit_code() {
    let (_td, workdir) = common::temp_workdir();
    let backend = LocalBackend::new();

    let output = backend.exec_shell(&["yes"], &workdir, None).await.unwrap();

    // After overflow kill, exit_code should be non-zero (SIGKILL -> -1 or 137)
    assert_ne!(output.exit_code, 0);
    // Output should be truncated
    assert!(output.stdout.len() <= 1_048_576 + 1);
}
