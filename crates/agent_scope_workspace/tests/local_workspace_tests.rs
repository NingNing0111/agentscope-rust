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
    assert!(instructions.contains(&workdir));
    assert!(instructions.contains("LocalBackend"));
}
