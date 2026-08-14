//! Tool example: custom `FunctionTool`s + `ToolKit` registration + schema output.
//!
//! This example needs no model or API key — it shows how a plain async Rust
//! function becomes a tool with an auto-generated JSON Schema (via `schemars`),
//! how tools are registered in a `ToolKit`, and how a tool call is executed
//! directly with JSON arguments.

use agent_scope_tool::{FunctionTool, Tool, ToolKit};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

/// Arguments for the calculator tool — only needs `Deserialize` + `JsonSchema`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput {
    /// A simple math expression, e.g. "2 + 2".
    expression: String,
}

async fn calculator(input: CalcInput) -> String {
    // A real implementation would use an eval library; this keeps the demo
    // dependency-free and deterministic.
    format!("calced: {}", input.expression)
}

/// Arguments for a read-file tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ReadInput {
    /// Absolute path of the file to read.
    path: String,
}

async fn read_file(input: ReadInput) -> String {
    match tokio::fs::read_to_string(&input.path).await {
        Ok(text) => format!("{} bytes:\n{}", text.len(), text),
        Err(err) => format!("read error: {err}"),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Build tools from plain async functions.
    let calc = FunctionTool::new("calculator", "Evaluate a math expression.", calculator);
    let read = FunctionTool::new("read_file", "Read a text file from disk.", read_file);

    // 2. Register them in a ToolKit.
    let mut toolkit = ToolKit::new();
    toolkit.register(calc);
    toolkit.register(read);
    println!(
        "registered tools: {} (contains calculator: {})",
        toolkit.len(),
        toolkit.contains("calculator")
    );

    // 3. Inspect the auto-generated, OpenAI-compatible JSON schemas.
    println!("\n--- tool schemas ---");
    for schema in toolkit.get_tool_schemas() {
        println!("{schema}");
    }

    // 4. Call a tool directly with JSON arguments.
    println!("\n--- direct calls ---");
    let calc = FunctionTool::new("calculator", "Evaluate a math expression.", calculator);
    let out = calc.call(json!({ "expression": "6 * 7" })).await?;
    println!("calculator(6 * 7) = {out:?}");

    let read = FunctionTool::new("read_file", "Read a text file from disk.", read_file);
    let out = read.call(json!({ "path": "/nonexistent" })).await?;
    println!("read_file(/nonexistent) = {out:?}");

    Ok(())
}
