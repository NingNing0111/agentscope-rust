//! ChatUsage — extended token usage statistics for model calls.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Extended token usage statistics for a model call.
///
/// Different from `agent_scope_message::Usage` (which only has `input_tokens` and
/// `output_tokens`). This adds timing, prompt caching stats, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub time: f64,
    #[serde(default)]
    pub cache_creation_input_tokens: i64,
    #[serde(default)]
    pub cache_input_tokens: i64,
    #[serde(rename = "type")]
    pub usage_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, JsonValue>>,
}

impl Default for ChatUsage {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            time: 0.0,
            cache_creation_input_tokens: 0,
            cache_input_tokens: 0,
            usage_type: "chat".to_string(),
            metadata: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_usage_serde_round_trip() {
        let usage = ChatUsage {
            input_tokens: 100,
            output_tokens: 50,
            time: 2.5,
            cache_creation_input_tokens: 20,
            cache_input_tokens: 30,
            ..Default::default()
        };
        let json = serde_json::to_string(&usage).unwrap();
        let parsed: ChatUsage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.input_tokens, 100);
        assert_eq!(parsed.output_tokens, 50);
        assert_eq!(parsed.time, 2.5);
        assert_eq!(parsed.cache_creation_input_tokens, 20);
        assert_eq!(parsed.cache_input_tokens, 30);
        assert_eq!(parsed.usage_type, "chat");
    }

    #[test]
    fn test_chat_usage_defaults() {
        let usage = ChatUsage::default();
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert_eq!(usage.cache_input_tokens, 0);
        assert_eq!(usage.usage_type, "chat");
    }

    #[test]
    fn test_chat_usage_json_has_type_field() {
        let usage = ChatUsage::default();
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains(r#""type":"chat""#));
    }
}
