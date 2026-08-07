//! Tests for workspace lifecycle and reset (US4: T049-T051)

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_scope_workspace::error::WorkspaceError;
use agent_scope_workspace::{
    LocalWorkspace, LocalWorkspaceConfig, McpConnectionHandle, McpConnectionsHost, WorkspaceBase,
};

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

// ─────────────────────────────────────────────────────────────────────────────
// MCP connection release on close()/reset() (US4: T038)
// ─────────────────────────────────────────────────────────────────────────────

/// A connection handle that records whether `disconnect()` was called.
#[derive(Debug)]
struct TrackingHandle {
    name: String,
    disconnected: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl McpConnectionHandle for TrackingHandle {
    fn name(&self) -> &str {
        &self.name
    }

    async fn disconnect(&self) -> Result<(), WorkspaceError> {
        self.disconnected.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// Register a fake live connection and assert `close()` releases it.
#[tokio::test]
async fn test_close_releases_mcp_connections() {
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

    let flag = Arc::new(AtomicBool::new(false));
    {
        let map = ws.mcp_connections();
        let mut guard = map.lock().await;
        guard.insert(
            "search".to_string(),
            Arc::new(TrackingHandle {
                name: "search".into(),
                disconnected: Arc::clone(&flag),
            }) as Arc<dyn McpConnectionHandle>,
        );
    }
    assert!(!ws.mcp_connections().lock().await.is_empty());

    ws.close().await.unwrap();

    assert!(
        ws.mcp_connections().lock().await.is_empty(),
        "close() must drain the MCP connections map"
    );
    assert!(
        flag.load(Ordering::SeqCst),
        "close() must call disconnect()"
    );
}

/// Register a fake live connection and assert `reset()` releases it.
#[tokio::test]
async fn test_reset_releases_mcp_connections() {
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

    let flag = Arc::new(AtomicBool::new(false));
    ws.mcp_connections().lock().await.insert(
        "search".to_string(),
        Arc::new(TrackingHandle {
            name: "search".into(),
            disconnected: Arc::clone(&flag),
        }) as Arc<dyn McpConnectionHandle>,
    );

    ws.reset().await.unwrap();

    assert!(
        ws.mcp_connections().lock().await.is_empty(),
        "reset() must drain the MCP connections map"
    );
    assert!(
        flag.load(Ordering::SeqCst),
        "reset() must call disconnect()"
    );
}
