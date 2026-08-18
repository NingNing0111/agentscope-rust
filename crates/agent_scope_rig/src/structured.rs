//! 结构化输出：原生 `output_schema` 路径 + 工具 bypass 回退（T019）。
//!
//! 契约见 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §6：
//! `generate_structured_output(messages, structured_model)` 由 `RigChatModel`
//! 的 `ChatModel` 覆写实现调用（orchestration 在 lib.rs，rig 类型访问集中在
//! 本模块）。本模块提供请求构建与响应提取的纯函数：
//! - [`native_output_request`]：`flatten_json_schema_with_defs_checked` 产物 →
//!   `CompletionRequest.output_schema`（schemars 1.x `Schema`，rig 原生路径）。
//! - [`bypass_output_request`]：注入 `generate_structured_output` 工具 +
//!   `tool_choice=required`（trait 默认 bypass，provider 不支持原生路径时回退）。
//! - [`extract_structured`]：从 assistant 内容提取 JSON（`json_repair` 兜底）。
//! - [`extract_tool_bypass`]：从工具调用 arguments 提取 JSON。

use rig::completion::message::AssistantContent;
use rig::completion::{CompletionRequest, Message};
use rig::core::OneOrMany;

use agent_scope_message::Msg;
use agent_scope_model::FormatError;
use agent_scope_model::json_repair::json_repair;
use agent_scope_model::model_error::ModelError;
use agent_scope_model::response::StructuredResponse;
use agent_scope_model::tool_choice::ToolChoice;

use crate::message::msg_to_rig_messages;
use crate::params::RigParameters;
use crate::tools::tool_choice_to_rig;

/// 构建原生 `output_schema` 请求（契约 §6.1）。
///
/// `json_schema` 必须是 `flatten_json_schema_with_defs_checked` 的产物（标准
/// JSON Schema 对象）；无法转换为 schemars 1.x `Schema` → `FormatError`（调用方
/// 视此为"原生路径不可用"，回退工具 bypass）。
pub(crate) fn native_output_request(
    model: &str,
    messages: &[Msg],
    json_schema: &serde_json::Value,
    params: &RigParameters,
) -> Result<CompletionRequest, ModelError> {
    let chat_history = to_chat_history(messages)?;
    let schema: schemars_1::Schema =
        serde_json::from_value(json_schema.clone()).map_err(|e| ModelError::FormatError {
            context: "rig:structured".to_string(),
            source: FormatError::InvalidMessage(format!(
                "invalid JSON Schema for output_schema: {e}"
            )),
        })?;
    let request = CompletionRequest {
        model: Some(model.to_string()),
        preamble: None,
        chat_history,
        documents: Vec::new(),
        tools: Vec::new(),
        temperature: None,
        max_tokens: None,
        tool_choice: None,
        additional_params: None,
        output_schema: Some(schema),
        record_telemetry_content: false,
    };
    Ok(crate::params::apply_params(request, params))
}

/// 构建工具 bypass 请求（契约 §6.2，同 trait 默认注入方式）。
///
/// 注入 `generate_structured_output` 工具（parameters=目标 JSON Schema）+ 强制
/// `tool_choice=required`；provider 不支持原生 `output_schema` 时回退至此。
pub(crate) fn bypass_output_request(
    model: &str,
    messages: &[Msg],
    json_schema: &serde_json::Value,
    params: &RigParameters,
) -> Result<CompletionRequest, ModelError> {
    let chat_history = to_chat_history(messages)?;
    let tool_schema = serde_json::json!({
        "type": "function",
        "function": {
            "name": "generate_structured_output",
            "description": "Generate structured output matching the required schema.",
            "parameters": json_schema,
        }
    });
    let tool_defs = crate::tools::json_schema_to_tool_definitions(&[tool_schema])?;
    let request = CompletionRequest {
        model: Some(model.to_string()),
        preamble: None,
        chat_history,
        documents: Vec::new(),
        tools: tool_defs,
        temperature: None,
        max_tokens: None,
        tool_choice: tool_choice_to_rig(&ToolChoice::required()),
        additional_params: None,
        output_schema: None,
        record_telemetry_content: false,
    };
    Ok(crate::params::apply_params(request, params))
}

