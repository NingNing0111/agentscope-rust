//! Integration tests for Msg creation, ContentBlock operations, and factory functions.
//! T102

use agent_scope_message::block::{
    BlockType, ContentBlock, DataBlock, DataSource, HintBlock, HintContent, TextBlock,
    ThinkingBlock, ToolCallBlock, ToolResultBlock,
};
use agent_scope_message::factory;
use agent_scope_message::msg::{Msg, Role};
use agent_scope_message::source::URLSource;
use agent_scope_types::ReplyFinishedReason;

// ── Helper ───────────────────────────────────────────────────────────

fn text_block(text: &str) -> ContentBlock {
    ContentBlock::Text(TextBlock::new(text.to_string()))
}

fn tool_call_block(id: &str, name: &str) -> ContentBlock {
    ContentBlock::ToolCall(ToolCallBlock::new(id.into(), name.into(), "{}".into()))
}

// ── Msg creation & role validation ──────────────────────────────────

#[test]
fn test_msg_user_role_rejects_tool_call() {
    let result = Msg::new(
        "user".into(),
        vec![tool_call_block("tc", "search")],
        Role::User,
    );
    assert!(result.is_err());
}

#[test]
fn test_msg_user_role_rejects_thinking() {
    let result = Msg::new(
        "user".into(),
        vec![ContentBlock::Thinking(ThinkingBlock::new("think".into()))],
        Role::User,
    );
    assert!(result.is_err());
}

#[test]
fn test_msg_user_role_rejects_hint() {
    let result = Msg::new(
        "user".into(),
        vec![ContentBlock::Hint(HintBlock::new(HintContent::Text(
            "hint".into(),
        )))],
        Role::User,
    );
    assert!(result.is_err());
}

#[test]
fn test_msg_user_role_accepts_text_and_data() {
    let data_block = ContentBlock::Data(DataBlock::new(DataSource::Url(URLSource {
        url: "https://example.com/img.png".into(),
        media_type: "image/png".into(),
    })));
    let result = Msg::new(
        "user".into(),
        vec![text_block("hello"), data_block],
        Role::User,
    );
    assert!(result.is_ok());
}

#[test]
fn test_msg_system_role_only_accepts_text() {
    let result = Msg::new("system".into(), vec![text_block("prompt")], Role::System);
    assert!(result.is_ok());

    let data_block = ContentBlock::Data(DataBlock::new(DataSource::Url(URLSource {
        url: "http://x".into(),
        media_type: "text/plain".into(),
    })));
    let result = Msg::new("system".into(), vec![data_block], Role::System);
    assert!(result.is_err());
}

#[test]
fn test_msg_assistant_accepts_all_block_types() {
    let blocks = vec![
        text_block("hello"),
        ContentBlock::Thinking(ThinkingBlock::new("think".into())),
        ContentBlock::Hint(HintBlock::new(HintContent::Text("hint".into()))),
        ContentBlock::Data(DataBlock::new(DataSource::Url(URLSource {
            url: "http://x".into(),
            media_type: "text/plain".into(),
        }))),
        tool_call_block("tc", "search"),
        ContentBlock::ToolResult(ToolResultBlock::new(
            "tr".into(),
            "search".into(),
            agent_scope_message::block::ToolOutput::Text("done".into()),
        )),
    ];
    let result = Msg::new("assistant".into(), blocks, Role::Assistant);
    assert!(result.is_ok());
}

// ── ContentBlock filtering and text extraction ───────────────────────

#[test]
fn test_get_content_blocks_filter_single_type() {
    let msg = Msg::new(
        "alice".into(),
        vec![text_block("a"), text_block("b"), tool_call_block("tc", "s")],
        Role::Assistant,
    )
    .unwrap();

    assert_eq!(msg.get_content_blocks(Some(BlockType::Text)).len(), 2);
    assert_eq!(msg.get_content_blocks(Some(BlockType::ToolCall)).len(), 1);
    assert_eq!(msg.get_content_blocks(Some(BlockType::Data)).len(), 0);
    assert_eq!(msg.get_content_blocks(None).len(), 3);
}

#[test]
fn test_get_text_content_with_various_separators() {
    let msg = Msg::new(
        "alice".into(),
        vec![text_block("A"), text_block("B"), text_block("C")],
        Role::Assistant,
    )
    .unwrap();

    assert_eq!(msg.get_text_content(" ").unwrap(), "A B C");
    assert_eq!(msg.get_text_content("").unwrap(), "ABC");
    assert_eq!(msg.get_text_content("\n").unwrap(), "A\nB\nC");
}

