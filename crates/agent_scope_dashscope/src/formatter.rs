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
            let role_str = match msg.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
            };

            let mut entry = serde_json::Map::new();
            entry.insert("role".to_string(), JsonValue::String(role_str.to_string()));

            // Determine content format
            let content = self.format_content(&msg.content)?;
            entry.insert("content".to_string(), content);

            // Handle tool calls (assistant messages)
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
                }
            }

            // Handle tool role (tool result messages)
            if msg.role == Role::Assistant {
                let has_tool_result = msg
                    .content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolResult(_)));
                if has_tool_result {
                    entry.insert("role".to_string(), JsonValue::String("tool".to_string()));
                }
            }

            // Handle name field
            if !msg.name.is_empty() {
                entry.insert("name".to_string(), JsonValue::String(msg.name.clone()));
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
}
