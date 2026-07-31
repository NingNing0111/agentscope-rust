//! Tests for context and tool result offloading (US3: T042-T043)

mod common;

use agent_scope_message::{
    Base64Source, ContentBlock, DataBlock, DataSource, Msg, Role, ToolOutput, ToolResultBlock,
};
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

#[tokio::test]
async fn test_offload_context_base64_extraction() {
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

    let base64_data = base64_encode(b"fake-image-data");
    let msg = Msg {
        name: "assistant".into(),
        content: vec![ContentBlock::Data(DataBlock::new(DataSource::Base64(
            Base64Source {
                data: base64_data.clone(),
                media_type: "image/png".into(),
            },
        )))],
        role: Role::Assistant,
        id: "msg-1".into(),
        metadata: serde_json::Value::Null,
        created_at: "2024-01-01T00:00:00Z".into(),
        usage: None,
        finished_at: None,
        finished_reason: None,
        structured_output: None,
        error: None,
    };

    let path = ws.offload_context("session-1", &[msg]).await.unwrap();
    assert!(path.ends_with("context.jsonl"));

    // Verify data/ directory has the extracted file
    let backend = ws.get_backend().unwrap();
    let data_dir = backend.join_path(ws.workdir(), "data");
    let entries = backend.list_dir(&data_dir, false).await.unwrap();
    // Should have at least the extracted data file (plus .keep)
    assert!(entries.len() >= 2);
}

#[tokio::test]
async fn test_offload_context_duplicate_base64_skip() {
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

    let base64_data = base64_encode(b"same-image-data");
    let msg1 = Msg {
        name: "assistant".into(),
        content: vec![ContentBlock::Data(DataBlock::new(DataSource::Base64(
            Base64Source {
                data: base64_data.clone(),
                media_type: "image/png".into(),
            },
        )))],
        role: Role::Assistant,
        id: "msg-1".into(),
        metadata: serde_json::Value::Null,
        created_at: "2024-01-01T00:00:00Z".into(),
        usage: None,
        finished_at: None,
        finished_reason: None,
        structured_output: None,
        error: None,
    };

    let msg2 = Msg {
        name: "assistant".into(),
        content: vec![ContentBlock::Data(DataBlock::new(DataSource::Base64(
            Base64Source {
                data: base64_data,
                media_type: "image/png".into(),
            },
        )))],
        role: Role::Assistant,
        id: "msg-2".into(),
        metadata: serde_json::Value::Null,
        created_at: "2024-01-01T00:00:01Z".into(),
        usage: None,
        finished_at: None,
        finished_reason: None,
        structured_output: None,
        error: None,
    };

    ws.offload_context("session-dup", &[msg1, msg2])
        .await
        .unwrap();

    // Only one file should be written for duplicate base64
    let backend = ws.get_backend().unwrap();
    let data_dir = backend.join_path(ws.workdir(), "data");
    let entries = backend.list_dir(&data_dir, false).await.unwrap();
    // .keep + 1 data file = 2
    assert_eq!(entries.len(), 2);
}

#[tokio::test]
async fn test_offload_context_jsonl_file() {
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

    let msg = Msg {
        name: "assistant".into(),
        content: vec![ContentBlock::Text(agent_scope_message::TextBlock::new(
            "hello".into(),
        ))],
        role: Role::Assistant,
        id: "plain-msg".into(),
        metadata: serde_json::Value::Null,
        created_at: "2024-01-01T00:00:00Z".into(),
        usage: None,
        finished_at: None,
        finished_reason: None,
        structured_output: None,
        error: None,
    };

    let path = ws.offload_context("session-jsonl", &[msg]).await.unwrap();

    let backend = ws.get_backend().unwrap();
    let data = backend.read_file(&path).await.unwrap();
    let text = String::from_utf8_lossy(&data);
    assert!(text.contains("plain-msg"));
    assert!(text.contains("\n"));
}

#[tokio::test]
async fn test_offload_tool_result_text() {
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

    let tr = ToolResultBlock::new(
        "tr-001".into(),
        "calculator".into(),
        ToolOutput::Text("42".into()),
    );

    let path = ws.offload_tool_result("session-tr", &tr).await.unwrap();
    assert!(path.contains("tool_result-tr-001"));

    let backend = ws.get_backend().unwrap();
    let data = backend.read_file(&path).await.unwrap();
    let text = String::from_utf8_lossy(&data);
    assert!(text.contains("calculator"));
    assert!(text.contains("42"));
}

#[tokio::test]
async fn test_offload_tool_result_filename_conflict() {
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

    let tr = ToolResultBlock::new(
        "same-id".into(),
        "tool".into(),
        ToolOutput::Text("result".into()),
    );

    let path1 = ws
        .offload_tool_result("session-conflict", &tr)
        .await
        .unwrap();
    let path2 = ws
        .offload_tool_result("session-conflict", &tr)
        .await
        .unwrap();

    // Second should have a different filename
    assert_ne!(path1, path2);
    assert!(path2.contains("(1)"));
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}
