//! Factory functions for creating Msg instances.

use crate::block::{ContentBlock, TextBlock};
use crate::msg::{Msg, Role, ValidationError};

/// Create a user message.
///
/// User messages only allow Text and Data blocks.
/// By default, `finished_at` is set to `created_at` (user messages
/// are considered "complete" upon creation).
pub fn user_msg(name: &str, text: &str) -> Result<Msg, ValidationError> {
    let block = TextBlock::new(text.to_string());
    let mut msg = Msg::new(
        name.to_string(),
        vec![ContentBlock::Text(block)],
        Role::User,
    )?;
    msg.finished_at = Some(msg.created_at.clone());
    Ok(msg)
}

/// Create a user message with custom content blocks.
pub fn user_msg_with_blocks(
    name: &str,
    content: Vec<ContentBlock>,
) -> Result<Msg, ValidationError> {
    let mut msg = Msg::new(name.to_string(), content, Role::User)?;
    msg.finished_at = Some(msg.created_at.clone());
    Ok(msg)
}

/// Create an assistant message.
///
/// Assistant messages accept all ContentBlock types.
/// `finished_at` defaults to `None` (assistant messages are built incrementally).
pub fn assistant_msg(name: &str, text: &str) -> Msg {
    let block = TextBlock::new(text.to_string());
    Msg::new(
        name.to_string(),
        vec![ContentBlock::Text(block)],
        Role::Assistant,
    )
    .expect("assistant messages accept all content types")
}

/// Create an assistant message with custom content blocks.
pub fn assistant_msg_with_blocks(name: &str, content: Vec<ContentBlock>) -> Msg {
    Msg::new(name.to_string(), content, Role::Assistant)
        .expect("assistant messages accept all content types")
}

/// Create a system message.
///
/// System messages only allow Text blocks.
/// By default, `finished_at` is set to `created_at`.
pub fn system_msg(name: &str, text: &str) -> Result<Msg, ValidationError> {
    let block = TextBlock::new(text.to_string());
    let mut msg = Msg::new(
        name.to_string(),
        vec![ContentBlock::Text(block)],
        Role::System,
    )?;
    msg.finished_at = Some(msg.created_at.clone());
    Ok(msg)
}

/// Create a system message with custom content blocks.
pub fn system_msg_with_blocks(
    name: &str,
    content: Vec<ContentBlock>,
) -> Result<Msg, ValidationError> {
    let mut msg = Msg::new(name.to_string(), content, Role::System)?;
    msg.finished_at = Some(msg.created_at.clone());
    Ok(msg)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{DataBlock, ToolCallBlock};

    #[test]
    fn test_user_msg_creation() {
        let msg = user_msg("user1", "Hello, what is the weather?").unwrap();
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.name, "user1");
        assert!(msg.has_content_blocks(Some(crate::block::BlockType::Text)));
        assert_eq!(
            msg.get_text_content(" ").unwrap(),
            "Hello, what is the weather?"
        );
        assert!(msg.finished_at.is_some());
    }

    #[test]
    fn test_user_msg_rejects_tool_call_blocks() {
        let result = user_msg_with_blocks(
            "user1",
            vec![ContentBlock::ToolCall(ToolCallBlock::new(
                "tc-1".into(),
                "search".into(),
                "{}".into(),
            ))],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_assistant_msg_creation() {
        let msg = assistant_msg("assistant1", "The weather is sunny.");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.name, "assistant1");
        assert!(msg.has_content_blocks(Some(crate::block::BlockType::Text)));
        assert!(msg.finished_at.is_none());
    }

    #[test]
    fn test_assistant_msg_accepts_tool_call_blocks() {
        let msg = assistant_msg_with_blocks(
            "assistant1",
            vec![
                ContentBlock::Text(TextBlock::new("I will search".into())),
                ContentBlock::ToolCall(ToolCallBlock::new(
                    "tc-1".into(),
                    "search".into(),
                    r#"{"q":"test"}"#.into(),
                )),
            ],
        );
        assert_eq!(msg.content.len(), 2);
    }

    #[test]
    fn test_system_msg_creation() {
        let msg = system_msg("system", "You are a helpful assistant.").unwrap();
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.name, "system");
        assert!(msg.finished_at.is_some());
    }

    #[test]
    fn test_system_msg_rejects_data_blocks() {
        let data_block = ContentBlock::Data(DataBlock::new(crate::block::DataSource::Url(
            crate::source::URLSource {
                url: "https://example.com/img.png".into(),
                media_type: "image/png".into(),
            },
        )));
        let result = system_msg_with_blocks("system", vec![data_block]);
        assert!(result.is_err());
    }
}
