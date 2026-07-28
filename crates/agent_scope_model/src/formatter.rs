//! Formatter trait — Msg → API format conversion.

use std::fmt;

use agent_scope_message::{ContentBlock, Msg, ToolOutput};
use serde_json::Value as JsonValue;

/// Errors that can occur during message formatting.
#[derive(Debug)]
pub enum FormatError {
    InvalidMessage(String),
    UnsupportedMediaType {
        media_type: String,
        block_id: String,
    },
    Io(std::io::Error),
    Base64Decode(base64::DecodeError),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMessage(msg) => write!(f, "Invalid message: {msg}"),
            Self::UnsupportedMediaType {
                media_type,
                block_id,
            } => {
                write!(
                    f,
                    "Unsupported media type '{media_type}' in block '{block_id}'"
                )
            }
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Base64Decode(e) => write!(f, "Base64 decode error: {e}"),
        }
    }
}

impl std::error::Error for FormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Base64Decode(e) => Some(e),
            _ => None,
        }
    }
}

/// Message grouping classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageGroup {
    ToolSequence,
    AgentMessage,
}

/// Trait for converting `Msg` objects into Provider-specific API format.
pub trait Formatter: Send + Sync {
    /// Return supported media type patterns (excluding text/plain and application/x-thinking).
    fn supported_input_media_types(&self) -> &[String];

    /// Convert a list of Msg objects to Provider API format dicts.
    fn format(&self, msgs: &[Msg]) -> Result<Vec<JsonValue>, FormatError>;

    /// Separate multimodal data from tool results.
    /// Returns (text representation, promoted ContentBlocks).
    fn convert_tool_result_to_string(
        &self,
        output: &ToolOutput,
    ) -> Result<(String, Vec<ContentBlock>), FormatError>;

    /// Group messages into tool sequences and agent messages, preserving order.
    fn group_messages<'a>(&self, msgs: &'a [Msg]) -> Vec<(MessageGroup, Vec<&'a Msg>)> {
        let mut groups: Vec<(MessageGroup, Vec<&Msg>)> = Vec::new();
        let mut current_group: Vec<&Msg> = Vec::new();
        let mut in_tool_sequence = false;

        for msg in msgs {
            let is_tool_msg = msg.has_content_blocks(None)
                && msg
                    .content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolCall(_) | ContentBlock::ToolResult(_)));

            if is_tool_msg {
                if !in_tool_sequence && !current_group.is_empty() {
                    groups.push((
                        MessageGroup::AgentMessage,
                        std::mem::take(&mut current_group),
                    ));
                }
                in_tool_sequence = true;
                current_group.push(msg);
            } else {
                if in_tool_sequence && !current_group.is_empty() {
                    groups.push((
                        MessageGroup::ToolSequence,
                        std::mem::take(&mut current_group),
                    ));
                }
                in_tool_sequence = false;
                current_group.push(msg);
            }
        }

        if !current_group.is_empty() {
            let group_type = if in_tool_sequence {
                MessageGroup::ToolSequence
            } else {
                MessageGroup::AgentMessage
            };
            groups.push((group_type, current_group));
        }

        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_message::factory::user_msg;

    #[test]
    fn test_group_messages_all_text() {
        struct TestFormatter;
        impl Formatter for TestFormatter {
            fn supported_input_media_types(&self) -> &[String] {
                &[]
            }
            fn format(&self, _msgs: &[Msg]) -> Result<Vec<JsonValue>, FormatError> {
                Ok(vec![])
            }
            fn convert_tool_result_to_string(
                &self,
                _output: &ToolOutput,
            ) -> Result<(String, Vec<ContentBlock>), FormatError> {
                Ok((String::new(), vec![]))
            }
        }

        let msg1 = user_msg("user", "Hello").unwrap();
        let msg2 = user_msg("user", "World").unwrap();
        let fmt = TestFormatter;
        let msgs = [msg1, msg2];
        let groups = fmt.group_messages(&msgs);
        assert_eq!(groups.len(), 1);
        assert!(matches!(groups[0].0, MessageGroup::AgentMessage));
    }
}
