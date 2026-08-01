//! Content block streaming events (text, data, thinking, hint).

use serde::{Deserialize, Serialize};

use agent_scope_message::HintContent;

use crate::base::EventBase;

// ---------------------------------------------------------------------------
// Text block events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlockStartEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub block_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlockDeltaEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub block_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlockEndEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub block_id: String,
    /// Complete text content for this block from Start to End.
    /// `Some("")` means known empty; `None` means unknown/unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

// ---------------------------------------------------------------------------
// Data block events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataBlockStartEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub block_id: String,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataBlockDeltaEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub block_id: String,
    pub data: String,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataBlockEndEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub block_id: String,
}

// ---------------------------------------------------------------------------
// Thinking block events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingBlockStartEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub block_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingBlockDeltaEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub block_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingBlockEndEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub block_id: String,
    /// Complete thinking content for this block from Start to End.
    /// `Some("")` means known empty; `None` means unknown/unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

// ---------------------------------------------------------------------------
// Hint block event (one-shot, non-streaming)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HintBlockEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub reply_id: String,
    pub block_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub hint: HintContent,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_block_delta_event_serialization() {
        let event = TextBlockDeltaEvent {
            base: EventBase::new(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            delta: "Hel".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""delta":"Hel""#));
        assert!(json.contains(r#""block_id":"block-001""#));
    }

    #[test]
    fn test_text_block_start_event_roundtrip() {
        let event = TextBlockStartEvent {
            base: EventBase::new(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: TextBlockStartEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.reply_id, "reply-001");
        assert_eq!(restored.block_id, "block-001");
    }

    #[test]
    fn test_data_block_delta_event_roundtrip() {
        let event = DataBlockDeltaEvent {
            base: EventBase::new(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            data: "aGVsbG8=".into(),
            media_type: "image/png".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: DataBlockDeltaEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.data, "aGVsbG8=");
        assert_eq!(restored.media_type, "image/png");
    }
}
