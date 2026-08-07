//! Real-world MCP integration debug: connect to `mcp-excalidraw-server`
//! (a real Node.js stdio MCP server), list its tools, and call a few of them
//! end-to-end through the `agent_scope_mcp` adapter.
//!
//! Usage:
//! ```bash
//! cargo run -p agent_scope_mcp --example mcp_excalidraw_debug
//! ```
//!
//! Requires `mcp-excalidraw-server` on `PATH` (install: `npm i -g mcp-excalidraw-server`).

use std::sync::Arc;

use agent_scope_mcp::McpExt;
use agent_scope_tool::{Tool, ToolExecOutput};
use agent_scope_workspace::mcp::{McpClientConfig, McpTransportConfig};
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt().try_init();

    // Workdir sits under macOS `/tmp` (a symlink to `/private/tmp`) and does
    // not exist yet — the exact scenario that used to trip a workspace
    // containment bug (fixed in agent_scope_workspace).
    let workdir = "/tmp/mcp-excalidraw-debug-ws";

    let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
        workdir: workdir.into(),
        workspace_id: None,
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: None,
    });
    ws.initialize().await?;

    // Register the real stdio MCP server (idempotent — drop any config left
    // over from a previous run so this example is re-runnable).
    if ws.list_mcps().await?.iter().any(|c| c.name == "excalidraw") {
        ws.remove_mcp("excalidraw").await?;
    }
    let config = McpClientConfig {
        name: "excalidraw".into(),
        transport: McpTransportConfig::Stdio {
            command: "mcp-excalidraw-server".into(),
            args: vec![],
        },
        is_stateful: true,
    };
    ws.add_mcp(config.clone()).await?;
    println!("[1] registered MCP 'excalidraw' → .mcp file written");

    // Connect (spawns the Node stdio subprocess, performs initialize +
    // tools/list over the MCP protocol).
    let tools = ws.connect_mcp("excalidraw").await?;
    println!("[2] connected, discovered {} remote tools:", tools.len());
    for t in &tools {
        println!("    - {} | read_only={}", t.name(), t.is_read_only());
    }

    // Make sure the cached list agrees with the live one.
    let cached = ws.get_mcp_tools("excalidraw").await?;
    assert_eq!(cached.len(), tools.len());

    // Find a few interesting tools by name.
    let find = |name: &str| -> Arc<dyn Tool> {
        tools
            .iter()
            .find(|t| t.name() == name)
            .unwrap_or_else(|| panic!("tool {name} not found"))
            .clone()
    };

    // Call 1: clear the canvas (idempotent, no args).
    let clear = find("excalidraw/clear_canvas");
    let out = call(&*clear, json!({})).await?;
    println!("[3] clear_canvas → {}", out);

    // Call 2: create a rectangle element.
    let create = find("excalidraw/create_element");
    let out = call(
        &*create,
        json!({
            "type": "rectangle",
            "x": 100,
            "y": 100,
            "width": 200,
            "height": 120,
            "backgroundColor": "#ffec99",
            "strokeColor": "#d9480f",
        }),
    )
    .await?;
    println!("[4] create_element → {}", out);

    // Call 3: describe the scene — the server reads back what it holds.
    let describe = find("excalidraw/describe_scene");
    let out = call(&*describe, json!({})).await?;
    println!("[5] describe_scene → {}", out);

    // Call 4: query elements back.
    let query = find("excalidraw/query_elements");
    let out = call(&*query, json!({})).await?;
    println!("[6] query_elements → {}", out);

    // Clean up: disconnect releases the subprocess.
    ws.disconnect_mcp("excalidraw").await?;
    println!("[7] disconnected, subprocess released");

    println!("\n✅ ALL STEPS PASSED — real stdio MCP round-trip works.");
    Ok(())
}

/// Run one remote tool and return the concatenated text output.
async fn call(
    tool: &dyn Tool,
    input: serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    match tool.call(input).await? {
        ToolExecOutput::Complete(block) => {
            let text = match &block.output {
                agent_scope_message::ToolOutput::Text(s) => s.clone(),
                agent_scope_message::ToolOutput::Blocks(_) => "<block output>".to_string(),
            };
            Ok(text)
        }
        ToolExecOutput::Stream(_) => Ok("<streaming output>".to_string()),
    }
}
