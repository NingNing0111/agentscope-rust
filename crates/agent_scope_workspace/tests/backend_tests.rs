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
