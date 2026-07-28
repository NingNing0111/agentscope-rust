//! Integration tests: Tool System ↔ ChatModel (US3).
//!
//! Verifies that [`ToolKit::get_tool_schemas`] output can be passed directly
//! to [`ChatModel::call`] and that [`ToolCallBlock`] → [`ToolKit::call_tool`]
//! forms a closed loop.

use agent_scope_message::{ToolCallBlock, ToolOutput, ToolResultBlock, ToolResultState};
use agent_scope_model::tool_choice::ToolChoice;
use agent_scope_tool::{FunctionTool, ToolExecOutput, ToolKit};
use schemars::JsonSchema;
use serde::Deserialize;

/// Shared input type.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SearchInput {
    query: String,
}

async fn search_handler(input: SearchInput) -> String {
    format!("found: {}", input.query)
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput {
    a: i32,
    b: i32,
    op: String,
}

async fn calc_handler(input: CalcInput) -> String {
    match input.op.as_str() {
        "add" => format!("{}", input.a + input.b),
        "mul" => format!("{}", input.a * input.b),
        _ => "unknown op".into(),
    }
}

fn make_toolkit() -> ToolKit {
    let mut tk = ToolKit::new();
    tk.register(FunctionTool::new(
        "search",
        "Search for things",
        search_handler,
    ));
    tk.register(FunctionTool::new("calc", "Do math", calc_handler));
    tk
}

// -- T036: Schema format compatible with ChatModel::call() tools param --
#[tokio::test]
async fn test_tool_schema_compatible_with_chat_model_call_signature() {
    let tk = make_toolkit();
    let schemas = tk.get_tool_schemas();

    // ChatModel::call_api takes &[JsonValue] for tools — verify type matches
    let _tools: &[serde_json::Value] = &schemas;

    // Verify structure is what models expect
    for schema in &schemas {
        assert_eq!(schema["type"], "function");
        let func = &schema["function"];
        assert!(func["name"].is_string());
        assert!(func["description"].is_string());
        assert!(func["parameters"].is_object());
    }
}

// -- T037: ToolCallBlock → ToolKit::call_tool closed loop --
#[tokio::test]
async fn test_toolcall_block_to_call_tool_closed_loop() {
    let tk = make_toolkit();

    // Simulate: model returns ToolCallBlock →
    //           developer calls ToolKit::call_tool →
    //           gets ToolExecOutput::Complete
    let tc = ToolCallBlock::new(
        "tc-calc-1".into(),
        "calc".into(),
        r#"{"a":10,"b":32,"op":"add"}"#.into(),
    );

    let result = tk.call_tool(&tc).await.unwrap();
    match result {
        ToolExecOutput::Complete(chunk) => {
            assert_eq!(chunk.state, ToolResultState::Success);
            assert!(chunk.is_last);
            match &chunk.output {
                ToolOutput::Text(text) => {
                    assert_eq!(text, "42");
                }
                _ => panic!("Expected Text output"),
            }
        }
        _ => panic!("Expected Complete"),
    }
}

// -- T038: ToolChoice::validate() works with toolkit schema output --
#[tokio::test]
async fn test_toolchoice_validate_with_toolkit_schemas() {
    let tk = make_toolkit();

    let tool_names: Vec<String> = tk
        .get_tool_schemas()
        .iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .map(|s| s.to_string())
        .collect();

    // Valid: specific tool
    let tc = ToolChoice::specific_tool("search");
    assert!(tc.validate(Some(&tool_names)).is_ok());

    // Valid: auto
    let tc = ToolChoice::auto();
    assert!(tc.validate(Some(&tool_names)).is_ok());

    // Invalid: missing tool
    let tc = ToolChoice::specific_tool("nonexistent");
    assert!(tc.validate(Some(&tool_names)).is_err());
}

// -- T039: ToolResultBlock serde round-trip with is_last field --
#[test]
fn test_tool_result_block_is_last_serde_roundtrip() {
    // With is_last = true
    let block = ToolResultBlock {
        id: "test-id".into(),
        name: "test-tool".into(),
        output: ToolOutput::Text("result".into()),
        state: ToolResultState::Success,
        is_last: true,
        metadata: std::collections::HashMap::new(),
        created_at: chrono::Utc::now().to_rfc3339(),
        finished_at: None,
    };

    let json = serde_json::to_string(&block).unwrap();
    assert!(json.contains(r#""is_last":true"#));

    let restored: ToolResultBlock = serde_json::from_str(&json).unwrap();
    assert!(restored.is_last);

    // Old JSON (no is_last field) → deserializes with is_last = false
    let old_json = r#"{"id":"x","name":"y","output":"text","state":"success","created_at":"2024-01-01T00:00:00+00:00"}"#;
    let old: ToolResultBlock = serde_json::from_str(old_json).unwrap();
    assert!(!old.is_last, "is_last should default to false");
}

// -- T040: Existing tests still pass (sanity check) --
#[tokio::test]
async fn test_existing_message_crate_patterns_still_work() {
    // This is a basic sanity test that proves ToolResultBlock with the new
    // is_last field still works the same way as before.
    let tc = ToolCallBlock::new("tc-1".into(), "search".into(), r#"{"query":"test"}"#.into());
    assert_eq!(tc.name, "search");
    assert_eq!(tc.input, r#"{"query":"test"}"#);

    let tr = ToolResultBlock::new(
        "tr-1".into(),
        "search".into(),
        ToolOutput::Text("done".into()),
    );
    // State is Running by default (existing behaviour)
    assert_eq!(tr.state, ToolResultState::Running);
    // is_last defaults to false (backward compat)
    assert!(!tr.is_last);
}
