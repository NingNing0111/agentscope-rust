//! MCP example: connect to any stdio MCP server and discover/use its tools.
//!
//! Usage:
//! ```bash
//! # List tools exposed by a stdio MCP server
//! cargo run -p mcp -- --server npx -- --call excalidraw describe_scene '{}'
//! ```
//!
//! The server command is passed to a temporary `LocalWorkspace`, which owns the
//! subprocess lifetime. Requires a stdio MCP server available on `PATH`.

use agent_scope_mcp::McpExt;
use agent_scope_workspace::mcp::{McpClientConfig, McpTransportConfig};
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};
use clap::Parser;

/// Connect to a stdio MCP server and list (optionally call) its tools.
#[derive(Parser)]
struct Cli {
    /// Command that starts the stdio MCP server.
    #[arg(long, default_value = "mcp-excalidraw-server")]
    server: String,

    /// Optional args for the server command.
    #[arg(long)]
    server_args: Vec<String>,

    /// Optional: call this tool with the given JSON arguments.
    #[arg(long)]
    call: Option<String>,

    /// JSON arguments for --call.
    #[arg(long, default_value = "{}")]
    args: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt().try_init();
    let cli = Cli::parse();

    let workdir = "/tmp/mcp-example-ws";
    let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
        workdir: workdir.into(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    });
    ws.initialize().await?;

    // Register the server (idempotent — drop any leftover config from a prior run).
    let server_name = "mcp-server";
    if ws.list_mcps().await?.iter().any(|c| c.name == server_name) {
        ws.remove_mcp(server_name).await?;
    }
    let config = McpClientConfig {
        name: server_name.into(),
        transport: McpTransportConfig::Stdio {
            command: cli.server.clone(),
            args: cli.server_args.clone(),
        },
        is_stateful: true,
    };
    ws.add_mcp(config).await?;

    // Connect: spawns the stdio subprocess, runs initialize + tools/list.
    let tools = ws.connect_mcp(server_name).await?;
    println!("connected; discovered {} remote tools:", tools.len());
    for t in &tools {
        println!("    - {} | read_only={}", t.name(), t.is_read_only());
    }

    // Optionally invoke one tool end-to-end.
    if let Some(tool_name) = cli.call {
        let tool = tools
            .iter()
            .find(|t| t.name() == tool_name)
            .ok_or_else(|| {
                anyhow::anyhow!("tool '{tool_name}' not found among {} tools", tools.len())
            })?;
        let input = serde_json::from_str(&cli.args)
            .map_err(|e| anyhow::anyhow!("--args 不是合法 JSON: {e}"))?;
        match tool.call(input).await? {
            agent_scope_tool::ToolExecOutput::Complete(out) => {
                println!("call result: {out:?}");
            }
            agent_scope_tool::ToolExecOutput::Stream(_) => {
                println!("call result: (streaming)");
            }
        }
    }

    ws.disconnect_mcp(server_name).await?;
    Ok(())
}
