//! Tests for workspace lifecycle and reset (US4: T049-T051)

mod common;

use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

#[tokio::test]
async fn test_close_sets_is_alive_false() {
    let (_td, workdir) = common::temp_workdir();
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

    ws.close().await.unwrap();
    assert!(!ws.is_alive());
}

#[tokio::test]
async fn test_close_on_closed_is_idempotent() {
    let (_td, workdir) = common::temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    ws.close().await.unwrap();
    ws.close().await.unwrap(); // no-op
    assert!(!ws.is_alive());
}

#[tokio::test]
async fn test_reset_clears_all() {
    let (_td, workdir) = common::temp_workdir();

    // Create a skill for testing
    let (_skill_td, skill_dir) = common::temp_workdir();
    std::fs::write(
        std::path::Path::new(&skill_dir).join("SKILL.md"),
        "---\nname: reset-skill\ndescription: Test\n---\n\nContent\n",
    )
    .unwrap();

    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    // Add some content
    ws.add_skill(&skill_dir).await.unwrap();

    // Reset
    ws.reset().await.unwrap();

    // Skills should be empty
    let skills = ws.list_skills().await.unwrap();
    assert!(skills.is_empty());

    // MCPs should be empty
    let mcps = ws.list_mcps().await.unwrap();
    assert!(mcps.is_empty());

    // Directories should exist but be empty (only .keep)
    let backend = ws.get_backend().unwrap();
    let skills_dir = backend.join_path(&workdir, "skills");
    let sessions_dir = backend.join_path(&workdir, "sessions");
    let data_dir = backend.join_path(&workdir, "data");

    assert!(backend.is_dir(&skills_dir).await.unwrap());
    assert!(backend.is_dir(&sessions_dir).await.unwrap());
    assert!(backend.is_dir(&data_dir).await.unwrap());
}

#[tokio::test]
async fn test_re_initialize_after_close() {
    let (_td, workdir) = common::temp_workdir();
    let config = LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();
    ws.close().await.unwrap();

    // Re-initialize — should succeed
    ws.initialize().await.unwrap();
    assert!(ws.is_alive());
}

#[tokio::test]
async fn test_reset_no_reseed_defaults() {
    let (_td, workdir) = common::temp_workdir();

    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();
    ws.reset().await.unwrap();

    let mcps = ws.list_mcps().await.unwrap();
    assert!(mcps.is_empty());
}
