//! DashScopeFormatter — Msg → DashScope OpenAI-compatible API format.

use agent_scope_message::{ContentBlock, DataSource, Msg, Role, ToolOutput};
use serde_json::Value as JsonValue;

use agent_scope_model::formatter::{FormatError, Formatter};

/// Formatter that converts AgentScope Msg objects to DashScope OpenAI-compatible format.
#[derive(Debug, Clone)]
pub struct DashScopeFormatter {
    pub input_types: Vec<String>,
}

impl Default for DashScopeFormatter {
    fn default() -> Self {
        Self {
            input_types: vec!["text/plain".to_string()],
        }
    }
}

impl Formatter for DashScopeFormatter {
    fn supported_input_media_types(&self) -> &[String] {
        &self.input_types
    }

    fn format(&self, msgs: &[Msg]) -> Result<Vec<JsonValue>, FormatError> {
        let mut result = Vec::new();

        for msg in msgs {
            let mut entry = serde_json::Map::new();

            // ── Tool result handling ──
            // Tool result messages MUST use role="tool", have a tool_call_id,
            // and content as a string. The OpenAI-compatible API rejects
            // tool-role messages without a tool_call_id.
            let tool_results: Vec<&agent_scope_message::ToolResultBlock> = msg
                .content
                .iter()
                .filter_map(|b| {
                    if let ContentBlock::ToolResult(tr) = b {
                        Some(tr)
                    } else {
                        None
                    }
                })
                .collect();

            if !tool_results.is_empty() {
                // Role is always "tool" for tool result messages
                entry.insert("role".to_string(), JsonValue::String("tool".to_string()));

                // tool_call_id from the first ToolResultBlock's id
                // (react_loop sets ToolResultBlock.id = ToolCallBlock.id)
                entry.insert(
                    "tool_call_id".to_string(),
                    JsonValue::String(tool_results[0].id.clone()),
                );

                // Content must be a string for tool role messages
                let (text_content, _promoted) =
                    self.convert_tool_result_to_string(&tool_results[0].output)?;
                entry.insert("content".to_string(), JsonValue::String(text_content));

                // If there are additional ToolResult blocks, also format them
                // as separate entries (each tool result is a separate message
                // in the OpenAI protocol)
                for tr in &tool_results[1..] {
                    let mut extra_entry = serde_json::Map::new();
                    extra_entry.insert("role".to_string(), JsonValue::String("tool".to_string()));
                    extra_entry
                        .insert("tool_call_id".to_string(), JsonValue::String(tr.id.clone()));
                    let (extra_text, _extra_promoted) =
                        self.convert_tool_result_to_string(&tr.output)?;
                    extra_entry.insert("content".to_string(), JsonValue::String(extra_text));
                    // No `name` on role=tool entries: OpenAI-compatible APIs
                    // reject it (see the guard on the first entry), so the
                    // extra entries must stay consistent with it (round-4 M22).
                    result.push(JsonValue::Object(extra_entry));
                }
            } else {
                // ── Normal message formatting (no tool results) ──
                let role_str = match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => "system",
                };
                entry.insert("role".to_string(), JsonValue::String(role_str.to_string()));

                // Handle tool calls (assistant messages) — must be checked
                // BEFORE content formatting so we can set content=null when
                // the message has tool calls but no text blocks.
                if msg.role == Role::Assistant {
                    let tool_calls: Vec<&ContentBlock> = msg
                        .content
                        .iter()
                        .filter(|b| matches!(b, ContentBlock::ToolCall(_)))
                        .collect();
                    if !tool_calls.is_empty() {
                        let tc_array: Vec<JsonValue> = tool_calls
                            .iter()
                            .filter_map(|b| {
                                if let ContentBlock::ToolCall(tc) = b {
                                    Some(serde_json::json!({
                                        "id": tc.id,
                                        "type": "function",
                                        "function": {
                                            "name": tc.name,
                                            "arguments": tc.input
                                        }
                                    }))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        entry.insert("tool_calls".to_string(), JsonValue::Array(tc_array));

                        // Check for text blocks. If there are none, set
                        // content=null per OpenAI-compatible API spec.
                        // FR-002: pure tool_call assistant messages MUST use
                        // null content, not empty string or empty array.
                        let has_text = msg
                            .content
                            .iter()
                            .any(|b| matches!(b, ContentBlock::Text(_)));
                        if has_text {
                            let content = self.format_content(&msg.content)?;
                            entry.insert("content".to_string(), content);
                        } else {
                            entry.insert("content".to_string(), JsonValue::Null);
                        }
                    } else {
                        let content = self.format_content(&msg.content)?;
                        entry.insert("content".to_string(), content);
                    }
                } else {
                    let content = self.format_content(&msg.content)?;
                    entry.insert("content".to_string(), content);
                }
            }

            // Handle name field — tool-role messages should not include a name.
            // OpenAI-compatible APIs reject name fields on role=tool messages.
            if !msg.name.is_empty() {
                let is_tool_role = msg
                    .content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolResult(_)));
                if !is_tool_role {
                    entry.insert("name".to_string(), JsonValue::String(msg.name.clone()));
                }
            }

            result.push(JsonValue::Object(entry));
        }

        Ok(result)
    }

