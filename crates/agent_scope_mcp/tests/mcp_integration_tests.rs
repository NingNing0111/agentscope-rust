//! In-process integration tests for `agent_scope_mcp`.
//!
//! These tests run entirely inside the process: a client↔server channel is
//! established over `tokio::io::duplex`, the server exposes an `add` tool, and
//! `McpClient`/`McpTool` are exercised against it. No external process or
//! network is required, so they run in any CI environment (US4).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use agent_scope_mcp::{McpClient, McpExt, McpTool};
use agent_scope_message::ToolOutput;
use agent_scope_tool::ToolExecOutput;
use agent_scope_tool::tool_trait::Tool;
use agent_scope_workspace::error::WorkspaceError;
use agent_scope_workspace::mcp::{McpClientConfig, McpTransportConfig};
use agent_scope_workspace::{
    LocalWorkspace, LocalWorkspaceConfig, McpConnectionHandle, McpConnectionsHost, WorkspaceBase,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ClientInfo, ToolAnnotations};
use rmcp::service::ServiceExt;
use rmcp::{tool, tool_router};
use schemars::JsonSchema;
use schemars_1 as schemars;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Test MCP server
// ─────────────────────────────────────────────────────────────────────────────

/// A minimal stateless MCP server exposing a single `add` tool.
#[derive(Debug, Clone)]
struct AddServer;

/// Input schema for the `add` tool.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct AddParams {
    a: i64,
    b: i64,
}

/// `server_handler` emits the `ServerHandler` impl; `#[tool]` registers `add`.
#[tool_router(server_handler)]
impl AddServer {
    #[tool(description = "Add two integers and return the sum")]
    fn add(&self, Parameters(AddParams { a, b }): Parameters<AddParams>) -> String {
        (a + b).to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fixture (T023)
// ─────────────────────────────────────────────────────────────────────────────

/// Build an in-process MCP server exposing `add`, connect a client to it, and
/// return the connected client plus the remote tool name.
///
/// The server runs in a spawned task; the client side is handed to
/// [`McpClient::attach`] — the documented test injection point.
async fn create_test_mcp_server_with_add_tool() -> (Arc<McpClient>, String) {
    let (server_io, client_io) = tokio::io::duplex(4096);

    tokio::spawn(async move {
        let running = AddServer
            .serve(server_io)
            .await
            .map_err(|e| e.to_string())?;
        running.waiting().await.map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    });

    let config = McpClientConfig {
        name: "search".to_string(),
        transport: McpTransportConfig::Stdio {
            command: "unused".to_string(),
            args: vec![],
        },
        is_stateful: true,
    };
    let client = Arc::new(McpClient::new(config));
    let service = ClientInfo::default()
        .serve(client_io)
        .await
        .expect("client side must serve");
    client.attach(service).await.expect("attach must succeed");
    (client, "add".to_string())
}

fn parse_args(input: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    input.as_object().expect("args must be an object").clone()
}

/// Real-network regression: connect to Huice's MCP endpoint with API-key auth
/// and verify tool discovery succeeds.
///
/// Run manually with:
/// `HUICE_MCP_URL=... HUICE_MCP_API_KEY=... cargo test -p agent_scope_mcp --test mcp_integration_tests test_real_huice_streamable_http_connects -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn test_real_huice_streamable_http_connects() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("HUICE_MCP_URL")?;
    let api_key = std::env::var("HUICE_MCP_API_KEY")?;

    let mut headers = HashMap::new();
    headers.insert("X-API-Key".to_string(), api_key);

    let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
        workdir: std::env::temp_dir()
            .join(format!("agentscope-huice-mcp-{}", std::process::id()))
            .to_string_lossy()
            .into_owned(),
        workspace_id: None,
        default_mcps: vec![McpClientConfig {
            name: "huice".to_string(),
            transport: McpTransportConfig::StreamableHttp { url, headers },
            is_stateful: true,
        }],
        skill_paths: vec![],
        instructions: None,
    });
    ws.initialize().await?;

    let tools = ws.connect_mcp("huice").await?;
    assert!(!tools.is_empty(), "expected at least one remote tool");
    for tool in &tools {
        println!("{} | {}", tool.name(), tool.description());
    }
    ws.disconnect_mcp("huice").await?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// US1+US2 tests (T024-T029)
