//! Tests for MCP and Skill management (US2: T025-T026)

mod common;

use agent_scope_workspace::{
    LocalWorkspace, LocalWorkspaceConfig, McpClientConfig, McpTransportConfig, WorkspaceBase,
};
use common::temp_workdir;

// ---------- MCP Tests ----------

#[tokio::test]
async fn test_mcp_add_list_remove() {
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

    let mcp = McpClientConfig {
        name: "test-mcp".into(),
        transport: McpTransportConfig::Stdio {
            command: "echo".into(),
            args: vec!["hello".into()],
        },
        is_stateful: false,
    };

    // Add
    ws.add_mcp(mcp).await.unwrap();
    let mcps = ws.list_mcps().await.unwrap();
    assert_eq!(mcps.len(), 1);
    assert_eq!(mcps[0].name, "test-mcp");

    // Remove
    ws.remove_mcp("test-mcp").await.unwrap();
    let mcps = ws.list_mcps().await.unwrap();
    assert_eq!(mcps.len(), 0);
}

#[tokio::test]
async fn test_mcp_duplicate_name_error() {
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

    let mcp = McpClientConfig {
        name: "dup".into(),
        transport: McpTransportConfig::Stdio {
            command: "echo".into(),
            args: vec!["x".into()],
        },
        is_stateful: false,
    };

    ws.add_mcp(mcp.clone()).await.unwrap();
    let result = ws.add_mcp(mcp).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mcp_remove_unknown_warns() {
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

    // Should not error
    ws.remove_mcp("nonexistent").await.unwrap();
}

#[tokio::test]
async fn test_mcp_persist_to_file() {
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

    let mcp = McpClientConfig {
        name: "persisted".into(),
        transport: McpTransportConfig::Stdio {
            command: "true".into(),
            args: vec![],
        },
        is_stateful: false,
    };
    ws.add_mcp(mcp).await.unwrap();

    // Check .mcp file exists with content
    let backend = ws.get_backend().unwrap();
    let mcp_path = backend.join_path(ws.workdir(), ".mcp");
    let data = backend.read_file(&mcp_path).await.unwrap();
    let text = String::from_utf8_lossy(&data);
    assert!(text.contains("persisted"));
}

#[tokio::test]
async fn test_mcp_restore_on_reinit() {
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

    let mcp = McpClientConfig {
        name: "restored".into(),
        transport: McpTransportConfig::Stdio {
            command: "true".into(),
            args: vec![],
        },
        is_stateful: false,
    };
    ws.add_mcp(mcp).await.unwrap();
    ws.close().await.unwrap();

    // Re-initialize
    ws.initialize().await.unwrap();
    let mcps = ws.list_mcps().await.unwrap();
    assert_eq!(mcps.len(), 1);
    assert_eq!(mcps[0].name, "restored");
}

#[tokio::test]
async fn test_mcp_default_seed() {
    let (_td, workdir) = temp_workdir();
    let default_mcps = vec![McpClientConfig {
        name: "default-mcp".into(),
        transport: McpTransportConfig::Stdio {
            command: "echo".into(),
            args: vec!["default".into()],
        },
        is_stateful: false,
    }];
    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps,
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let mcps = ws.list_mcps().await.unwrap();
    assert_eq!(mcps.len(), 1);
    assert_eq!(mcps[0].name, "default-mcp");
}

// ---------- Skill Tests ----------

#[tokio::test]
async fn test_skill_add_valid_skill() {
    let (_td, workdir) = temp_workdir();

    // Create a temp skill dir
    let (_skill_td, skill_dir) = common::temp_workdir();
    let skill_md_path = std::path::Path::new(&skill_dir).join("SKILL.md");
    std::fs::write(
        &skill_md_path,
        "---\nname: test-skill\ndescription: A test skill\n---\n\n# Test Skill Content\n",
    )
    .unwrap();

    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    ws.add_skill(&skill_dir).await.unwrap();
    let skills = ws.list_skills().await.unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "test-skill");
    assert_eq!(skills[0].description, "A test skill");
}

#[tokio::test]
async fn test_skill_add_missing_skill_md_error() {
    let (_td, workdir) = temp_workdir();
    let (_empty_td, empty_dir) = common::temp_workdir();

    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    let result = ws.add_skill(&empty_dir).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_skill_add_duplicate_hash_skip() {
    let (_td, workdir) = temp_workdir();

    let (_skill_td, skill_dir) = common::temp_workdir();
    let skill_md_path = std::path::Path::new(&skill_dir).join("SKILL.md");
    std::fs::write(
        &skill_md_path,
        "---\nname: dup-skill\ndescription: A duplicate skill\n---\n\nContent\n",
    )
    .unwrap();

    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    ws.add_skill(&skill_dir).await.unwrap();
    // Add again — should skip
    ws.add_skill(&skill_dir).await.unwrap();

    let skills = ws.list_skills().await.unwrap();
    assert_eq!(skills.len(), 1);
}

#[tokio::test]
async fn test_skill_remove() {
    let (_td, workdir) = temp_workdir();

    let (_skill_td, skill_dir) = common::temp_workdir();
    let skill_md_path = std::path::Path::new(&skill_dir).join("SKILL.md");
    std::fs::write(
        &skill_md_path,
        "---\nname: removable\ndescription: To be removed\n---\n\nContent\n",
    )
    .unwrap();

    let config = LocalWorkspaceConfig {
        workdir,
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await.unwrap();

    ws.add_skill(&skill_dir).await.unwrap();
    ws.remove_skill("removable").await.unwrap();

    let skills = ws.list_skills().await.unwrap();
    assert!(!skills.iter().any(|s| s.name == "removable"));
}

#[tokio::test]
async fn test_skill_remove_unknown_warns() {
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

    ws.remove_skill("nonexistent-skill").await.unwrap();
}
