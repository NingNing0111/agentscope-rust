//! ContentBlock types — the building blocks of AgentScope messages.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::source::{Base64Source, URLSource};
use crate::state::{ToolCallState, ToolResultState};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn default_id() -> String {
    agent_scope_utils::id::generate_id()
}

fn default_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ---------------------------------------------------------------------------
// PermissionRule (placeholder)
// ---------------------------------------------------------------------------

/// Placeholder permission rule — will be replaced by the permission module.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionRule {
    #[serde(flatten)]
    pub extras: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// TextBlock
// ---------------------------------------------------------------------------

/// Plain text content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub text: String,
    #[serde(default = "default_id")]
    pub id: String,
    #[serde(default = "default_timestamp")]
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}

impl TextBlock {
    pub fn new(text: String) -> Self {
        Self {
            text,
            id: default_id(),
            created_at: default_timestamp(),
            finished_at: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ThinkingBlock
// ---------------------------------------------------------------------------

/// Model reasoning content block with provider-specific field passthrough.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingBlock {
    pub thinking: String,
    #[serde(default = "default_id")]
    pub id: String,
    #[serde(default = "default_timestamp")]
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    /// Provider-specific extra fields (e.g. Anthropic `signature`, `redacted_thinking_data`).
    #[serde(flatten)]
    pub extras: HashMap<String, serde_json::Value>,
}

impl ThinkingBlock {
    pub fn new(thinking: String) -> Self {
        Self {
            thinking,
            id: default_id(),
            created_at: default_timestamp(),
            finished_at: None,
            extras: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// HintBlock
// ---------------------------------------------------------------------------

/// Content for a HintBlock — either a plain string or a list of blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HintContent {
    Text(String),
    Blocks(Vec<HintBlockItem>),
}

/// An item inside a HintBlock's block list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HintBlockItem {
    #[serde(rename = "text")]
    Text(TextBlock),
    #[serde(rename = "data")]
    Data(DataBlock),
}

/// Hint / instruction content block — one-shot, non-streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HintBlock {
    pub hint: HintContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default = "default_id")]
    pub id: String,
    #[serde(default = "default_timestamp")]
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}

impl HintBlock {
    pub fn new(hint: HintContent) -> Self {
        Self {
            hint,
            source: None,
            id: default_id(),
            created_at: default_timestamp(),
            finished_at: None,
        }
    }
}

// ---------------------------------------------------------------------------
// DataBlock
// ---------------------------------------------------------------------------

/// Discriminated data source — base64 or URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DataSource {
    #[serde(rename = "base64")]
    Base64(Base64Source),
    #[serde(rename = "url")]
    Url(URLSource),
}

/// Binary data content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataBlock {
    pub source: DataSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "default_id")]
    pub id: String,
    #[serde(default = "default_timestamp")]
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}