// ─────────────────────────────────────────────────────────────────────────────

/// After a successful connection, `list_tools()` returns the remote `add` tool.
#[tokio::test]
async fn test_connect_and_list_tools() -> Result<(), Box<dyn std::error::Error>> {
    let (client, tool_name) = create_test_mcp_server_with_add_tool().await;
    let tools = client.list_tools()?;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name.as_ref(), tool_name);
    Ok(())
}

/// Calling `add` with valid arguments returns the sum.
#[tokio::test]
async fn test_call_tool_success() -> Result<(), Box<dyn std::error::Error>> {
    let (client, _) = create_test_mcp_server_with_add_tool().await;
    let result = client
        .call_tool("add", parse_args(serde_json::json!({"a": 1, "b": 2})))
        .await?;
    let text = result
        .content
        .first()
        .and_then(|c| c.as_text())
        .expect("add must return text content");
    assert_eq!(text.text, "3");
    Ok(())
}

/// Calling `add` with an incomplete argument set is surfaced as an
/// error-marked `CallToolResult` (the protocol-level error channel).
#[tokio::test]
async fn test_call_tool_error() -> Result<(), Box<dyn std::error::Error>> {
    let (client, _) = create_test_mcp_server_with_add_tool().await;
    let result = client
        .call_tool("add", parse_args(serde_json::json!({"a": 1})))
        .await?;
    assert_eq!(
        result.is_error,
        Some(true),
        "missing field must mark the result as an error"
    );
    assert!(
        result.content.iter().any(|c| c.as_text().is_some()),
        "error result must carry an explanatory text block"
    );
    Ok(())
}

/// Calling an unknown tool name is a typed call error (`McpCallError`), not a
/// panic (FR-009 / SC-004).
#[tokio::test]
async fn test_call_unknown_tool_returns_typed_error() -> Result<(), Box<dyn std::error::Error>> {
    let (client, _) = create_test_mcp_server_with_add_tool().await;
    let err = client
        .call_tool("no_such_tool", serde_json::Map::new())
        .await
        .expect_err("unknown tool must fail");
    assert!(
        matches!(err, WorkspaceError::McpCallError { .. }),
        "expected McpCallError, got {err:?}"
    );
    Ok(())
}

/// The unified tool name is `{mcp_name}/{tool_name}` and the description is
/// prefixed with the MCP client name.
#[tokio::test]
async fn test_mcp_tool_name_prefix() -> Result<(), Box<dyn std::error::Error>> {
    let (client, _) = create_test_mcp_server_with_add_tool().await;
    // `Tool` is #[non_exhaustive], so use the builder API.
    let mut annotations = ToolAnnotations::default();
    annotations.read_only_hint = Some(true);
    let rmcp_tool = rmcp::model::Tool::new(
        "query",
        "Query the search index",
        serde_json::json!({}).as_object().expect("object").clone(),
    )
    .with_annotations(annotations);
    let tool = McpTool::new("search".into(), rmcp_tool, Arc::clone(&client));
    assert_eq!(tool.name(), "search/query");
    assert_eq!(
        tool.description(),
        "[remote MCP: search] Query the search index"
    );
    assert!(tool.is_read_only());
    assert!(tool.is_concurrency_safe());
    Ok(())
}

/// After `disconnect()`, the connection is released and calls fail with
/// `McpNotConnected`.
#[tokio::test]
async fn test_disconnect_releases_connection() -> Result<(), Box<dyn std::error::Error>> {
    let (client, _) = create_test_mcp_server_with_add_tool().await;
    assert!(client.is_connected());
    client.disconnect().await?;
    assert!(!client.is_connected());
    let err = client
        .call_tool("add", serde_json::Map::new())
        .await
        .expect_err("call after disconnect must fail");
    assert!(
        matches!(err, WorkspaceError::McpNotConnected { .. }),
        "expected McpNotConnected, got {err:?}"
    );
    Ok(())
}

