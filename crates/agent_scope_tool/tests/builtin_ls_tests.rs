//! pi-compatible lowercase `ls` workspace tool tests.

mod common;

use agent_scope_message::ToolResultState;
use agent_scope_tool::Tool;
use agent_scope_tool::builtin::LsTool;

use common::{ctx_in, text_of, write_ws_file};

fn complete_block(out: agent_scope_tool::ToolExecOutput) -> agent_scope_message::ToolResultBlock {
    match out {
        agent_scope_tool::ToolExecOutput::Complete(block) => block,
        _ => panic!("expected Complete"),
    }
}

#[tokio::test]
async fn ls_default_lists_workspace_root() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "a.txt", "a\n");
    std::fs::create_dir_all(std::path::Path::new(&h.workdir).join("src")).unwrap();
    std::fs::write(std::path::Path::new(&h.workdir).join(".env"), "x\n").unwrap();

    let tool = LsTool::new(h.ctx.clone());
    let block = complete_block(tool.call(serde_json::json!({})).await.unwrap());

    assert_eq!(block.state, ToolResultState::Success);
    let text = text_of(&block);
    assert!(text.contains("a.txt"), "got: {text}");
    assert!(text.contains("src/"), "got: {text}");
    assert!(text.contains(".env"), "got: {text}");
}

#[tokio::test]
async fn ls_supports_subdir_and_limit_truncation() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "src/a.txt", "a\n");
    write_ws_file(&h, "src/b.txt", "b\n");

    let tool = LsTool::new(h.ctx.clone());
    let block = complete_block(
        tool.call(serde_json::json!({ "path": "src", "limit": 1 }))
            .await
            .unwrap(),
    );

    assert_eq!(block.state, ToolResultState::Success);
    let text = text_of(&block);
    assert_eq!(
        text.lines().filter(|line| line.ends_with(".txt")).count(),
        1
    );
    assert!(text.contains("truncated"), "got: {text}");
}

#[tokio::test]
async fn ls_limit_zero_on_non_empty_dir_reports_no_entries_returned() {
    let h = ctx_in(&[]);
    write_ws_file(&h, "src/a.txt", "a\n");

    let tool = LsTool::new(h.ctx.clone());
    let block = complete_block(
        tool.call(serde_json::json!({ "path": "src", "limit": 0 }))
            .await
            .unwrap(),
    );

    assert_eq!(block.state, ToolResultState::Success);
    let text = text_of(&block);
    assert!(text.contains("No entries returned from"), "got: {text}");
    assert!(!text.contains("No entries found"), "got: {text}");
    assert!(text.contains("truncated"), "got: {text}");
}

#[tokio::test]
async fn ls_rejects_path_escape_and_non_directory() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "a.txt", "a\n");
    let tool = LsTool::new(h.ctx.clone());

    let escaped = complete_block(
        tool.call(serde_json::json!({ "path": "../outside" }))
            .await
            .unwrap(),
    );
    assert_eq!(escaped.state, ToolResultState::Error);
    assert!(text_of(&escaped).contains("path_outside_workspace"));

    let not_dir = complete_block(
        tool.call(serde_json::json!({ "path": file }))
            .await
            .unwrap(),
    );
    assert_eq!(not_dir.state, ToolResultState::Error);
    assert!(text_of(&not_dir).contains("file_not_found"));
}