impl DataBlock {
    pub fn new(source: DataSource) -> Self {
        Self {
            source,
            name: None,
            id: default_id(),
            created_at: default_timestamp(),
            finished_at: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ToolCallBlock
// ---------------------------------------------------------------------------

/// Tool call request content block with state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallBlock {
    pub id: String,
    pub name: String,
    /// Raw JSON arguments string (not parsed at Foundation layer).
    pub input: String,
    #[serde(default)]
    pub state: ToolCallState,
    #[serde(default)]
    pub suggested_rules: Vec<PermissionRule>,
    #[serde(default = "default_timestamp")]
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}

impl ToolCallBlock {
    pub fn new(id: String, name: String, input: String) -> Self {
        Self {
            id,
            name,
            input,
            state: ToolCallState::Pending,
            suggested_rules: Vec::new(),
            created_at: default_timestamp(),
            finished_at: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ToolResultBlock
// ---------------------------------------------------------------------------

/// Output content for a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolOutput {
    Text(String),
    Blocks(Vec<ToolResultBlockItem>),
}

/// An item inside a ToolResultBlock's block list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResultBlockItem {
    #[serde(rename = "text")]
    Text(TextBlock),
    #[serde(rename = "data")]
    Data(DataBlock),
}

/// Tool execution result content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultBlock {
    pub id: String,
    pub name: String,
    pub output: ToolOutput,
    #[serde(default)]
    pub state: ToolResultState,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    #[serde(default = "default_timestamp")]
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    /// True when this is the final chunk in a streaming tool result.
    #[serde(default)]
    pub is_last: bool,
}

impl ToolResultBlock {
    pub fn new(id: String, name: String, output: ToolOutput) -> Self {
        Self {
            id,
            name,
            output,
            state: ToolResultState::Running,
            metadata: HashMap::new(),
            created_at: default_timestamp(),
            finished_at: None,
            is_last: false,
        }
    }
}

// ---------------------------------------------------------------------------
// ContentBlock — tagged union
// ---------------------------------------------------------------------------

/// Discriminated union of all content block types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text(TextBlock),
    #[serde(rename = "thinking")]
    Thinking(ThinkingBlock),
    #[serde(rename = "hint")]
    Hint(HintBlock),
    #[serde(rename = "data")]
    Data(DataBlock),
    #[serde(rename = "tool_call")]
    ToolCall(ToolCallBlock),
    #[serde(rename = "tool_result")]
    ToolResult(ToolResultBlock),
    /// Catch-all for forward compatibility with unknown block types.
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// BlockType — runtime type discriminator
// ---------------------------------------------------------------------------

/// Runtime discriminator for filtering content blocks by type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    Text,
    Thinking,
    Hint,
    Data,
    ToolCall,
    ToolResult,
}

impl ContentBlock {
    /// Return the runtime BlockType for this content block.
    pub fn block_type(&self) -> BlockType {
        match self {
            ContentBlock::Text(_) => BlockType::Text,
            ContentBlock::Thinking(_) => BlockType::Thinking,
            ContentBlock::Hint(_) => BlockType::Hint,
            ContentBlock::Data(_) => BlockType::Data,
            ContentBlock::ToolCall(_) => BlockType::ToolCall,
            ContentBlock::ToolResult(_) => BlockType::ToolResult,
            ContentBlock::Unknown => BlockType::Text, // best-effort fallback
        }
    }
}

// ---------------------------------------------------------------------------
// From impls for ergonomic construction
// ---------------------------------------------------------------------------

impl From<TextBlock> for ContentBlock {
    fn from(b: TextBlock) -> Self {
        ContentBlock::Text(b)
    }
}
impl From<ThinkingBlock> for ContentBlock {
    fn from(b: ThinkingBlock) -> Self {
        ContentBlock::Thinking(b)
    }
}
impl From<HintBlock> for ContentBlock {
    fn from(b: HintBlock) -> Self {
        ContentBlock::Hint(b)
    }
}
impl From<DataBlock> for ContentBlock {
    fn from(b: DataBlock) -> Self {
        ContentBlock::Data(b)
    }
}
impl From<ToolCallBlock> for ContentBlock {
    fn from(b: ToolCallBlock) -> Self {
        ContentBlock::ToolCall(b)
    }
}
impl From<ToolResultBlock> for ContentBlock {
    fn from(b: ToolResultBlock) -> Self {
        ContentBlock::ToolResult(b)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- TextBlock --
    #[test]
    fn test_text_block_creation() {
        let tb = TextBlock::new("Hello".into());
        assert_eq!(tb.text, "Hello");
        assert!(!tb.id.is_empty());
        assert!(!tb.created_at.is_empty());
        assert!(tb.finished_at.is_none());
    }

    #[test]
    fn test_text_block_json_roundtrip() {
        let tb = TextBlock::new("Hello, world!".into());
        let json = serde_json::to_string(&tb).unwrap();
        let restored: TextBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.text, "Hello, world!");
    }