    fn convert_tool_result_to_string(
        &self,
        output: &ToolOutput,
    ) -> Result<(String, Vec<ContentBlock>), FormatError> {
        match output {
            ToolOutput::Text(text) => Ok((text.clone(), vec![])),
            ToolOutput::Blocks(blocks) => {
                let mut text_parts = Vec::new();
                let mut promoted = Vec::new();
                for item in blocks {
                    match item {
                        agent_scope_message::ToolResultBlockItem::Text(tb) => {
                            text_parts.push(tb.text.clone());
                        }
                        agent_scope_message::ToolResultBlockItem::Data(db) => match &db.source {
                            DataSource::Url(url_src) => {
                                let media_type = url_src.media_type.as_str();
                                let is_supported = self.input_types.iter().any(|t| {
                                    if t.ends_with("/*") {
                                        let prefix = t.trim_end_matches('*');
                                        media_type.starts_with(prefix)
                                    } else {
                                        t == media_type
                                    }
                                });
                                if is_supported {
                                    let short_id = uuid::Uuid::new_v4().as_simple().to_string();
                                    text_parts.push(format!(
                                        "<system-reminder>Image available: block_id={short_id}</system-reminder>"
                                    ));
                                    let mut promoted_db = db.clone();
                                    promoted_db.id = short_id;
                                    promoted.push(ContentBlock::Data(promoted_db));
                                } else {
                                    text_parts.push(url_src.url.clone());
                                }
                            }
                            DataSource::Base64(bs) => {
                                let is_supported = self.input_types.iter().any(|t| {
                                    if t.ends_with("/*") {
                                        bs.media_type.starts_with(t.trim_end_matches('*'))
                                    } else {
                                        t == &bs.media_type
                                    }
                                });
                                if is_supported {
                                    let short_id = uuid::Uuid::new_v4().as_simple().to_string();
                                    text_parts.push(format!(
                                        "<system-reminder>Media available: block_id={short_id}</system-reminder>"
                                    ));
                                    let mut promoted_db = db.clone();
                                    promoted_db.id = short_id;
                                    promoted.push(ContentBlock::Data(promoted_db));
                                } else {
                                    text_parts
                                        .push(format!("[base64 data, type={}]", bs.media_type));
                                }
                            }
                        },
                    }
                }
                Ok((text_parts.join("\n"), promoted))
            }
        }
    }
}

