//! Feature 029 — built-in tool schema compatibility against the vendored
//! Python reference (Constitution Art.1/Art.3/Art.6).
//!
//! Loads the golden snapshot `tests/compatibility/fixtures/builtin_tool_schemas.json`
//! (transcribed from `agentscope/tool/_builtin/`, upstream `9d1026fa`) and
//! asserts each built-in tool's live `input_schema()` matches it structurally:
//! tool name, required parameters, and every property's JSON type.

mod common;

use std::collections::BTreeMap;

use agent_scope_tool::Tool;
use agent_scope_tool::builtin::{
    BashTool, BuiltInToolContext, EditTool, GlobTool, GrepTool, PowerShellTool, ReadTool,
    ResetToolsTool, SkillTool, WriteTool,
};

use common::ctx_in;

/// Fixture path relative to the workspace root.
const FIXTURE: &str = "tests/compatibility/fixtures/builtin_tool_schemas.json";

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn load_fixture() -> serde_json::Value {
    let path = repo_root().join(FIXTURE);
    let text = std::fs::read_to_string(&path).expect("read builtin_tool_schemas.json fixture");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parse fixture");
    value["data"].clone()
}

/// Build every built-in tool bound to a fresh context, returning name → tool.
///
/// Uses an empty authorized-group set so `ResetTools`'s dynamic schema matches
/// the golden baseline (no non-basic groups → no boolean fields).
fn all_tools() -> Vec<(&'static str, Box<dyn Tool>)> {
    let h = ctx_in(&[]);
    let ctx = h.ctx;
    let ctx: BuiltInToolContext = ctx;
    let tools: Vec<(Box<dyn Tool>, &'static str)> = vec![
        (Box::new(BashTool::new(ctx.clone())), "Bash"),
        (Box::new(ReadTool::new(ctx.clone())), "Read"),
        (Box::new(EditTool::new(ctx.clone())), "Edit"),
        (Box::new(WriteTool::new(ctx.clone())), "Write"),
        (Box::new(GrepTool::new(ctx.clone())), "Grep"),
        (Box::new(GlobTool::new(ctx.clone())), "Glob"),
        (Box::new(PowerShellTool::new(ctx.clone())), "PowerShell"),
        (Box::new(ResetToolsTool::new(ctx.clone())), "ResetTools"),
        (
            Box::new(SkillTool::new(
                ctx,
                Box::new(|_| std::collections::HashMap::new()),
            )),
            "Skill",
        ),
    ];
    tools
        .into_iter()
        .map(|(t, name)| {
            assert_eq!(t.name(), name, "tool name mismatch");
            (name, t)
        })
        .collect()
}

/// Extract `{property: json_type}` from a tool's input schema.
fn property_types(schema: &serde_json::Value) -> BTreeMap<String, String> {
    schema["properties"]
        .as_object()
        .unwrap_or(&serde_json::Map::new())
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?")
                    .to_string(),
            )
        })
        .collect()
}

fn required(schema: &serde_json::Value) -> Vec<String> {
    schema["required"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn builtin_tool_schemas_match_python_golden() {
    let expected = load_fixture();
    for (name, tool) in all_tools() {
        let schema = tool.input_schema();
        let entry = &expected[name];

        // Required parameters match.
        assert_eq!(
            required(&schema),
            required(entry),
            "{name}: required mismatch\n  rust:    {:?}\n  fixture: {:?}",
            required(&schema),
            required(entry)
        );

        // Every property name + JSON type matches (fixture may be a subset
        // that the Rust implementation must cover exactly).
        let rust_props = property_types(&schema);
        let expected_props: BTreeMap<String, String> = entry["properties"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("?").to_string()))
            .collect();
        assert_eq!(rust_props, expected_props, "{name}: property/type mismatch");
    }
}

#[test]
fn builtin_tool_schema_names_are_stable() {
    // The set of injected built-in tool names (SC-001, FR-022).
    let expected_names = [
        "Bash",
        "Read",
        "Edit",
        "Write",
        "Grep",
        "Glob",
        "PowerShell",
        "ResetTools",
        "Skill",
    ];
    let names: Vec<&str> = all_tools().iter().map(|(n, _)| *n).collect();
    for n in expected_names {
        assert!(names.contains(&n), "missing tool {n}");
    }
}
