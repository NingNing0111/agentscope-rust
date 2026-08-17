//! Tests for LocalWorkspace (US1: T013-T015)

mod common;

use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};
use common::temp_workdir;

#[tokio::test]
async fn test_constructor_and_initialize() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: Some("test-ws-001".into()),
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);

    assert!(!ws.is_alive());
    assert_eq!(ws.workspace_id(), "test-ws-001");

    ws.initialize().await.unwrap();
    assert!(ws.is_alive());

    // Check subdirectories created
    let backend = ws.get_backend().unwrap();
    assert!(
        backend
            .is_dir(&backend.join_path(&workdir, "data"))
            .await
            .unwrap()
    );
    assert!(
        backend
            .is_dir(&backend.join_path(&workdir, "skills"))
            .await
            .unwrap()
    );
    assert!(
        backend
            .is_dir(&backend.join_path(&workdir, "sessions"))
            .await
            .unwrap()
    );
    assert!(
        backend
            .file_exists(&backend.join_path(&workdir, ".mcp"))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_double_init_is_noop() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);

    ws.initialize().await.unwrap();
    assert!(ws.is_alive());

    // Double init should be no-op
    ws.initialize().await.unwrap();
    assert!(ws.is_alive());
}

#[tokio::test]
async fn test_workdir_is_absolute() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let ws = LocalWorkspace::new(config);
    assert!(!ws.workdir().is_empty());
}

/// Regression: a workdir under a symlinked parent (e.g. macOS `/tmp` →
/// `/private/tmp`) whose leaf directory does not exist yet must still
/// initialize successfully. Before the fix, `LocalWorkspace::new` fell back
/// to the *un-canonicalized* workdir when the directory was missing, and the
/// backend containment check later rejected the canonicalized ancestor —
/// spurious `PathTraversal`.
#[tokio::test]
#[cfg(unix)]
async fn test_initialize_under_symlinked_parent_missing_leaf() {
    use std::os::unix::fs::symlink;

    let (_td, base) = temp_workdir();
    let real = std::path::Path::new(&base).join("real-dir");
    std::fs::create_dir_all(&real).unwrap();
    // `link` → `real-dir`; the workdir lives *through* the symlink, in a
    // subdirectory that does not exist yet.
    let link = std::path::Path::new(&base).join("link");
    symlink(&real, &link).unwrap();
    let workdir = link.join("brand-new-workspace");

    let config = LocalWorkspaceConfig {
        workdir: workdir.to_string_lossy().to_string(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize()
        .await
        .expect("initialize under symlinked parent must succeed");
    assert!(ws.is_alive());
}

#[tokio::test]
async fn test_list_tools_returns_six_tools() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let tools = ws.list_tools().await.unwrap();
    assert_eq!(tools.len(), 6);

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"Bash"));
    assert!(names.contains(&"Read"));
    assert!(names.contains(&"Write"));
    assert!(names.contains(&"Edit"));
    assert!(names.contains(&"Glob"));
    assert!(names.contains(&"Grep"));

    // Bash description includes workdir
    let bash = tools.iter().find(|t| t.name == "Bash").unwrap();
    assert!(bash.description.contains("shell"));
}

#[tokio::test]
async fn test_get_backend_on_uninitialized_fails() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let ws = LocalWorkspace::new(config);

    let result = ws.get_backend();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_backend_after_init() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let backend = ws.get_backend().unwrap();
    // Verify we can use the backend
    assert!(backend.is_absolute(ws.workdir()));
}

#[tokio::test]
async fn test_get_instructions() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let instructions = ws.get_instructions().await;
    // The instructions embed the workspace's canonical workdir. On Windows,
    // canonicalization resolves 8.3 short names and `\\?\` prefixes, so the
    // canonical form can differ from the raw config path — compare against
    // `ws.workdir()` rather than the un-canonicalized input.
    assert!(instructions.contains(ws.workdir()));
    assert!(instructions.contains("LocalBackend"));
}

// ============================================================================
// Defect 1: Path escape tests (FAILING before containment fix)
// ============================================================================

/// The backend exposed via `get_backend()` must refuse to read files outside
/// the workdir via absolute paths.
#[tokio::test]
async fn test_cannot_read_absolute_path_outside_workdir() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let backend = ws.get_backend().unwrap();
    // Attempt to read /etc/passwd — must fail
    let result = backend.read_file("/etc/passwd").await;
    assert!(
        result.is_err(),
        "read_file should reject absolute path outside workdir"
    );
}

/// The backend must refuse to read files via `..` escape from the workdir.
#[tokio::test]
async fn test_cannot_read_parent_traversal() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let backend = ws.get_backend().unwrap();
    let traversal = backend.join_path(&workdir, "../../etc/passwd");
    let result = backend.read_file(&traversal).await;
    assert!(
        result.is_err(),
        "read_file should reject path with `..` traversal"
    );
}

/// The backend must refuse to write files via absolute path outside workdir.
#[tokio::test]
async fn test_cannot_write_absolute_path_outside_workdir() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let backend = ws.get_backend().unwrap();
    let result = backend
        .write_file("/tmp/should-not-exist-escape-test", b"evil")
        .await;
    assert!(
        result.is_err(),
        "write_file should reject absolute path outside workdir"
    );
}

/// The backend must refuse to write via `..` traversal.
#[tokio::test]
async fn test_cannot_write_parent_traversal() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let backend = ws.get_backend().unwrap();
    let traversal = backend.join_path(&workdir, "../../tmp/escape-test");
    let result = backend.write_file(&traversal, b"evil").await;
    assert!(
        result.is_err(),
        "write_file should reject path with `..` traversal"
    );
}

/// The backend must refuse to delete paths outside workdir.
#[tokio::test]
async fn test_cannot_delete_absolute_path_outside_workdir() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let backend = ws.get_backend().unwrap();
    // Attempt to delete /etc — must fail (even if it doesn't exist, path escape is rejected)
    let result = backend
        .delete_path("/tmp/workspace-escape-delete-test")
        .await;
    assert!(
        result.is_err(),
        "delete_path should reject absolute path outside workdir"
    );
}

/// The backend must refuse to delete via `..` traversal.
#[tokio::test]
async fn test_cannot_delete_parent_traversal() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let backend = ws.get_backend().unwrap();
    let traversal = backend.join_path(&workdir, "../../tmp/escape-delete-test");
    let result = backend.delete_path(&traversal).await;
    assert!(
        result.is_err(),
        "delete_path should reject path with `..` traversal"
    );
}

/// The backend must refuse exec_shell with cwd outside workdir.
#[tokio::test]
async fn test_exec_shell_rejects_cwd_outside_workdir() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let backend = ws.get_backend().unwrap();
    let result = backend.exec_shell(&["echo", "hello"], "/etc", None).await;
    assert!(
        result.is_err(),
        "exec_shell should reject cwd outside workdir"
    );
}

/// File operations within the workdir must still work after containment.
#[tokio::test]
async fn test_contained_operations_still_work() {
    let (_td, workdir) = temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let backend = ws.get_backend().unwrap();
    let file_path = backend.join_path(&workdir, "test.txt");

    // Write within workdir should work
    backend.write_file(&file_path, b"hello").await.unwrap();
    assert!(backend.file_exists(&file_path).await.unwrap());

    // Read should work
    let data = backend.read_file(&file_path).await.unwrap();
    assert_eq!(data, b"hello");

    // Delete should work
    backend.delete_path(&file_path).await.unwrap();
    assert!(!backend.file_exists(&file_path).await.unwrap());
}