/// A never-connected client reports `McpNotConnected` from both `call_tool`
/// and `list_tools`.
#[tokio::test]
async fn test_not_connected_returns_error() -> Result<(), Box<dyn std::error::Error>> {
    let config = McpClientConfig {
        name: "never".to_string(),
        transport: McpTransportConfig::Stdio {
            command: "unused".to_string(),
            args: vec![],
        },
        is_stateful: true,
    };
    let client = McpClient::new(config);

    let call_err = client
        .call_tool("add", serde_json::Map::new())
        .await
        .expect_err("not connected");
    assert!(matches!(call_err, WorkspaceError::McpNotConnected { .. }));

    let list_err = client.list_tools().expect_err("not connected");
    assert!(matches!(list_err, WorkspaceError::McpNotConnected { .. }));
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// US3 tests (T032)
// ─────────────────────────────────────────────────────────────────────────────

/// A legacy `"type": "sse"` config still parses, and `connect()` routes it to
/// the streamable-http transport (mapping notice) then fails cleanly with a
/// typed connection error rather than panicking.
#[tokio::test]
async fn test_sse_config_parsed_and_mapped() -> Result<(), Box<dyn std::error::Error>> {
    let json = r#"{
        "name": "legacy-sse",
        "transport": { "type": "sse", "url": "http://127.0.0.1:1/mcp" }
    }"#;
    let cfg: McpClientConfig = serde_json::from_str(json)?;
    assert!(
        matches!(cfg.transport, McpTransportConfig::Sse { .. }),
        "sse tag must parse to the Sse variant"
    );

    // Capture the `info!` mapping notice emitted by `connect()` (FR-002).
    let logs: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = LogWriter(Arc::clone(&logs));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_max_level(tracing::Level::INFO)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    // connect() maps SSE → streamable-http and attempts a real connection; the
    // unroutable address must produce a typed McpConnectionError (not a panic).
    let client = McpClient::new(cfg);
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), client.connect()).await;
    match outcome {
        Ok(Err(WorkspaceError::McpConnectionError { .. })) => {}
        Ok(Ok(())) => panic!("connect to unroutable address must not succeed"),
        Ok(Err(other)) => panic!("expected McpConnectionError, got {other:?}"),
        Err(_) => panic!("connect to unroutable address timed out"),
    }

    // FR-002: the SSE→streamable-http mapping must be announced.
    let collected = logs.lock().unwrap_or_else(|p| p.into_inner());
    assert!(
        collected
            .iter()
            .any(|l| l.contains("mapped to streamable-http transport")),
        "expected the SSE mapping notice, got: {collected:?}"
    );
    Ok(())
}

/// Minimal `io::Write` that records formatted tracing output in memory.
#[derive(Clone)]
struct LogWriter(Arc<std::sync::Mutex<Vec<String>>>);

impl std::io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut logs = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("log buffer poisoned"))?;
        logs.push(String::from_utf8_lossy(buf).into_owned());
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// US4 tests (T034-T037)
// ─────────────────────────────────────────────────────────────────────────────

/// Connecting to an unroutable URL fails with a typed `McpConnectionError`,
/// not a panic (FR-009 / SC-004).
#[tokio::test]
async fn test_connection_error_typed() -> Result<(), Box<dyn std::error::Error>> {
    let config = McpClientConfig {
        name: "bad-http".to_string(),
        transport: McpTransportConfig::StreamableHttp {
            url: "http://127.0.0.1:1/mcp".to_string(),
            headers: HashMap::new(),
        },
        is_stateful: true,
    };
    let client = McpClient::new(config);
    let outcome = tokio::time::timeout(Duration::from_secs(10), client.connect()).await;
    match outcome {
        Ok(Err(WorkspaceError::McpConnectionError { .. })) => Ok(()),
        Ok(Ok(())) => panic!("connect to unroutable URL must not succeed"),
        Ok(Err(other)) => panic!("expected McpConnectionError, got {other:?}"),
        Err(_) => panic!("connect to unroutable URL timed out"),
    }
}

