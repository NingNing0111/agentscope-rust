//! Workspace example: a `LocalWorkspace` — initialize, list tools/skills, and
//! execute a command through the contained backend.
//!
//! Needs no model or API key. A workspace scopes an agent's filesystem and
//! tool operations to a controlled working directory.

use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let workdir = dir.path().to_str().unwrap();

    // 1. Create and initialize the workspace.
    let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
        workdir: workdir.to_string(),
        workspace_id: Some("demo-ws".into()),
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: Some("Project documents live under data/.".into()),
    });
    ws.initialize().await?;
    println!("workspace initialized at {workdir}");

    // 2. Workspace instructions + tool discovery.
    println!("instructions: {}", ws.get_instructions().await);
    let tools = ws.list_tools().await?;
    println!(
        "exposed tools: {}",
        tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // 3. Execute a command through the workspace backend.
    let backend = ws.get_backend()?;
    let out = backend
        .exec_shell(&["echo", "hello from workspace"], workdir, None)
        .await?;
    println!(
        "command stdout: {}",
        String::from_utf8_lossy(&out.stdout).trim()
    );
    println!("command exit code: {:?}", out.exit_code);

    // 4. Skills (empty here).
    let skills = ws.list_skills().await?;
    println!("skills: {} loaded", skills.len());

    ws.close().await?;
    println!("\nOK: workspace initialized, queried, and executed a command.");
    Ok(())
}
