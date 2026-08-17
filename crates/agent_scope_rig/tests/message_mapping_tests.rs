//! T010 — 消息映射测试（出站 `msg_to_rig_messages` + 入站 `assistant_content_to_blocks`）。
//!
//! 对照 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §1。

use agent_scope_message::Base64Source;
use agent_scope_message::block::{
    ContentBlock, DataBlock, DataSource, HintBlock, HintContent, TextBlock, ThinkingBlock,
    ToolCallBlock, ToolOutput, ToolResultBlock,
};
use agent_scope_message::msg::{Msg, Role};
use agent_scope_model::ModelError;
use agent_scope_rig::message::{assistant_content_to_blocks, msg_to_rig_messages};
use rig::completion::Message;
use rig::completion::message::{
    AssistantContent, DocumentSourceKind, Image, ImageMediaType, UserContent,
};

/// 构造 Msg（直接字面量，绕过 `Msg::new` 的角色校验——User 校验拒绝 ToolResult 块，
/// 但真实工具结果消息含 ToolResult）。
fn msg(name: &str, role: Role, content: Vec<ContentBlock>) -> Msg {
    Msg {
        name: name.to_string(),
        content,
        role,
        id: "mid".to_string(),
        metadata: serde_json::Value::Object(serde_json::Map::new()),
        created_at: "t".to_string(),
        usage: None,
        finished_at: None,
        finished_reason: None,
        structured_output: None,
        error: None,
    }
}

fn text_block(s: &str) -> ContentBlock {
    ContentBlock::Text(TextBlock::new(s.to_string()))
}

fn tool_result_block(id: &str, name: &str, output: &str) -> ContentBlock {
    ContentBlock::ToolResult(ToolResultBlock::new(
        id.to_string(),
        name.to_string(),
        ToolOutput::Text(output.to_string()),
    ))
}

fn data_block_base64(data: &str, media_type: &str) -> ContentBlock {
    ContentBlock::Data(DataBlock::new(DataSource::Base64(Base64Source {
        data: data.to_string(),
        media_type: media_type.to_string(),
    })))
}

// ---------------------------------------------------------------------------
// 出站
// ---------------------------------------------------------------------------

#[test]
fn system_maps_to_rig_system() {
    let msgs = vec![msg(
        "assistant",
        Role::System,
        vec![text_block("be concise")],
    )];
    let rig_msgs = msg_to_rig_messages(&msgs).unwrap();
    assert_eq!(rig_msgs.len(), 1);
    match &rig_msgs[0] {
        Message::System { content } => assert_eq!(content, "be concise"),
        other => panic!("expected system message, got {other:?}"),
    }
}

#[test]
fn user_text_maps_to_rig_user_text() {
    let msgs = vec![msg("user", Role::User, vec![text_block("hello")])];
    let rig_msgs = msg_to_rig_messages(&msgs).unwrap();
    assert_eq!(rig_msgs.len(), 1);
    match &rig_msgs[0] {
        Message::User { content } => match content.first() {
            UserContent::Text(t) => assert_eq!(t.text, "hello"),
            other => panic!("expected user text, got {other:?}"),
        },
        other => panic!("expected user message, got {other:?}"),
    }
}

#[test]
fn user_data_maps_to_rig_user_image() {
    let msgs = vec![msg(
        "user",
        Role::User,
        vec![data_block_base64("aGVsbG8=", "image/png")],
    )];
    let rig_msgs = msg_to_rig_messages(&msgs).unwrap();
    match &rig_msgs[0] {
        Message::User { content } => match content.first() {
            UserContent::Image(img) => match &img.data {
                DocumentSourceKind::Base64(data) => assert_eq!(data, "aGVsbG8="),
                other => panic!("expected base64 image data, got {other:?}"),
            },
            other => panic!("expected user image, got {other:?}"),
        },
        other => panic!("expected user message, got {other:?}"),
    }
}

