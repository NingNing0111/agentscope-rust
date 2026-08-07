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

// ============================================================================
// Defect 3: MCP header scrubbing tests (FAILING before scrubbing fix)
// ============================================================================

/// Bearer tokens in SSE transport headers must not be persisted to .mcp.
#[tokio::test]
async fn test_mcp_headers_bearer_not_persisted() {
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

    let mut headers = std::collections::HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        "Bearer sk-abc123secret".to_string(),
    );
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let mcp = McpClientConfig {
        name: "bearer-mcp".into(),
        transport: McpTransportConfig::Sse {
            url: "https://api.example.com/sse".into(),
            headers,
        },
        is_stateful: false,
    };
    ws.add_mcp(mcp).await.unwrap();

    // Check .mcp file does NOT contain the secret
    let backend = ws.get_backend().unwrap();
    let mcp_path = backend.join_path(ws.workdir(), ".mcp");
    let data = backend.read_file(&mcp_path).await.unwrap();
    let text = String::from_utf8_lossy(&data);
    assert!(
        !text.contains("sk-abc123secret"),
        "Bearer token leaked to .mcp file"
    );
    // The header key should be preserved (or redacted), but value must not leak
    assert!(
        text.contains("bearer-mcp"),
        "MCP entry should still be present"
    );
}

/// Bearer tokens in StreamableHttp transport headers must not be persisted.
#[tokio::test]
async fn test_mcp_headers_bearer_not_persisted_streamable_http() {
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

    let mut headers = std::collections::HashMap::new();
    headers.insert("x-api-key".to_string(), "key-12345-secret".to_string());
    headers.insert("Accept".to_string(), "application/json".to_string());

    let mcp = McpClientConfig {
        name: "api-key-mcp".into(),
        transport: McpTransportConfig::StreamableHttp {
            url: "https://api.example.com/mcp".into(),
            headers,
        },
        is_stateful: false,
    };
    ws.add_mcp(mcp).await.unwrap();

    // Check .mcp file does NOT contain the API key value
    let backend = ws.get_backend().unwrap();
    let mcp_path = backend.join_path(ws.workdir(), ".mcp");
    let data = backend.read_file(&mcp_path).await.unwrap();
    let text = String::from_utf8_lossy(&data);
    assert!(
        !text.contains("key-12345-secret"),
        "API key leaked to .mcp file"
    );
    // Non-sensitive header values should be preserved
    assert!(
        text.contains("application/json"),
        "Non-sensitive header should persist"
    );
}

/// list_mcps() must not return sensitive header values in-memory either.
#[tokio::test]
async fn test_mcp_list_mcps_scrubs_headers() {
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

    let mut headers = std::collections::HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        "Bearer top-secret-token".to_string(),
    );
    headers.insert(
        "Cookie".to_string(),
        "session=abc123; token=xyz".to_string(),
    );
    headers.insert("User-Agent".to_string(), "test-agent".to_string());

    let mcp = McpClientConfig {
        name: "sensitive-headers".into(),
        transport: McpTransportConfig::Sse {
            url: "https://api.example.com/sse".into(),
            headers,
        },
        is_stateful: false,
    };
    ws.add_mcp(mcp).await.unwrap();

    let mcps = ws.list_mcps().await.unwrap();
    assert_eq!(mcps.len(), 1);

    // Get the headers from the listed MCP
    let listed_headers = match &mcps[0].transport {
        McpTransportConfig::Sse { headers, .. } => headers,
        _ => panic!("expected SSE transport"),
    };

    // Sensitive values must be scrubbed
    for (key, value) in listed_headers {
        let key_lower = key.to_lowercase();
        if key_lower == "authorization" || key_lower == "cookie" {
            assert!(
                !value.contains("top-secret-token") && !value.contains("abc123"),
                "Sensitive header '{key}' value leaked in list_mcps: {value}"
            );
        }
    }

    // Non-sensitive header should be intact
    assert_eq!(
        listed_headers.get("User-Agent").map(|s| s.as_str()),
        Some("test-agent")
    );
}

/// After re-initialize, sensitive headers loaded from .mcp must still be scrubbed.
#[tokio::test]
async fn test_mcp_headers_scrubbed_on_reinit() {
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

    let mut headers = std::collections::HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        "Bearer reinit-secret".to_string(),
    );

    let mcp = McpClientConfig {
        name: "reinit-mcp".into(),
        transport: McpTransportConfig::Sse {
            url: "https://api.example.com/sse".into(),
            headers,
        },
        is_stateful: false,
    };
    ws.add_mcp(mcp).await.unwrap();
    ws.close().await.unwrap();

    // Re-initialize — headers must be scrubbed from loaded config
    ws.initialize().await.unwrap();
    let mcps = ws.list_mcps().await.unwrap();
    assert_eq!(mcps.len(), 1);

    let listed_headers = match &mcps[0].transport {
        McpTransportConfig::Sse { headers, .. } => headers,
        _ => panic!("expected SSE transport"),
    };

    let auth_value = listed_headers
        .get("Authorization")
        .map(|s| s.as_str())
        .unwrap_or("");
    assert!(
        !auth_value.contains("reinit-secret"),
        "Bearer token survived reinit in list_mcps: {auth_value}"
    );
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
