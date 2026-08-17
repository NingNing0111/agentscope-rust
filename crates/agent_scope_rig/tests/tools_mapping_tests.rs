//! T011 — 工具映射测试（schema → `ToolDefinition`、`ToolChoice` 四模式 + 子集过滤）。
//!
//! 对照 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §2。

use agent_scope_model::ModelError;
use agent_scope_model::tool_choice::ToolChoice;
use agent_scope_rig::tools::{
    filter_tool_definitions, json_schema_to_tool_definitions, tool_choice_to_rig,
};
use rig::completion::ToolDefinition;
use rig::completion::message::ToolChoice as RigToolChoice;

fn schema(name: &str, description: &str, parameters: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": parameters,
        }
    })
}

// ---------------------------------------------------------------------------
// 2.1 schema → ToolDefinition
// ---------------------------------------------------------------------------

#[test]
fn schema_converts_to_tool_definition() {
    let schemas = vec![schema(
        "search",
        "search the web",
        serde_json::json!({"type": "object", "properties": {"q": {"type": "string"}}}),
    )];
    let defs = json_schema_to_tool_definitions(&schemas).unwrap();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "search");
    assert_eq!(defs[0].description, "search the web");
    assert_eq!(
        defs[0].parameters,
        serde_json::json!({"type": "object", "properties": {"q": {"type": "string"}}})
    );
}

#[test]
fn empty_schemas_yield_empty_definitions() {
    let defs = json_schema_to_tool_definitions(&[]).unwrap();
    assert!(defs.is_empty());
}

#[test]
fn missing_function_wrapper_errors() {
    let schemas = vec![serde_json::json!({"type": "function", "name": "orphan"})];
    let err = json_schema_to_tool_definitions(&schemas).unwrap_err();
    assert!(
        matches!(err, ModelError::FormatError { .. }),
        "expected FormatError, got {err:?}"
    );
}

#[test]
fn missing_name_errors() {
    let schemas = vec![serde_json::json!({
        "type": "function",
        "function": {"description": "no name"}
    })];
    let err = json_schema_to_tool_definitions(&schemas).unwrap_err();
    assert!(matches!(err, ModelError::FormatError { .. }));
}

#[test]
fn missing_description_and_parameters_get_defaults() {
    let schemas = vec![serde_json::json!({
        "type": "function",
        "function": {"name": "bare"}
    })];
    let defs = json_schema_to_tool_definitions(&schemas).unwrap();
    assert_eq!(defs[0].name, "bare");
    assert_eq!(defs[0].description, "");
    assert_eq!(
        defs[0].parameters,
        serde_json::json!({"type": "object", "properties": {}})
    );
}

// ---------------------------------------------------------------------------
// 2.2 ToolChoice → rig tool_choice
// ---------------------------------------------------------------------------

#[test]
fn auto_maps_to_none() {
    assert_eq!(tool_choice_to_rig(&ToolChoice::auto()), None);
}

#[test]
fn none_maps_to_none_choice() {
    assert_eq!(
        tool_choice_to_rig(&ToolChoice::none()),
        Some(RigToolChoice::None)
    );
}

#[test]
fn required_maps_to_required() {
    assert_eq!(
        tool_choice_to_rig(&ToolChoice::required()),
        Some(RigToolChoice::Required)
    );
}

#[test]
fn specific_tool_maps_to_function_names() {
    assert_eq!(
        tool_choice_to_rig(&ToolChoice::specific_tool("search")),
        Some(RigToolChoice::Specific {
            function_names: vec!["search".to_string()],
        })
    );
}

#[test]
fn tools_filter_passes_through_for_auto() {
    // `with_tools("auto", ["search"])` — mode 仍是 auto，tool_choice 为 None；
    // 子集过滤由 `filter_tool_definitions` 单独处理。
    let tc = ToolChoice::with_tools("auto", vec!["search".to_string()]);
    assert_eq!(tool_choice_to_rig(&tc), None);
}

// ---------------------------------------------------------------------------
// 2.2 tools 子集过滤
// ---------------------------------------------------------------------------

#[test]
fn filter_keeps_only_listed_tools() {
    let defs = vec![
        ToolDefinition {
            name: "search".to_string(),
            description: "s".to_string(),
            parameters: serde_json::json!({}),
        },
        ToolDefinition {
            name: "calc".to_string(),
            description: "c".to_string(),
            parameters: serde_json::json!({}),
        },
    ];
    let filtered = filter_tool_definitions(&defs, Some(&["search".to_string()]));
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "search");
}

#[test]
fn filter_none_keeps_all() {
    let defs = vec![
        ToolDefinition {
            name: "search".to_string(),
            description: "s".to_string(),
            parameters: serde_json::json!({}),
        },
        ToolDefinition {
            name: "calc".to_string(),
            description: "c".to_string(),
            parameters: serde_json::json!({}),
        },
    ];
    let filtered = filter_tool_definitions(&defs, None);
    assert_eq!(filtered.len(), 2);
}

#[test]
fn filter_unknown_names_yields_empty() {
    let defs = vec![ToolDefinition {
        name: "search".to_string(),
        description: "s".to_string(),
        parameters: serde_json::json!({}),
    }];
    let filtered = filter_tool_definitions(&defs, Some(&["nope".to_string()]));
    assert!(filtered.is_empty());
}
