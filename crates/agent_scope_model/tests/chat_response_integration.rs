//! T097: Integration test — ChatResponse content blocks match
//! agent_scope_message ContentBlock types.

use agent_scope_message::{ContentBlock, DataBlock, DataSource};
use agent_scope_model::{ChatResponse, ChatUsage, FinishedReason};

#[test]
fn test_chat_response_to_msg_content_roundtrip() {
    // Create a ChatResponse with various content block types
    let mut resp = ChatResponse::default();
    resp.append_text("Hello, world!", Some("t1"));
    resp.append_text(" How are you?", Some("t1"));
    resp.is_last = true;
    resp.usage = Some(ChatUsage {
        input_tokens: 50,
        output_tokens: 25,
        time: 1.5,
        cache_creation_input_tokens: 0,
        cache_input_tokens: 10,
        usage_type: "chat".to_string(),
        metadata: None,
    });
    resp.finished_reason = FinishedReason::Completed;

    // Verify content block types match agent_scope_message
    assert_eq!(resp.content.len(), 1);
    let block = &resp.content[0];
    assert!(matches!(block, ContentBlock::Text(_)));
    if let ContentBlock::Text(tb) = block {
        assert_eq!(tb.text, "Hello, world! How are you?");
    }

    // Verify serialization
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["type"], "chat_response");
    assert_eq!(json["is_last"], true);

    // Deserialize back
    let parsed: ChatResponse = serde_json::from_value(json).unwrap();
    assert!(parsed.is_last);
    assert_eq!(parsed.get_text_content(""), "Hello, world! How are you?");
}

#[test]
fn test_chat_response_text_and_data_blocks() {
    use agent_scope_message::Base64Source;

    let mut resp = ChatResponse::default();
    resp.append_text("Look at this:", Some("text1"));

    // DataBlock in ChatResponse uses append_data_block
    let data_block = DataBlock::new(DataSource::Base64(Base64Source {
        data: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"fake_image"),
        media_type: "image/png".to_string(),
    }));
    resp.content.push(ContentBlock::Data(data_block));

    assert_eq!(resp.content.len(), 2);
    assert!(matches!(&resp.content[0], ContentBlock::Text(_)));
    assert!(matches!(&resp.content[1], ContentBlock::Data(_)));
}

#[test]
fn test_struct_to_chat_response_consistency() {
    // Verify that StructuredResponse can reference the same content types
    let mut resp = ChatResponse::default();
    resp.append_text("test", None);

    let json = serde_json::to_string(&resp).unwrap();
    // Ensure the serialized form is valid JSON
    let _parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
}
