//! Msg — the core message type.

use serde::{Deserialize, Serialize};

use crate::block::{BlockType, ContentBlock};
use agent_scope_types::{ErrorInfo, ReplyFinishedReason};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_id() -> String {
    agent_scope_utils::id::generate_id()
}

fn default_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Default `metadata` value: an empty object, matching what `Msg::new`
/// constructs, so omitted `metadata` does not round-trip to `null`.
fn default_metadata() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

// ---------------------------------------------------------------------------
// Role
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

// ---------------------------------------------------------------------------
// Usage
// ---------------------------------------------------------------------------

/// Token usage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: i64,
    pub output_tokens: i64,
}

// ---------------------------------------------------------------------------
// Msg
// ---------------------------------------------------------------------------

/// The core message data structure in AgentScope.
///
/// Messages are the fundamental unit of communication between agents.
/// They contain a role (user/assistant/system), a list of ContentBlocks,
/// and metadata such as timestamps, token usage, and error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Msg {
    /// Name of the message sender / agent.
    pub name: String,
    /// Content blocks composing the message body.
    pub content: Vec<ContentBlock>,
    /// Role discriminator: user, assistant, or system.
    pub role: Role,
    /// Unique identifier, auto-generated if not provided.
    #[serde(default = "default_id")]
    pub id: String,
    /// Arbitrary metadata dictionary.
    /// Defaults to `{}` (not `null`) so round-tripping a message that omitted
    /// `metadata` stays consistent with `Msg::new`, which always constructs an
    /// empty object (round-4 M4).
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
    /// ISO 8601 creation timestamp.
    #[serde(default = "default_timestamp")]
    pub created_at: String,
    /// Token usage statistics (set after model call completes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// Timestamp when the message was finalized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    /// Why the reply finished (completed, interrupted, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_reason: Option<ReplyFinishedReason>,
    /// Structured output (e.g., from function calling / JSON mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<serde_json::Value>,
    /// Error information if the message represents a failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

// ---------------------------------------------------------------------------
// ValidationError
// ---------------------------------------------------------------------------

/// Error returned when message construction fails validation.
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Content blocks violate role constraints.
    InvalidContentForRole {
        role: Role,
        disallowed_types: Vec<BlockType>,
    },
    /// Message content is empty (optional check).
    EmptyContent,
}

// ---------------------------------------------------------------------------
// AppendEventError
// ---------------------------------------------------------------------------

/// Error returned when an event cannot be applied to a message.
#[derive(Debug, Clone)]
pub enum AppendEventError {
    /// The event's reply_id does not match the message's id.
    ReplyIdMismatch {
        event_reply_id: String,
        msg_id: String,
    },
    /// Referenced content block was not found.
    BlockNotFound {
        block_type: BlockType,
        block_id: String,
    },
    /// Event type is not recognized.
    UnknownEventType(String),
}

// ---------------------------------------------------------------------------
// Msg impl
// ---------------------------------------------------------------------------