#[test]
fn assistant_blocks_map_to_rig_contents() {
    let msgs = vec![msg(
        "assistant",
        Role::Assistant,
        vec![
            text_block("let me think"),
            ContentBlock::Thinking(ThinkingBlock::new("reasoning here".to_string())),
            ContentBlock::ToolCall(ToolCallBlock::new(
                "tc1".to_string(),
                "search".to_string(),
                r#"{"q":"rust"}"#.to_string(),
            )),
        ],
    )];
    let rig_msgs = msg_to_rig_messages(&msgs).unwrap();
    assert_eq!(rig_msgs.len(), 1);
    match &rig_msgs[0] {
        Message::Assistant { content, .. } => {
            let items: Vec<_> = content.iter().collect();
            assert_eq!(items.len(), 3);
            assert!(matches!(items[0], AssistantContent::Text(_)));
            assert!(matches!(items[1], AssistantContent::Reasoning(_)));
            match items[2] {
                AssistantContent::ToolCall(tc) => {
                    assert_eq!(tc.id, "tc1");
                    assert_eq!(tc.function.name, "search");
                    assert_eq!(tc.function.arguments, serde_json::json!({"q": "rust"}));
                }
                other => panic!("expected tool call, got {other:?}"),
            }
        }
        other => panic!("expected assistant message, got {other:?}"),
    }
}

#[test]
fn tool_result_expands_to_separate_user_message_after_primary() {
    let msgs = vec![msg(
        "user",
        Role::User,
        vec![
            text_block("search please"),
            tool_result_block("tr1", "search", "5 results"),
        ],
    )];
    let rig_msgs = msg_to_rig_messages(&msgs).unwrap();
    // 主消息在前，ToolResult 展开为独立 User 消息紧随其后。
    assert_eq!(rig_msgs.len(), 2);
    match &rig_msgs[0] {
        Message::User { content } => assert!(matches!(content.first(), UserContent::Text(_))),
        other => panic!("expected user message, got {other:?}"),
    }
    match &rig_msgs[1] {
        Message::User { content } => match content.first() {
            UserContent::ToolResult(tr) => {
                assert_eq!(tr.id, "tr1");
                assert!(matches!(
                    tr.content.first(),
                    rig::completion::message::ToolResultContent::Text(_)
                ));
            }
            other => panic!("expected tool result, got {other:?}"),
        },
        other => panic!("expected user message, got {other:?}"),
    }
}

#[test]
fn hint_blocks_are_sent_as_text() {
    let msgs = vec![msg(
        "assistant",
        Role::Assistant,
        vec![
            ContentBlock::Hint(HintBlock::new(HintContent::Text("internal".to_string()))),
            text_block("visible"),
        ],
    )];
    let rig_msgs = msg_to_rig_messages(&msgs).unwrap();
    assert_eq!(rig_msgs.len(), 1);
    match &rig_msgs[0] {
        Message::Assistant { content, .. } => {
            assert_eq!(content.iter().count(), 2, "hint must be sent as text");
            let texts: Vec<&str> = content
                .iter()
                .filter_map(|c| match c {
                    AssistantContent::Text(t) => Some(t.text.as_str()),
                    _ => None,
                })
                .collect();
            assert!(texts.contains(&"internal"));
            assert!(texts.contains(&"visible"));
        }
        other => panic!("expected assistant message, got {other:?}"),
    }
}

#[test]
fn unknown_block_errors() {
    let msgs = vec![msg(
        "assistant",
        Role::Assistant,
        vec![ContentBlock::Unknown],
    )];
    let err = msg_to_rig_messages(&msgs).unwrap_err();
    assert!(
        matches!(err, ModelError::FormatError { .. }),
        "expected FormatError, got {err:?}"
    );
}