/// Several `McpTool` adapters sharing one client must call concurrently
/// without deadlock, races, or wrong results (the client serializes through
/// its internal mutex).
#[tokio::test]
async fn test_concurrent_tool_calls() -> Result<(), Box<dyn std::error::Error>> {
    let (client, tool_name) = create_test_mcp_server_with_add_tool().await;
    let schema = serde_json::json!({})
        .as_object()
        .cloned()
        .ok_or("schema must be an object")?;
    let rmcp_tool = rmcp::model::Tool::new(
        tool_name.clone(),
        "Add two integers and return the sum",
        schema,
    );

    let mut handles = Vec::new();
    for _ in 0..8 {
        let adapter = Arc::new(McpTool::new(
            "search".into(),
            rmcp_tool.clone(),
            Arc::clone(&client),
        ));
        handles.push(tokio::spawn(async move {
            adapter.call(serde_json::json!({"a": 1, "b": 2})).await
        }));
    }
    for handle in handles {
        let output = handle.await??;
        match output {
            ToolExecOutput::Complete(block) => match &block.output {
                ToolOutput::Text(text) => assert_eq!(text, "3"),
                other => panic!("unexpected output kind: {other:?}"),
            },
            ToolExecOutput::Stream(_) => panic!("add is not a streaming tool"),
        }
    }
    Ok(())
}

/// `close()` disconnects all live MCP connections and empties the registry, so
/// later `get_mcp_tools()` reports not-connected (FR-010).
#[tokio::test]
async fn test_close_disconnects_all_mcps() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = tempfile::tempdir()?;
    let config = LocalWorkspaceConfig {
        workdir: tmp.path().to_string_lossy().to_string(),
        workspace_id: None,
        default_mcps: vec![mcp_config("search")],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await?;

    // Simulate a live registered connection with a real (unconnected)
    // `McpClient` handle — the lifecycle under test is the workspace's release
    // path, not the transport itself.
    let client = Arc::new(McpClient::new(mcp_config("search")));
    ws.mcp_connections().lock().await.insert(
        "search".to_string(),
        Arc::clone(&client) as Arc<dyn McpConnectionHandle>,
    );
    assert!(ws.mcp_connections().lock().await.contains_key("search"));

    ws.close().await?;

    assert!(
        ws.mcp_connections().lock().await.is_empty(),
        "close() must release all MCP connections"
    );
    let err = match ws.get_mcp_tools("search").await {
        Ok(_) => panic!("get_mcp_tools after close must fail"),
        Err(e) => e,
    };
    assert!(matches!(err, WorkspaceError::McpNotConnected { .. }));
    Ok(())
}

/// `reset()` drops all MCP connections and clears the config list.
#[tokio::test]
async fn test_reset_clears_mcps() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = tempfile::tempdir()?;
    let config = LocalWorkspaceConfig {
        workdir: tmp.path().to_string_lossy().to_string(),
        workspace_id: None,
        default_mcps: vec![mcp_config("search")],
        skill_paths: vec![],
        instructions: None,
    };
    let mut ws = LocalWorkspace::new(config);
    ws.initialize().await?;

    let client = Arc::new(McpClient::new(mcp_config("search")));
    ws.mcp_connections().lock().await.insert(
        "search".to_string(),
        Arc::clone(&client) as Arc<dyn McpConnectionHandle>,
    );

    ws.reset().await?;

    assert!(
        ws.mcp_connections().lock().await.is_empty(),
        "reset() must release all MCP connections"
    );
    let mcps = ws.list_mcps().await?;
    assert!(mcps.is_empty(), "reset() must clear the MCP config list");
    Ok(())
}

/// A config helper for tests that need a persisted MCP entry.
fn mcp_config(name: &str) -> McpClientConfig {
    McpClientConfig {
        name: name.to_string(),
        transport: McpTransportConfig::Stdio {
            command: "unused".to_string(),
            args: vec![],
        },
        is_stateful: true,
    }
}