/// 从原生路径响应提取 JSON（契约 §6.1：取符合 schema 的文本/JSON）。
///
/// 扫描 `AssistantContent` 中的全部 `Text` 并拼接；空/无 JSON → `StructuredOutputError`
/// （调用方视为原生路径未产出结构化结果，回退工具 bypass）。JSON 解析失败时
/// 用 `json_repair` 兜底（契约 §6.2/§6.4）。
pub(crate) fn extract_structured(
    choice: &[AssistantContent],
    usage: &rig::completion::Usage,
    message_id: Option<String>,
) -> Result<StructuredResponse, ModelError> {
    let json =
        extract_json_from_choice(choice).ok_or_else(|| ModelError::StructuredOutputError {
            reason: "No structured JSON found in response".to_string(),
        })?;
    let mut resp = StructuredResponse {
        content: json,
        usage: Some(crate::stream::rig_usage_to_chat_usage(usage)),
        ..Default::default()
    };
    if let Some(mid) = message_id {
        resp.metadata
            .insert("message_id".to_string(), serde_json::json!(mid));
    }
    Ok(resp)
}

/// 从工具 bypass 响应提取 JSON（契约 §6.2）。
///
/// 取第一个 `ToolCall.function.arguments`（rig 已解析为 `Value`；个别 provider
/// 以字符串返回时 `json_repair` 兜底）。无工具调用 → `StructuredOutputError`。
pub(crate) fn extract_tool_bypass(
    choice: &[AssistantContent],
    usage: &rig::completion::Usage,
    message_id: Option<String>,
) -> Result<StructuredResponse, ModelError> {
    let arguments = choice
        .iter()
        .find_map(|c| match c {
            AssistantContent::ToolCall(tc) => Some(tc.function.arguments.clone()),
            _ => None,
        })
        .ok_or_else(|| ModelError::StructuredOutputError {
            reason: "No tool call found in structured-output response".to_string(),
        })?;

    let parsed = match arguments {
        serde_json::Value::String(s) => serde_json::from_str(&s)
            .or_else(|_| serde_json::from_str(&json_repair(&s)))
            .map_err(|e| ModelError::StructuredOutputError {
                reason: format!("Failed to parse tool call input as JSON: {e}"),
            })?,
        value => value,
    };

    let mut resp = StructuredResponse {
        content: parsed,
        usage: Some(crate::stream::rig_usage_to_chat_usage(usage)),
        ..Default::default()
    };
    if let Some(mid) = message_id {
        resp.metadata
            .insert("message_id".to_string(), serde_json::json!(mid));
    }
    Ok(resp)
}

// ---------------------------------------------------------------------------
// 内部工具
// ---------------------------------------------------------------------------

/// 消息 → rig `chat_history`（空 → `ValidationError`）。
fn to_chat_history(messages: &[Msg]) -> Result<OneOrMany<Message>, ModelError> {
    let rig_messages = msg_to_rig_messages(messages)?;
    OneOrMany::from_iter_optional(rig_messages).ok_or_else(|| ModelError::ValidationError {
        field: "messages".to_string(),
        message: "messages must not be empty".to_string(),
    })
}

/// 从 assistant 内容拼接文本并解析 JSON（`json_repair` 兜底）。
fn extract_json_from_choice(choice: &[AssistantContent]) -> Option<serde_json::Value> {
    let text: String = choice
        .iter()
        .filter_map(|c| match c {
            AssistantContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect();
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    serde_json::from_str(text).ok().or_else(|| {
        let repaired = json_repair(text);
        serde_json::from_str(&repaired).ok()
    })
}