    #[test]
    fn test_text_block_json_has_type_tag() {
        let cb = ContentBlock::Text(TextBlock::new("hi".into()));
        let json = serde_json::to_string(&cb).unwrap();
        assert!(json.contains(r#""type":"text""#));
    }

    // -- ThinkingBlock --
    #[test]
    fn test_thinking_block_creation() {
        let tb = ThinkingBlock::new("reasoning...".into());
        assert_eq!(tb.thinking, "reasoning...");
        assert!(tb.extras.is_empty());
    }

    #[test]
    fn test_thinking_block_json_roundtrip() {
        let tb = ThinkingBlock::new("Let me think...".into());
        let json = serde_json::to_string(&tb).unwrap();
        let restored: ThinkingBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.thinking, "Let me think...");
    }

    #[test]
    fn test_thinking_block_json_has_type_tag() {
        let cb = ContentBlock::Thinking(ThinkingBlock::new("...".into()));
        let json = serde_json::to_string(&cb).unwrap();
        assert!(json.contains(r#""type":"thinking""#));
    }

    // -- HintBlock --
    #[test]
    fn test_hint_block_text_content() {
        let hb = HintBlock::new(HintContent::Text("a hint".into()));
        if let HintContent::Text(t) = &hb.hint {
            assert_eq!(t, "a hint");
        } else {
            panic!("expected Text variant");
        }
    }

    #[test]
    fn test_hint_block_json_roundtrip() {
        let cb = ContentBlock::Hint(HintBlock::new(HintContent::Text("hint".into())));
        let json = serde_json::to_string(&cb).unwrap();
        assert!(json.contains(r#""type":"hint""#));
        let restored: ContentBlock = serde_json::from_str(&json).unwrap();
        assert!(matches!(restored, ContentBlock::Hint(_)));
    }

    // -- DataBlock --
    #[test]
    fn test_data_block_creation() {
        let source = DataSource::Base64(Base64Source {
            data: "aGVsbG8=".into(),
            media_type: "text/plain".into(),
        });
        let db = DataBlock::new(source);
        assert!(matches!(db.source, DataSource::Base64(_)));
    }

    #[test]
    fn test_data_block_json_has_type_tag() {
        let source = DataSource::Url(URLSource {
            url: "https://example.com/file.txt".into(),
            media_type: "text/plain".into(),
        });
        let cb = ContentBlock::Data(DataBlock::new(source));
        let json = serde_json::to_string(&cb).unwrap();
        assert!(json.contains(r#""type":"data""#));
    }

    // -- ToolCallBlock --
    #[test]
    fn test_tool_call_block_creation() {
        let tc = ToolCallBlock::new("tc-1".into(), "search".into(), r#"{"q":"test"}"#.into());
        assert_eq!(tc.name, "search");
        assert!(matches!(tc.state, ToolCallState::Pending));
        assert!(tc.suggested_rules.is_empty());
    }

    #[test]
    fn test_tool_call_block_json_helps_roundtrip() {
        let tc = ToolCallBlock::new("tc-1".into(), "search".into(), r#"{"q":"test"}"#.into());
        let cb = ContentBlock::ToolCall(tc);
        let json = serde_json::to_string(&cb).unwrap();
        assert!(json.contains(r#""type":"tool_call""#));
        assert!(json.contains(r#""state":"pending""#));
        let _restored: ContentBlock = serde_json::from_str(&json).unwrap();
    }

    // -- ToolResultBlock --
    #[test]
    fn test_tool_result_block_text_output() {
        let tr = ToolResultBlock::new(
            "tr-1".into(),
            "search".into(),
            ToolOutput::Text("results found".into()),
        );
        if let ToolOutput::Text(t) = &tr.output {
            assert_eq!(t, "results found");
        } else {
            panic!("expected Text variant");
        }
    }

    #[test]
    fn test_tool_result_block_json_has_type_tag() {
        let tr = ToolResultBlock::new(
            "tr-1".into(),
            "search".into(),
            ToolOutput::Text("done".into()),
        );
        let cb = ContentBlock::ToolResult(tr);
        let json = serde_json::to_string(&cb).unwrap();
        assert!(json.contains(r#""type":"tool_result""#));
    }

    // -- ContentBlock tagged enum --
    #[test]
    fn test_content_block_all_variants_serialize_with_type_tag() {
        let variants: Vec<ContentBlock> = vec![
            ContentBlock::Text(TextBlock::new("t".into())),
            ContentBlock::Thinking(ThinkingBlock::new("th".into())),
            ContentBlock::Hint(HintBlock::new(HintContent::Text("h".into()))),
            ContentBlock::Data(DataBlock::new(DataSource::Url(URLSource {
                url: "http://x".into(),
                media_type: "text/plain".into(),
            }))),
            ContentBlock::ToolCall(ToolCallBlock::new("id".into(), "n".into(), "{}".into())),
            ContentBlock::ToolResult(ToolResultBlock::new(
                "id".into(),
                "n".into(),
                ToolOutput::Text("o".into()),
            )),
        ];

        let expected_tags = [
            "text",
            "thinking",
            "hint",
            "data",
            "tool_call",
            "tool_result",
        ];
        for (cb, expected) in variants.iter().zip(expected_tags.iter()) {
            let json = serde_json::to_string(cb).unwrap();
            assert!(
                json.contains(&format!(r#""type":"{}""#, expected)),
                "expected type tag '{}' in JSON: {}",
                expected,
                json
            );
        }
    }

    // -- BlockType --
    #[test]
    fn test_block_type_matches_content_block() {
        assert_eq!(
            ContentBlock::Text(TextBlock::new("x".into())).block_type(),
            BlockType::Text
        );
        assert_eq!(
            ContentBlock::Thinking(ThinkingBlock::new("x".into())).block_type(),
            BlockType::Thinking
        );
        assert_eq!(
            ContentBlock::ToolCall(ToolCallBlock::new("i".into(), "n".into(), "{}".into()))
                .block_type(),
            BlockType::ToolCall
        );
    }

    // -- From impls --
    #[test]
    fn test_from_impls() {
        let _cb: ContentBlock = TextBlock::new("hello".into()).into();
        let _cb: ContentBlock = ThinkingBlock::new("think".into()).into();
    }

    // -- PermissionRule placeholder --
    #[test]
    fn test_permission_rule_default() {
        let pr = PermissionRule::default();
        assert!(pr.extras.is_empty());
    }
}
