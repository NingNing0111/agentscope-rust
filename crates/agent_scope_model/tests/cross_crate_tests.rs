//! T101: Cross-crate consistency test — verify ChatResponse serialized
//! from model crate correctly uses agent_scope_message ContentBlock types.

use agent_scope_message::ContentBlock;
use agent_scope_model::ChatResponse;

/// Verify that ChatResponse → JSON → ChatResponse round-trip preserves
/// ContentBlock type tags from agent_scope_message.
#[test]
fn test_cross_crate_serde_compatibility() {
    let mut resp = ChatResponse::default();
    resp.append_text("Hello, cross-crate!", None);
    resp.is_last = true;

    // Serialize
    let json = serde_json::to_string(&resp).unwrap();

    // Deserialize back — this exercises both crate's serde impls
    let parsed: ChatResponse = serde_json::from_str(&json).unwrap();

    assert!(parsed.is_last);
    assert_eq!(parsed.response_type, "chat_response");

    // Verify ContentBlock types are correctly deserialized
    for block in &parsed.content {
        match block {
            ContentBlock::Text(tb) => {
                assert_eq!(tb.text, "Hello, cross-crate!");
            }
            ContentBlock::Thinking(_) => {}
            ContentBlock::ToolCall(_) => {}
            ContentBlock::Data(_) => {}
            ContentBlock::Hint(_) => {}
            ContentBlock::ToolResult(_) => {}
            ContentBlock::Unknown => {}
        }
    }
}

/// Verify the JSON format includes the `type` discriminant for ChatResponse.
#[test]
fn test_cross_crate_type_discriminant() {
    let resp = ChatResponse::default();
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["type"], "chat_response");
    // Also verify ContentBlock array is present
    assert!(json["content"].is_array());
}
