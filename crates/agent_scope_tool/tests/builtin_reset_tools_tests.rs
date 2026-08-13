//! US3 — ResetTools meta-tool (FR-019, SC-004).
//!
//! Mirrors quickstart scenario 4 and contracts/reset-tools.md.

mod common;

use agent_scope_message::ToolResultState;
use agent_scope_tool::Tool;
use agent_scope_tool::builtin::ResetToolsTool;

use common::{ctx_in, text_of};

#[tokio::test]
async fn reset_tools_activates_requested_groups() {
    let h = ctx_in(&["coding", "docs"]);
    let tool = ResetToolsTool::new(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({ "coding": true, "docs": false }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(block.state, ToolResultState::Success);
    let groups = h
        .session
        .read()
        .unwrap()
        .list_groups()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(groups, vec!["coding".to_string()]);
}

#[tokio::test]
async fn reset_tools_final_state_semantics() {
    let h = ctx_in(&["coding", "docs"]);
    let tool = ResetToolsTool::new(h.ctx.clone());
    tool.call(serde_json::json!({ "coding": true, "docs": true }))
        .await
        .unwrap();
    // Re-record with only docs → coding deactivated (final state, not incremental).
    tool.call(serde_json::json!({ "docs": true }))
        .await
        .unwrap();
    let groups = h
        .session
        .read()
        .unwrap()
        .list_groups()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(groups, vec!["docs".to_string()]);
}

#[tokio::test]
async fn reset_tools_unauthorized_group_rejected() {
    let h = ctx_in(&["coding"]);
    let tool = ResetToolsTool::new(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({ "admin": true }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(block.state, ToolResultState::Error);
    assert!(
        text_of(&block).contains("permission_denied"),
        "got: {}",
        text_of(&block)
    );
    // Nothing outside the boundary was activated.
    assert!(!h.session.read().unwrap().is_group_active("admin"));
}

#[tokio::test]
async fn reset_tools_non_bool_argument_rejected() {
    let h = ctx_in(&["coding"]);
    let tool = ResetToolsTool::new(h.ctx.clone());

    let out = tool
        .call(serde_json::json!({ "coding": "yes" }))
        .await
        .unwrap();
    let block = match out {
        agent_scope_tool::ToolExecOutput::Complete(b) => b,
        _ => panic!("expected Complete"),
    };
    assert_eq!(block.state, ToolResultState::Error);
    assert!(text_of(&block).contains("invalid_arguments"));
}

#[tokio::test]
async fn reset_tools_schema_is_dynamic() {
    let h = ctx_in(&["coding", "docs"]);
    let tool = ResetToolsTool::new(h.ctx.clone());
    let schema = tool.input_schema();
    assert!(schema["properties"]["coding"]["type"] == "boolean");
    assert!(schema["properties"]["docs"]["type"] == "boolean");
    // basic is never a dynamic field.
    assert!(schema["properties"].get("basic").is_none());
}