impl Msg {
    /// Create a new Msg with role-content validation.
    ///
    /// Validation rules (FR-014):
    /// - User role: only Text and Data blocks allowed
    /// - System role: only Text blocks allowed
    /// - Assistant role: all block types allowed
    pub fn new(
        name: String,
        content: Vec<ContentBlock>,
        role: Role,
    ) -> Result<Self, ValidationError> {
        Self::validate_role_content(&role, &content)?;
        Ok(Self {
            name,
            content,
            role,
            id: default_id(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            created_at: default_timestamp(),
            usage: None,
            finished_at: None,
            finished_reason: None,
            structured_output: None,
            error: None,
        })
    }

    /// Validate that content blocks are compatible with the given role.
    fn validate_role_content(role: &Role, content: &[ContentBlock]) -> Result<(), ValidationError> {
        match role {
            Role::User => {
                let disallowed: Vec<BlockType> = content
                    .iter()
                    .filter(|b| !matches!(b.block_type(), BlockType::Text | BlockType::Data))
                    .map(|b| b.block_type())
                    .collect();
                if !disallowed.is_empty() {
                    return Err(ValidationError::InvalidContentForRole {
                        role: Role::User,
                        disallowed_types: disallowed,
                    });
                }
            }
            Role::System => {
                let disallowed: Vec<BlockType> = content
                    .iter()
                    .filter(|b| !matches!(b.block_type(), BlockType::Text))
                    .map(|b| b.block_type())
                    .collect();
                if !disallowed.is_empty() {
                    return Err(ValidationError::InvalidContentForRole {
                        role: Role::System,
                        disallowed_types: disallowed,
                    });
                }
            }
            Role::Assistant => {
                // No restrictions — assistant messages can contain any block type
            }
        }
        Ok(())
    }

    /// Get content blocks filtered by type.
    ///
    /// - `Some(block_type)`: returns only blocks of that type
    /// - `None`: returns all blocks
    pub fn get_content_blocks(&self, block_type: Option<BlockType>) -> Vec<&ContentBlock> {
        match block_type {
            Some(bt) => self
                .content
                .iter()
                .filter(|b| b.block_type() == bt)
                .collect(),
            None => self.content.iter().collect(),
        }
    }

    /// Concatenate all TextBlock text fields with the given separator.
    ///
    /// Returns `None` if there are no TextBlocks in the message.
    pub fn get_text_content(&self, separator: &str) -> Option<String> {
        let texts: Vec<&str> = self
            .content
            .iter()
            .filter_map(|b| {
                if let ContentBlock::Text(tb) = b {
                    Some(tb.text.as_str())
                } else {
                    None
                }
            })
            .collect();
        if texts.is_empty() {
            None
        } else {
            Some(texts.join(separator))
        }
    }

    /// Check whether the message contains content blocks of a given type.
    ///
    /// - `Some(block_type)`: returns true if at least one block matches
    /// - `None`: returns true if the message has any content
    pub fn has_content_blocks(&self, block_type: Option<BlockType>) -> bool {
        match block_type {
            Some(bt) => self.content.iter().any(|b| b.block_type() == bt),
            None => !self.content.is_empty(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{TextBlock, ToolCallBlock};

    fn make_text_block(text: &str) -> ContentBlock {
        ContentBlock::Text(TextBlock::new(text.to_string()))
    }

    fn make_tool_call_block(id: &str, name: &str) -> ContentBlock {
        ContentBlock::ToolCall(ToolCallBlock::new(id.into(), name.into(), "{}".into()))
    }

    // -- Msg::new validation --
    #[test]
    fn test_user_msg_rejects_tool_call() {
        let result = Msg::new(
            "user".into(),
            vec![make_tool_call_block("tc-1", "search")],
            Role::User,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_user_msg_accepts_text() {
        let result = Msg::new("user".into(), vec![make_text_block("hello")], Role::User);
        assert!(result.is_ok());
    }

    #[test]
    fn test_system_msg_rejects_data_block() {
        let data_block = ContentBlock::Data(crate::block::DataBlock::new(
            crate::block::DataSource::Url(crate::source::URLSource {
                url: "http://x".into(),
                media_type: "image/png".into(),
            }),
        ));
        let result = Msg::new("system".into(), vec![data_block], Role::System);
        assert!(result.is_err());
    }

    #[test]
    fn test_system_msg_accepts_text() {
        let result = Msg::new(
            "system".into(),
            vec![make_text_block("system prompt")],
            Role::System,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_assistant_msg_accepts_all_types() {
        let result = Msg::new(
            "assistant".into(),
            vec![
                make_text_block("hello"),
                make_tool_call_block("tc-1", "search"),
            ],
            Role::Assistant,
        );
        assert!(result.is_ok());
    }

    // -- Msg::get_content_blocks --
    #[test]
    fn test_get_content_blocks_filtering() {
        let msg = Msg::new(
            "alice".into(),
            vec![
                make_text_block("first"),
                make_text_block("second"),
                make_tool_call_block("tc-1", "search"),
            ],
            Role::Assistant,
        )
        .unwrap();

        let text_blocks = msg.get_content_blocks(Some(BlockType::Text));
        assert_eq!(text_blocks.len(), 2);

        let tool_calls = msg.get_content_blocks(Some(BlockType::ToolCall));
        assert_eq!(tool_calls.len(), 1);

        let all = msg.get_content_blocks(None);
        assert_eq!(all.len(), 3);

        let hints = msg.get_content_blocks(Some(BlockType::Hint));
        assert!(hints.is_empty());
    }

    // -- Msg::get_text_content --
    #[test]
    fn test_get_text_content() {
        let msg = Msg::new(
            "alice".into(),
            vec![make_text_block("Hello"), make_text_block("World")],
            Role::Assistant,
        )
        .unwrap();

        assert_eq!(msg.get_text_content(" ").unwrap(), "Hello World");
        assert_eq!(msg.get_text_content("").unwrap(), "HelloWorld");
        assert_eq!(msg.get_text_content(", ").unwrap(), "Hello, World");
    }

    #[test]
    fn test_get_text_content_no_text_blocks() {
        let msg = Msg::new(
            "alice".into(),
            vec![make_tool_call_block("tc-1", "search")],
            Role::Assistant,
        )
        .unwrap();

        assert!(msg.get_text_content(" ").is_none());
    }

    // -- Msg::has_content_blocks --
    #[test]
    fn test_has_content_blocks() {
        let msg = Msg::new("alice".into(), vec![make_text_block("hi")], Role::Assistant).unwrap();

        assert!(msg.has_content_blocks(Some(BlockType::Text)));
        assert!(!msg.has_content_blocks(Some(BlockType::ToolCall)));
        assert!(msg.has_content_blocks(None));
    }

    #[test]
    fn test_has_content_blocks_empty_message() {
        let msg = Msg::new("alice".into(), vec![], Role::Assistant).unwrap();
        assert!(!msg.has_content_blocks(None));
    }

    // -- Msg serialization --
    #[test]
    fn test_msg_json_roundtrip() {
        let msg = Msg::new(
            "alice".into(),
            vec![make_text_block("Hello, world!")],
            Role::Assistant,
        )
        .unwrap();

        let json = serde_json::to_string_pretty(&msg).unwrap();
        let restored: Msg = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.name, msg.name);
        assert_eq!(restored.role, msg.role);
        assert_eq!(restored.content.len(), msg.content.len());
        assert_eq!(restored.get_text_content(" "), msg.get_text_content(" "));
    }

    #[test]
    fn test_msg_json_role_is_lowercase() {
        let msg = Msg::new("user1".into(), vec![make_text_block("hello")], Role::User).unwrap();
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""role":"user""#));

        let msg = Msg::new("sys".into(), vec![make_text_block("prompt")], Role::System).unwrap();
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""role":"system""#));

        let msg = Msg::new(
            "asst".into(),
            vec![make_text_block("reply")],
            Role::Assistant,
        )
        .unwrap();
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""role":"assistant""#));
    }

    // -- Usage --
    #[test]
    fn test_usage_serialization() {
        let usage = Usage {
            input_tokens: 150,
            output_tokens: 50,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains(r#""input_tokens":150"#));
        assert!(json.contains(r#""output_tokens":50"#));
    }
}
