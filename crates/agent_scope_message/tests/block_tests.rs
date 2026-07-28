//! Integration tests for ContentBlock tagged serialization (all 6 variants).
//! T103

use agent_scope_message::block::{
    ContentBlock, DataBlock, DataSource, HintBlock, HintContent, TextBlock, ThinkingBlock,
    ToolCallBlock, ToolOutput, ToolResultBlock,
};
use agent_scope_message::source::{Base64Source, URLSource};

#[test]
fn test_all_six_content_block_variants_serialize_with_correct_type_tag() {
    let variants: Vec<(&str, ContentBlock)> = vec![
        ("text", ContentBlock::Text(TextBlock::new("hello".into()))),
        (
            "thinking",
            ContentBlock::Thinking(ThinkingBlock::new("reasoning...".into())),
        ),
        (
            "hint",
            ContentBlock::Hint(HintBlock::new(HintContent::Text("tip".into()))),
        ),
        (
            "data",
            ContentBlock::Data(DataBlock::new(DataSource::Url(URLSource {
                url: "https://example.com/file.txt".into(),
                media_type: "text/plain".into(),
            }))),
        ),
        (
            "tool_call",
            ContentBlock::ToolCall(ToolCallBlock::new(
                "tc-1".into(),
                "search".into(),
                r#"{"q":"test"}"#.into(),
            )),
        ),
        (
            "tool_result",
            ContentBlock::ToolResult(ToolResultBlock::new(
                "tr-1".into(),
                "search".into(),
                ToolOutput::Text("results".into()),
            )),
        ),
    ];

    assert_eq!(variants.len(), 6, "must test all 6 ContentBlock variants");

    for (expected_tag, block) in variants {
        let json = serde_json::to_string(&block).unwrap();
        let expected = format!(r#""type":"{}""#, expected_tag);
        assert!(
            json.contains(&expected),
            "ContentBlock variant should have type tag '{}', got: {}",
            expected_tag,
            json
        );

        // Verify round-trip
        let restored: ContentBlock = serde_json::from_str(&json).unwrap();
        let restored_json = serde_json::to_string(&restored).unwrap();
        assert!(
            restored_json.contains(&expected),
            "Round-tripped ContentBlock should still have type tag '{}'",
            expected_tag
        );
    }
}

#[test]
fn test_content_block_text_roundtrip_preserves_text() {
    let orig = ContentBlock::Text(TextBlock::new("Hello, agent!".into()));
    let json = serde_json::to_string(&orig).unwrap();
    let restored: ContentBlock = serde_json::from_str(&json).unwrap();

    match restored {
        ContentBlock::Text(tb) => assert_eq!(tb.text, "Hello, agent!"),
        _ => panic!("Expected Text variant"),
    }
}

#[test]
fn test_content_block_tool_call_preserves_input() {
    let orig = ContentBlock::ToolCall(ToolCallBlock::new(
        "call-abc".into(),
        "get_weather".into(),
        r#"{"city":"Beijing"}"#.into(),
    ));
    let json = serde_json::to_string(&orig).unwrap();
    let restored: ContentBlock = serde_json::from_str(&json).unwrap();

    match restored {
        ContentBlock::ToolCall(tc) => {
            assert_eq!(tc.id, "call-abc");
            assert_eq!(tc.name, "get_weather");
            assert_eq!(tc.input, r#"{"city":"Beijing"}"#);
        }
        _ => panic!("Expected ToolCall variant"),
    }
}

#[test]
fn test_content_block_data_base64_roundtrip() {
    let orig = ContentBlock::Data(DataBlock::new(DataSource::Base64(Base64Source {
        data: "SGVsbG8=".into(),
        media_type: "text/plain".into(),
    })));
    let json = serde_json::to_string(&orig).unwrap();
    let restored: ContentBlock = serde_json::from_str(&json).unwrap();

    match restored {
        ContentBlock::Data(db) => {
            if let DataSource::Base64(bs) = &db.source {
                assert_eq!(bs.data, "SGVsbG8=");
                assert_eq!(bs.media_type, "text/plain");
            } else {
                panic!("Expected Base64 source");
            }
        }
        _ => panic!("Expected Data variant"),
    }
}

#[test]
fn test_content_block_tool_result_preserves_state() {
    use agent_scope_message::state::ToolResultState;

    let mut tr = ToolResultBlock::new(
        "tr-1".into(),
        "calculator".into(),
        ToolOutput::Text("42".into()),
    );
    tr.state = ToolResultState::Success;

    let orig = ContentBlock::ToolResult(tr);
    let json = serde_json::to_string(&orig).unwrap();
    let restored: ContentBlock = serde_json::from_str(&json).unwrap();

    match restored {
        ContentBlock::ToolResult(tr) => {
            assert_eq!(tr.id, "tr-1");
            assert_eq!(tr.name, "calculator");
            assert!(matches!(tr.state, ToolResultState::Success));
        }
        _ => panic!("Expected ToolResult variant"),
    }
}

#[test]
fn test_unknown_content_block_handled() {
    let json = r#"{"type":"future_block_type","data":"some_value"}"#;
    let result: Result<ContentBlock, _> = serde_json::from_str(json);
    // The #[serde(other)] fallback should handle this
    assert!(
        result.is_ok(),
        "Unknown block types should not break deserialization"
    );
}

#[test]
fn test_content_block_block_type_method_accuracy() {
    let cases: Vec<(ContentBlock, agent_scope_message::block::BlockType)> = vec![
        (
            ContentBlock::Text(TextBlock::new("t".into())),
            agent_scope_message::block::BlockType::Text,
        ),
        (
            ContentBlock::Thinking(ThinkingBlock::new("think".into())),
            agent_scope_message::block::BlockType::Thinking,
        ),
        (
            ContentBlock::Hint(HintBlock::new(HintContent::Text("hint".into()))),
            agent_scope_message::block::BlockType::Hint,
        ),
        (
            ContentBlock::Data(DataBlock::new(DataSource::Url(URLSource {
                url: "http://x".into(),
                media_type: "text/plain".into(),
            }))),
            agent_scope_message::block::BlockType::Data,
        ),
        (
            ContentBlock::ToolCall(ToolCallBlock::new("i".into(), "n".into(), "{}".into())),
            agent_scope_message::block::BlockType::ToolCall,
        ),
        (
            ContentBlock::ToolResult(ToolResultBlock::new(
                "i".into(),
                "n".into(),
                ToolOutput::Text("o".into()),
            )),
            agent_scope_message::block::BlockType::ToolResult,
        ),
    ];

    assert_eq!(cases.len(), 6);
    for (block, expected_block_type) in cases {
        assert_eq!(
            block.block_type(),
            expected_block_type,
            "block_type() should match variant"
        );
    }
}

#[test]
fn test_content_block_from_impls() {
    let _: ContentBlock = TextBlock::new("hello".into()).into();
    let _: ContentBlock = ThinkingBlock::new("think".into()).into();
    let _: ContentBlock = HintBlock::new(HintContent::Text("hint".into())).into();
    let _: ContentBlock = DataBlock::new(DataSource::Url(URLSource {
        url: "http://x".into(),
        media_type: "text/plain".into(),
    }))
    .into();
    let _: ContentBlock = ToolCallBlock::new("i".into(), "n".into(), "{}".into()).into();
    let _: ContentBlock =
        ToolResultBlock::new("i".into(), "n".into(), ToolOutput::Text("o".into())).into();
}
