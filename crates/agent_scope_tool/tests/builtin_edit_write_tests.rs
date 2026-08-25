//! US2 — read-before-modify guards for Edit/Write (FR-008/FR-012, SC-003/SC-004).
//!
//! Mirrors the acceptance scenarios in `specs/029-agent-workspace-tools/spec.md`
//! and the quickstart scenario 2.

mod common;

use agent_scope_message::ToolResultState;
use agent_scope_tool::Tool;
use agent_scope_tool::builtin::{EditTool, ReadTool, WriteTool};

use common::{ctx_in, text_of, write_ws_file};

fn state_of(block: &agent_scope_message::ToolResultBlock) -> ToolResultState {
    block.state.clone()
}

#[tokio::test]
async fn edit_unread_file_rejected_read_before_modify() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "note.txt", "unique content\n");
    let tool = EditTool::new(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({
            "file_path": file,
            "old_string": "unique",
            "new_string": "replaced",
        }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Error);
    let text = text_of(&block);
    assert!(text.contains("read_before_modify_required"), "got: {text}");
}

#[tokio::test]
async fn edit_after_read_replaces_unique_string() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "note.txt", "unique content\n");
    let read = ReadTool::new(h.ctx.clone());
    read.call(serde_json::json!({ "file_path": file }))
        .await
        .unwrap();
    let tool = EditTool::new(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({
            "file_path": file,
            "old_string": "unique",
            "new_string": "replaced",
        }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Success);
    assert!(text_of(&block).contains("Successfully replaced"));
    let actual = std::fs::read_to_string(std::path::Path::new(&file)).unwrap();
    assert_eq!(actual, "replaced content\n");
}

#[tokio::test]
async fn edit_ambiguous_without_replace_all_rejected() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "note.txt", "dup dup\n");
    let read = ReadTool::new(h.ctx.clone());
    read.call(serde_json::json!({ "file_path": file }))
        .await
        .unwrap();
    let tool = EditTool::new(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({
            "file_path": file,
            "old_string": "dup",
            "new_string": "x",
        }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Error);
    assert!(text_of(&block).contains("ambiguous_edit"));

    // With replace_all=true it succeeds.
    let out = tool
        .call(serde_json::json!({
            "file_path": file,
            "old_string": "dup",
            "new_string": "x",
            "replace_all": true,
        }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Success);
}

#[tokio::test]
async fn write_new_file_succeeds() {
    let h = ctx_in(&[]);
    let file = std::path::Path::new(&h.workdir).join("new.txt");
    let file_str = file.to_string_lossy().to_string();
    let tool = WriteTool::new(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({ "file_path": file_str, "content": "hi\n" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Success);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "hi\n");
}

#[tokio::test]
async fn write_overwrite_unread_rejected() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "a.txt", "old\n");
    let tool = WriteTool::new(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({ "file_path": file, "content": "new\n" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Error);
    assert!(text_of(&block).contains("read_before_modify_required"));
    // File unchanged.
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(&file)).unwrap(),
        "old\n"
    );
}

#[tokio::test]
async fn write_overwrite_after_read_succeeds() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "a.txt", "old\n");
    let read = ReadTool::new(h.ctx.clone());
    read.call(serde_json::json!({ "file_path": file }))
        .await
        .unwrap();
    let tool = WriteTool::new(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({ "file_path": file, "content": "new content\n" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Success);
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(&file)).unwrap(),
        "new content\n"
    );
}

#[tokio::test]
async fn edit_pattern_not_found_rejected() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "a.txt", "hello\n");
    let read = ReadTool::new(h.ctx.clone());
    read.call(serde_json::json!({ "file_path": file }))
        .await
        .unwrap();
    let tool = EditTool::new(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({
            "file_path": file,
            "old_string": "missing",
            "new_string": "x",
        }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(state_of(&block), ToolResultState::Error);
    assert!(text_of(&block).contains("pattern_not_found"));
}

#[tokio::test]
async fn pi_read_records_state_for_pi_edit_and_write() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "a.txt", "hello old\n");

    let read = ReadTool::new_pi(h.ctx.clone());
    let read_block = match read
        .call(serde_json::json!({ "path": file }))
        .await
        .unwrap()
    {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(read_block.name, "read");
    assert_eq!(state_of(&read_block), ToolResultState::Success);

    let edit = EditTool::new_pi(h.ctx.clone());
    let edit_block = match edit
        .call(serde_json::json!({
            "path": file,
            "edits": [{ "oldText": "old", "newText": "new" }]
        }))
        .await
        .unwrap()
    {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(edit_block.name, "edit");
    assert_eq!(state_of(&edit_block), ToolResultState::Success);
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(&file)).unwrap(),
        "hello new\n"
    );

    let write = WriteTool::new_pi(h.ctx.clone());
    let write_block = match write
        .call(serde_json::json!({ "path": file, "content": "replacement\n" }))
        .await
        .unwrap()
    {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(write_block.name, "write");
    assert_eq!(state_of(&write_block), ToolResultState::Success);
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(&file)).unwrap(),
        "replacement\n"
    );
}

#[tokio::test]
async fn pi_edit_and_write_require_prior_read_for_existing_files() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "a.txt", "old\n");

    let edit = EditTool::new_pi(h.ctx.clone());
    let edit_block = match edit
        .call(serde_json::json!({
            "path": file,
            "edits": [{ "oldText": "old", "newText": "new" }]
        }))
        .await
        .unwrap()
    {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(edit_block.name, "edit");
    assert_eq!(state_of(&edit_block), ToolResultState::Error);
    assert!(text_of(&edit_block).contains("read_before_modify_required"));

    let write = WriteTool::new_pi(h.ctx.clone());
    let write_block = match write
        .call(serde_json::json!({ "path": file, "content": "new\n" }))
        .await
        .unwrap()
    {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(write_block.name, "write");
    assert_eq!(state_of(&write_block), ToolResultState::Error);
    assert!(text_of(&write_block).contains("read_before_modify_required"));
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(&file)).unwrap(),
        "old\n"
    );
}

#[tokio::test]
async fn pi_edit_batch_is_atomic_on_missing_pattern() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "a.txt", "one two\n");
    ReadTool::new_pi(h.ctx.clone())
        .call(serde_json::json!({ "path": file }))
        .await
        .unwrap();

    let edit = EditTool::new_pi(h.ctx.clone());
    let block = match edit
        .call(serde_json::json!({
            "path": file,
            "edits": [
                { "oldText": "one", "newText": "ONE" },
                { "oldText": "missing", "newText": "MISSING" }
            ]
        }))
        .await
        .unwrap()
    {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };

    assert_eq!(block.name, "edit");
    assert_eq!(state_of(&block), ToolResultState::Error);
    assert!(text_of(&block).contains("pattern_not_found"));
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(&file)).unwrap(),
        "one two\n"
    );
}

#[tokio::test]
async fn pi_edit_rejects_too_many_replacements() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "a.txt", "target\n");
    ReadTool::new_pi(h.ctx.clone())
        .call(serde_json::json!({ "path": file }))
        .await
        .unwrap();

    let edits: Vec<_> = (0..101)
        .map(|idx| serde_json::json!({ "oldText": format!("target{idx}"), "newText": "x" }))
        .collect();
    let block = match EditTool::new_pi(h.ctx.clone())
        .call(serde_json::json!({ "path": file, "edits": edits }))
        .await
        .unwrap()
    {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };

    assert_eq!(block.name, "edit");
    assert_eq!(state_of(&block), ToolResultState::Error);
    assert!(text_of(&block).contains("at most 100 items"));
    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(&file)).unwrap(),
        "target\n"
    );
}
