//! Feature 029 — workspace built-in tool injection (FR-001/FR-002, SC-001/SC-002).
//!
//! Mirrors quickstart scenario 1: a workspace-enabled agent automatically
//! receives the built-in file/command tools; an agent without a workspace does
//! not.

use std::sync::Arc;

use agent_scope_agent::{AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_tool::ToolKit;
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

mod mocks;

use mocks::MockModel;

/// Tool names that must be injected for a workspace-enabled agent.
const WORKSPACE_TOOLS: &[&str] = &[
    "Bash",
    "Read",
    "Edit",
    "Write",
    "Grep",
    "Glob",
    "ResetTools",
    "Skill",
    "bash",
    "read",
    "edit",
    "write",
    "grep",
    "find",
    "ls",
];

/// Workspace tool names that must not be exposed without an enabled workspace.
const FILE_COMMAND_TOOLS: &[&str] = &[
    "Bash",
    "Read",
    "Edit",
    "Write",
    "Grep",
    "Glob",
    "ResetTools",
    "bash",
    "read",
    "edit",
    "write",
    "grep",
    "find",
    "ls",
    "PowerShell",
    "powershell",
];

/// Build an initialized workspace rooted at `workdir`.
async fn make_workspace(workdir: &std::path::Path) -> Arc<dyn WorkspaceBase> {
    let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
        workdir: workdir.to_string_lossy().to_string(),
        workspace_id: None,
        default_mcps: Vec::new(),
        skill_paths: Vec::new(),
        instructions: None,
    });
    ws.initialize().await.unwrap();
    Arc::new(ws)
}

/// Build a workspace-enabled agent rooted at `workdir`.
async fn workspace_agent(workdir: &std::path::Path) -> ReActAgent {
    let ws = make_workspace(workdir).await;

    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("ws-agent")
        .model(model)
        .workspace(Arc::clone(&ws))
        .auto_persist(false)
        .build()
        .unwrap();
    ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap()
}

/// Build an agent with no workspace configured.
fn plain_agent() -> ReActAgent {
    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("plain-agent")
        .model(model)
        .auto_persist(false)
        .build()
        .unwrap();
    ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap()
}

#[tokio::test]
async fn workspace_agent_has_all_builtin_tools() {
    let dir = tempfile::tempdir().unwrap();
    let agent = workspace_agent(dir.path()).await;
    let toolkit = agent.toolkit().expect("toolkit present");
    let schemas = toolkit.get_tool_schemas();
    let names: Vec<&str> = schemas
        .iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .collect();

    for expected in WORKSPACE_TOOLS {
        assert!(
            names.contains(expected),
            "missing tool '{expected}' in schemas: {names:?}"
        );
    }

    #[cfg(target_os = "windows")]
    for expected in ["PowerShell", "powershell"] {
        assert!(
            names.contains(&expected),
            "missing Windows tool '{expected}' in schemas: {names:?}"
        );
    }

    #[cfg(not(target_os = "windows"))]
    for forbidden in ["PowerShell", "powershell"] {
        assert!(
            !names.contains(&forbidden),
            "non-Windows agent must not expose '{forbidden}', got: {names:?}"
        );
    }

    // Every built-in tool carries a name, description, and JSON Schema input.
    for schema in &schemas {
        assert_eq!(schema["type"], "function");
        assert!(schema["function"]["name"].is_string());
        assert!(schema["function"]["description"].is_string());
        assert!(schema["function"]["parameters"].is_object());
    }
}

#[tokio::test]
async fn plain_agent_has_no_file_command_tools() {
    let agent = plain_agent();
    let toolkit = agent.toolkit().expect("toolkit present");
    let schemas = toolkit.get_tool_schemas();
    let names: Vec<&str> = schemas
        .iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .collect();

    for forbidden in FILE_COMMAND_TOOLS {
        assert!(
            !names.contains(forbidden),
            "agent without workspace must not expose '{forbidden}', got: {names:?}"
        );
    }
}

#[tokio::test]
async fn workspace_tools_disabled_yields_no_builtins() {
    let dir = tempfile::tempdir().unwrap();
    let ws = make_workspace(dir.path()).await;

    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("ws-off")
        .model(model)
        .workspace(Arc::clone(&ws))
        .workspace_tools_enabled(false)
        .auto_persist(false)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let toolkit = agent.toolkit().expect("toolkit present");
    let schemas = toolkit.get_tool_schemas();
    let names: Vec<&str> = schemas
        .iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .collect();
    for forbidden in FILE_COMMAND_TOOLS {
        assert!(
            !names.contains(forbidden),
            "workspace tools disabled: unexpected '{forbidden}' in {names:?}"
        );
    }
}

#[tokio::test]
async fn reset_tools_activation_filters_toolkit_schemas() {
    // A toolkit with a non-basic tool group; binding it to a workspace and
    // running ResetTools should hide tools in deactivated groups.
    let dir = tempfile::tempdir().unwrap();
    let ws = make_workspace(dir.path()).await;

    let mut toolkit = ToolKit::new();
    // Register a marker tool in a non-basic group.
    toolkit
        .try_register_in_group(
            "coding",
            agent_scope_tool::FunctionTool::new("code_marker", "Marker tool", marker_handler),
        )
        .unwrap();

    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("ws-groups")
        .model(model)
        .toolkit(toolkit)
        .workspace(Arc::clone(&ws))
        .auto_persist(false)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    // The custom group's tool is registered.
    let tk = agent.toolkit().unwrap();
    assert!(tk.contains("code_marker"));

    // By default no non-basic group is active, so `code_marker` (group
    // "coding") is hidden while `basic` tools (Skill) stay visible.
    let schemas = tk.get_tool_schemas();
    let names: Vec<&str> = schemas
        .iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .collect();
    assert!(
        !names.contains(&"code_marker"),
        "deactivated group leaked: {names:?}"
    );
    assert!(
        names.contains(&"Skill"),
        "basic group must stay visible: {names:?}"
    );
}

#[tokio::test]
async fn workspace_builtin_name_collision_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let ws = make_workspace(dir.path()).await;

    let mut reserved_names = FILE_COMMAND_TOOLS.to_vec();

    #[cfg(not(target_os = "windows"))]
    reserved_names.retain(|name| !matches!(*name, "PowerShell" | "powershell"));

    for reserved in reserved_names {
        let mut toolkit = ToolKit::new();
        toolkit
            .try_register(agent_scope_tool::FunctionTool::new(
                reserved,
                "Conflicting custom tool",
                marker_handler,
            ))
            .unwrap();

        let model = Arc::new(MockModel::new("mock", "ok"));
        let config = AgentConfig::builder()
            .name(format!("ws-conflict-{reserved}"))
            .model(model)
            .toolkit(toolkit)
            .workspace(Arc::clone(&ws))
            .auto_persist(false)
            .build()
            .unwrap();

        let err = match ReActAgent::new(
            config,
            ReActConfig::default(),
            ContextConfig::default(),
            vec![],
        ) {
            Ok(_) => panic!("reserved workspace built-in collision must fail construction"),
            Err(err) => err,
        };

        let message = err.to_string();
        assert!(
            message.contains(reserved),
            "error should identify reserved tool '{reserved}', got: {message}"
        );
    }
}

async fn marker_handler(_x: serde_json::Value) -> String {
    "ok".to_string()
}
