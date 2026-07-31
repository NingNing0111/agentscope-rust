//! Tests for WorkspaceManager (US5: T054)

use std::time::Duration;

use agent_scope_workspace::{LocalWorkspaceConfig, WorkspaceManager};

#[tokio::test]
async fn test_manager_get_creates_workspace() {
    let manager = WorkspaceManager::new(None, |key| LocalWorkspaceConfig {
        workdir: format!("/tmp/ws-test-{key}"),
        workspace_id: Some(key.clone()),
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    });

    let ws = manager.get("user-a").await.unwrap();
    assert!(ws.is_alive());
}

#[tokio::test]
async fn test_manager_same_key_returns_same_instance() {
    let manager = WorkspaceManager::new(None, |key| LocalWorkspaceConfig {
        workdir: format!("/tmp/ws-same-{key}"),
        workspace_id: Some(key.clone()),
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    });

    let ws1 = manager.get("user-x").await.unwrap();
    let ws2 = manager.get("user-x").await.unwrap();
    assert_eq!(ws1.workspace_id(), ws2.workspace_id());
}

#[tokio::test]
async fn test_manager_different_keys_different_instances() {
    let manager = WorkspaceManager::new(None, |key| LocalWorkspaceConfig {
        workdir: format!("/tmp/ws-diff-{key}"),
        workspace_id: Some(key.clone()),
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    });

    let ws_a = manager.get("user-a").await.unwrap();
    let ws_b = manager.get("user-b").await.unwrap();
    assert_ne!(ws_a.workspace_id(), ws_b.workspace_id());
    assert_ne!(ws_a.workdir(), ws_b.workdir());
}

#[tokio::test]
async fn test_manager_no_ttl_never_evicts() {
    let manager = WorkspaceManager::new(None, |key| LocalWorkspaceConfig {
        workdir: format!("/tmp/ws-ttl-{key}"),
        workspace_id: Some(key.clone()),
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    });

    manager.get("keeper").await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Workspace should still exist (no TTL)
    let ws = manager.get("keeper").await.unwrap();
    assert!(ws.is_alive());
}