#[test]
fn test_get_text_content_skips_non_text_blocks() {
    let msg = Msg::new(
        "alice".into(),
        vec![
            text_block("Hello"),
            tool_call_block("tc", "search"),
            text_block("World"),
        ],
        Role::Assistant,
    )
    .unwrap();
    assert_eq!(msg.get_text_content(" ").unwrap(), "Hello World");
}

#[test]
fn test_has_content_blocks_empty_and_non_empty() {
    let empty = Msg::new("a".into(), vec![], Role::Assistant).unwrap();
    assert!(!empty.has_content_blocks(None));

    let full = Msg::new("a".into(), vec![text_block("x")], Role::Assistant).unwrap();
    assert!(full.has_content_blocks(None));
    assert!(full.has_content_blocks(Some(BlockType::Text)));
    assert!(!full.has_content_blocks(Some(BlockType::ToolCall)));
}

// ── Factory functions ────────────────────────────────────────────────

#[test]
fn test_factory_user_msg_properties() {
    let msg = factory::user_msg("user1", "Hello, world!").unwrap();
    assert_eq!(msg.role, Role::User);
    assert_eq!(msg.name, "user1");
    assert_eq!(msg.get_text_content(" ").unwrap(), "Hello, world!");
    assert!(msg.finished_at.is_some());
    assert!(!msg.id.is_empty());
}

#[test]
fn test_factory_user_msg_with_blocks() {
    let blocks = vec![text_block("text"), {
        ContentBlock::Data(DataBlock::new(DataSource::Url(URLSource {
            url: "http://x".into(),
            media_type: "image/png".into(),
        })))
    }];
    let msg = factory::user_msg_with_blocks("user1", blocks).unwrap();
    assert_eq!(msg.content.len(), 2);
    assert!(msg.finished_at.is_some());
}

#[test]
fn test_factory_assistant_msg_properties() {
    let msg = factory::assistant_msg("asst", "The weather is sunny.");
    assert_eq!(msg.role, Role::Assistant);
    assert_eq!(msg.name, "asst");
    assert!(msg.finished_at.is_none()); // assistant messages are built incrementally
}

#[test]
fn test_factory_assistant_msg_with_blocks() {
    let blocks = vec![text_block("reply"), tool_call_block("tc", "search")];
    let msg = factory::assistant_msg_with_blocks("asst", blocks);
    assert_eq!(msg.content.len(), 2);
    assert_eq!(msg.role, Role::Assistant);
}

#[test]
fn test_factory_system_msg_properties() {
    let msg = factory::system_msg("sys", "You are helpful.").unwrap();
    assert_eq!(msg.role, Role::System);
    assert_eq!(msg.name, "sys");
    assert!(msg.finished_at.is_some());
}

#[test]
fn test_factory_system_msg_rejects_data() {
    let blocks = vec![ContentBlock::Data(DataBlock::new(DataSource::Url(
        URLSource {
            url: "http://x".into(),
            media_type: "image/png".into(),
        },
    )))];
    let result = factory::system_msg_with_blocks("sys", blocks);
    assert!(result.is_err());
}

// ── Msg JSON serialization ──────────────────────────────────────────

#[test]
fn test_msg_serialization_role_is_lowercase() {
    let msg = factory::user_msg("u", "hi").unwrap();
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""role":"user""#));

    let msg = factory::assistant_msg("a", "hi");
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""role":"assistant""#));

    let msg = factory::system_msg("s", "prompt").unwrap();
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""role":"system""#));
}

#[test]
fn test_msg_serialization_includes_name_and_id() {
    let msg = factory::user_msg("alice", "hello").unwrap();
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""name":"alice""#));
    assert!(!msg.id.is_empty());
}

#[test]
fn test_msg_complex_roundtrip() {
    let msg = {
        let mut m = Msg::new(
            "agent".into(),
            vec![text_block("I'll search"), tool_call_block("tc-1", "search")],
            Role::Assistant,
        )
        .unwrap();
        m.usage = Some(agent_scope_message::msg::Usage {
            input_tokens: 100,
            output_tokens: 50,
        });
        m.finished_reason = Some(ReplyFinishedReason::Completed);
        m
    };

    let json = serde_json::to_string(&msg).unwrap();
    let restored: Msg = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.name, "agent");
    assert_eq!(restored.role, Role::Assistant);
    assert_eq!(restored.content.len(), 2);
    assert_eq!(restored.usage.unwrap().input_tokens, 100);
    assert_eq!(
        restored.finished_reason,
        Some(ReplyFinishedReason::Completed)
    );
}
