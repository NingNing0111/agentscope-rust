//! OpenAI function schema → rig `ToolDefinition`；`ToolChoice` → rig `tool_choice`（T007）。
//!
//! 契约见 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §2。
//! 已记录偏差：契约 §2.1 记 `ToolDefinition{..., strict: false}`，rig 0.41.0 无
//! `strict` 字段；契约 §2.2 记 `ToolChoice::Specific(name)`，0.41 为
//! `Specific{function_names: Vec<String>}`。

use rig::completion::ToolDefinition;
use rig::completion::message::ToolChoice as RigToolChoice;

use agent_scope_model::FormatError;
use agent_scope_model::model_error::ModelError;
use agent_scope_model::tool_choice::ToolChoice;

/// OpenAI function-calling schema → rig `ToolDefinition`（契约 §2.1）。
///
/// 输入为 `{"type":"function","function":{"name","description","parameters"}}`；
/// 缺 `function` 包裹或 `function.name` → `FormatError`。`description`/`parameters`
/// 缺失时分别按空串 / 空对象处理（`tools` 数组为空 → 调用方不设置
/// `CompletionRequest.tools`）。
pub fn json_schema_to_tool_definitions(
    schemas: &[serde_json::Value],
) -> Result<Vec<ToolDefinition>, ModelError> {
    let mut out = Vec::with_capacity(schemas.len());
    for schema in schemas {
        let func = schema
            .get("function")
            .ok_or_else(|| ModelError::FormatError {
                context: "rig:tool-schema".to_string(),
                source: FormatError::InvalidMessage(
                    "tool schema missing 'function' wrapper".to_string(),
                ),
            })?;
        let name = func
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| ModelError::FormatError {
                context: "rig:tool-schema".to_string(),
                source: FormatError::InvalidMessage("tool function missing 'name'".to_string()),
            })?
            .to_string();
        let description = func
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or_default()
            .to_string();
        let parameters = func
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({ "type": "object", "properties": {} }));
        out.push(ToolDefinition {
            name,
            description,
            parameters,
        });
    }
    Ok(out)
}

/// `ToolChoice` → rig `tool_choice`（契约 §2.2）。
///
/// | agent_scope `ToolChoice` | rig `tool_choice` |
/// |--------------------------|-------------------|
/// | `mode=auto` | `None`（rig 默认 auto） |
/// | `mode=none` | `Some(ToolChoice::None)` |
/// | `mode=required` | `Some(ToolChoice::Required)` |
/// | 具体工具名 | `Some(ToolChoice::Specific{function_names: vec![name]})` |
///
/// `mode=required` 在 `thinking_tool_choice_incompatible` 时的降级由调用方处理
/// （T030，不在此函数内静默）。
pub fn tool_choice_to_rig(tc: &ToolChoice) -> Option<RigToolChoice> {
    match tc.mode.as_str() {
        "auto" => None,
        "none" => Some(RigToolChoice::None),
        "required" => Some(RigToolChoice::Required),
        name => Some(RigToolChoice::Specific {
            function_names: vec![name.to_string()],
        }),
    }
}

/// 按 `ToolChoice.tools` 过滤工具定义（延续 round-4 M18 语义；契约 §2.2 末行）。
///
/// `filter` 为 `None` 时不过滤；`Some(names)` 时仅保留名字在 `names` 内的工具。
pub fn filter_tool_definitions(
    defs: &[ToolDefinition],
    filter: Option<&[String]>,
) -> Vec<ToolDefinition> {
    match filter {
        Some(names) => defs
            .iter()
            .filter(|d| names.contains(&d.name))
            .cloned()
            .collect(),
        None => defs.to_vec(),
    }
}