impl DashScopeFormatter {
    /// Format content blocks into DashScope API content format.
    fn format_content(&self, blocks: &[ContentBlock]) -> Result<JsonValue, FormatError> {
        let text_blocks: Vec<&ContentBlock> = blocks
            .iter()
            .filter(|b| matches!(b, ContentBlock::Text(_)))
            .collect();

        let has_non_text = blocks.iter().any(|b| !matches!(b, ContentBlock::Text(_)));

        if !has_non_text {
            let texts: Vec<&str> = text_blocks
                .iter()
                .filter_map(|b| {
                    if let ContentBlock::Text(tb) = b {
                        Some(tb.text.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            Ok(JsonValue::String(texts.join("")))
        } else {
            let mut parts = Vec::new();
            for block in blocks {
                match block {
                    ContentBlock::Text(tb) => {
                        parts.push(serde_json::json!({"type": "text", "text": tb.text}));
                    }
                    ContentBlock::Data(db) => match &db.source {
                        DataSource::Url(url_src) => {
                            parts.push(serde_json::json!({
                                "type": "image_url",
                                "image_url": {"url": url_src.url, "detail": "auto"}
                            }));
                        }
                        DataSource::Base64(bs) => {
                            let data_url = format!("data:{};base64,{}", bs.media_type, bs.data);
                            if bs.media_type.starts_with("image/") {
                                parts.push(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": {"url": data_url, "detail": "auto"}
                                }));
                            } else if bs.media_type.starts_with("audio/") {
                                parts.push(serde_json::json!({
                                    "type": "input_audio",
                                    "input_audio": {"data": bs.data, "format": bs.media_type}
                                }));
                            } else {
                                parts.push(serde_json::json!({
                                    "type": "file", "file": {"file_data": data_url}
                                }));
                            }
                        }
                    },
                    _ => {}
                }
            }
            Ok(JsonValue::Array(parts))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_message::factory::user_msg;

    #[test]
    fn test_format_simple_text() {
        let fmt = DashScopeFormatter::default();
        let msg = user_msg("user", "Hello!").unwrap();
        let result = fmt.format(&[msg]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "Hello!");
    }

    #[test]
    fn test_format_multiple_messages() {
        let fmt = DashScopeFormatter::default();
        let msg1 = user_msg("user", "Hi").unwrap();
        let msg2 = user_msg("user", "There").unwrap();
        let result = fmt.format(&[msg1, msg2]).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_convert_tool_result_text_only() {
        let fmt = DashScopeFormatter::default();
        let output = ToolOutput::Text("result text".to_string());
        let (text, promoted) = fmt.convert_tool_result_to_string(&output).unwrap();
        assert_eq!(text, "result text");
        assert!(promoted.is_empty());
    }

    #[test]
    fn test_assistant_tool_calls_content_is_null() {
        // When an assistant message has tool calls but NO text blocks,
        // content must be JSON null (OpenAI-compatible API requirement).
        let fmt = DashScopeFormatter::default();
        let tc = agent_scope_message::ToolCallBlock::new(
            "call_1".into(),
            "calculator".into(),
            r#"{"expression": "1+1"}"#.into(),
        );
        let msg = Msg::new(
            "assistant".into(),
            vec![ContentBlock::ToolCall(tc)],
            Role::Assistant,
        )
        .unwrap();
        let result = fmt.format(&[msg]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        // content must be null for pure tool_call messages
        assert!(result[0]["content"].is_null());
        // tool_calls array must be present
        let tcs = &result[0]["tool_calls"];
        assert!(tcs.is_array());
        assert_eq!(tcs[0]["function"]["name"], "calculator");
    }

    #[test]
    fn test_assistant_text_plus_tool_call_has_content() {
        // When an assistant message has BOTH text and tool calls,
        // content should be present (the text content).
        let fmt = DashScopeFormatter::default();
        let tb = agent_scope_message::TextBlock::new("Let me calculate".into());
        let tc = agent_scope_message::ToolCallBlock::new(
            "call_1".into(),
            "calculator".into(),
            r#"{"expression": "1+1"}"#.into(),
        );
        let msg = Msg::new(
            "assistant".into(),
            vec![ContentBlock::Text(tb), ContentBlock::ToolCall(tc)],
            Role::Assistant,
        )
        .unwrap();
        let result = fmt.format(&[msg]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        // content should be an array with the text part
        let content_arr = result[0]["content"]
            .as_array()
            .expect("content should be array");
        assert_eq!(content_arr[0]["type"], "text");
        assert_eq!(content_arr[0]["text"], "Let me calculate");
        // tool_calls array must still be present
        assert!(result[0]["tool_calls"].is_array());
    }

    #[test]
    fn test_tool_role_message_no_name_field() {
        // Tool result messages (role=tool) must NOT include a name field.
        // OpenAI-compatible APIs reject name on tool-role messages.
        let fmt = DashScopeFormatter::default();
        let trb = agent_scope_message::ToolResultBlock::new(
            "call_1".into(),
            "calculator".into(),
            ToolOutput::Text("42".into()),
        );
        let msg = Msg::new(
            "assistant".into(),
            vec![ContentBlock::ToolResult(trb)],
            Role::Assistant,
        )
        .unwrap();
        let result = fmt.format(&[msg]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "tool");
        assert_eq!(result[0]["tool_call_id"], "call_1");
        assert_eq!(result[0]["content"], "42");
        // name must NOT be present on tool-role messages
        assert!(result[0].get("name").is_none());
    }
}