#[test]
fn invalid_tool_call_input_errors() {
    let msgs = vec![msg(
        "assistant",
        Role::Assistant,
        vec![ContentBlock::ToolCall(ToolCallBlock::new(
            "tc1".to_string(),
            "search".to_string(),
            "not json {".to_string(),
        ))],
    )];
    let err = msg_to_rig_messages(&msgs).unwrap_err();
    assert!(
        matches!(err, ModelError::FormatError { .. }),
        "expected FormatError, got {err:?}"
    );
}

#[test]
fn empty_tool_call_input_is_empty_object() {
    let msgs = vec![msg(
        "assistant",
        Role::Assistant,
        vec![ContentBlock::ToolCall(ToolCallBlock::new(
            "tc1".to_string(),
            "search".to_string(),
            "".to_string(),
        ))],
    )];
    let rig_msgs = msg_to_rig_messages(&msgs).unwrap();
    match &rig_msgs[0] {
        Message::Assistant { content, .. } => match content.first() {
            AssistantContent::ToolCall(tc) => {
                assert_eq!(tc.function.arguments, serde_json::json!({}));
            }
            other => panic!("expected tool call, got {other:?}"),
        },
        other => panic!("expected assistant message, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 入站
// ---------------------------------------------------------------------------

#[test]
fn inbound_text_maps_to_text_block() {
    let blocks = assistant_content_to_blocks(vec![AssistantContent::text("hi")]).unwrap();
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::Text(tb) => assert_eq!(tb.text, "hi"),
        other => panic!("expected text block, got {other:?}"),
    }
}

#[test]
fn inbound_reasoning_maps_to_thinking_block_with_id() {
    let blocks = assistant_content_to_blocks(vec![AssistantContent::reasoning("think")]).unwrap();
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::Thinking(tb) => {
            assert_eq!(tb.thinking, "think");
        }
        other => panic!("expected thinking block, got {other:?}"),
    }
}

#[test]
fn inbound_tool_call_maps_to_tool_call_block() {
    let blocks = assistant_content_to_blocks(vec![AssistantContent::tool_call(
        "tc1",
        "search",
        serde_json::json!({"q": "rust"}),
    )])
    .unwrap();
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::ToolCall(tc) => {
            assert_eq!(tc.id, "tc1");
            assert_eq!(tc.name, "search");
            assert_eq!(tc.input, r#"{"q":"rust"}"#);
        }
        other => panic!("expected tool call block, got {other:?}"),
    }
}

#[test]
fn inbound_image_base64_maps_to_data_block() {
    let blocks = assistant_content_to_blocks(vec![AssistantContent::image_base64(
        "aGVsbG8=",
        Some(ImageMediaType::PNG),
        None,
    )])
    .unwrap();
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::Data(db) => match &db.source {
            DataSource::Base64(src) => {
                assert_eq!(src.data, "aGVsbG8=");
                assert_eq!(src.media_type, "image/png");
            }
            other => panic!("expected base64 source, got {other:?}"),
        },
        other => panic!("expected data block, got {other:?}"),
    }
}

#[test]
fn inbound_image_url_maps_to_data_block() {
    let img = Image {
        data: DocumentSourceKind::url("https://example.com/a.png"),
        media_type: Some(ImageMediaType::PNG),
        detail: None,
        additional_params: None,
    };
    let blocks = assistant_content_to_blocks(vec![AssistantContent::Image(img)]).unwrap();
    match &blocks[0] {
        ContentBlock::Data(db) => match &db.source {
            DataSource::Url(src) => {
                assert_eq!(src.url, "https://example.com/a.png");
                assert_eq!(src.media_type, "image/png");
            }
            other => panic!("expected url source, got {other:?}"),
        },
        other => panic!("expected data block, got {other:?}"),
    }
}

#[test]
fn inbound_empty_choice_errors() {
    let err = assistant_content_to_blocks(vec![]).unwrap_err();
    assert!(
        matches!(err, ModelError::FormatError { .. }),
        "expected FormatError for empty choice, got {err:?}"
    );
}
