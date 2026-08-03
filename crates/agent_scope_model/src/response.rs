//! ChatResponse, StructuredResponse, and FinishedReason types.

use std::collections::HashMap;

use agent_scope_message::{
    Base64Source, ContentBlock, DataBlock, DataSource, TextBlock, ThinkingBlock, ToolCallBlock,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::usage::ChatUsage;

/// Reason the model response finished.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FinishedReason {
    #[default]
    Completed,
    Interrupted,
}

/// A streaming or non-streaming model response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: Vec<ContentBlock>,
    pub is_last: bool,
    pub id: String,
    pub created_at: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub usage: Option<ChatUsage>,
    #[serde(default)]
    pub finished_reason: FinishedReason,
    #[serde(default)]
    pub metadata: HashMap<String, JsonValue>,
}

impl Default for ChatResponse {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            content: Vec::new(),
            is_last: false,
            id: uuid::Uuid::new_v4().as_simple().to_string(),
            created_at: now,
            response_type: "chat_response".to_string(),
            usage: None,
            finished_reason: FinishedReason::default(),
            metadata: HashMap::new(),
        }
    }
}

impl ChatResponse {
    /// Append text to a TextBlock identified by `block_id`.
    pub fn append_text(&mut self, text: &str, block_id: Option<&str>) -> &mut Self {
        if let Some(bid) = block_id {
            for block in &mut self.content {
                if let ContentBlock::Text(tb) = block
                    && tb.id == bid
                {
                    tb.text.push_str(text);
                    return self;
                }
            }
        }
        let mut tb = TextBlock::new(text.to_string());
        if let Some(bid) = block_id {
            tb.id = bid.to_string();
        }
        self.content.push(ContentBlock::Text(tb));
        self
    }

    /// Append thinking text to a ThinkingBlock identified by `block_id`.
    pub fn append_thinking(
        &mut self,
        thinking: &str,
        block_id: Option<&str>,
        extra_fields: HashMap<String, JsonValue>,
    ) -> &mut Self {
        if let Some(bid) = block_id {
            for block in &mut self.content {
                if let ContentBlock::Thinking(tb) = block
                    && tb.id == bid
                {
                    tb.thinking.push_str(thinking);
                    for (k, v) in extra_fields {
                        tb.extras.insert(k, v);
                    }
                    return self;
                }
            }
        }
        let mut tb = ThinkingBlock::new(thinking.to_string());
        if let Some(bid) = block_id {
            tb.id = bid.to_string();
        }
        tb.extras = extra_fields;
        self.content.push(ContentBlock::Thinking(tb));
        self
    }

    /// Append tool call input to a ToolCallBlock identified by `block_id`.
    ///
    /// Note: `extra_fields` are stored in response-level metadata keyed by tool call id
    /// since `ToolCallBlock` in the Foundation layer does not have an extras field.
    pub fn append_tool_call(
        &mut self,
        block_id: &str,
        name: &str,
        input: &str,
        extra_fields: HashMap<String, JsonValue>,
    ) -> &mut Self {
        for block in &mut self.content {
            if let ContentBlock::ToolCall(tc) = block
                && tc.id == block_id
            {
                tc.input.push_str(input);
                // Mirror StreamAccumulator::AccToolCallBlock::append: adopt the
                // name/id when they arrive in a later chunk (DashScope streams
                // may send the first chunk without them).
                if !name.is_empty() && tc.name.is_empty() {
                    tc.name = name.to_string();
                }
            }
        }
        // Store extras in response metadata for later use
        if !extra_fields.is_empty() {
            let key = format!("tool_call_extras_{}", block_id);
            if let Some(obj) = self
                .metadata
                .entry(key)
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
            {
                for (k, v) in extra_fields {
                    obj.insert(k, v);
                }
            }
        }
        // Check if we already matched an existing block
        if self
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolCall(tc) if tc.id == block_id))
        {
            return self;
        }

        // Create new ToolCallBlock
        let tc = ToolCallBlock::new(block_id.to_string(), name.to_string(), input.to_string());
        self.content.push(ContentBlock::ToolCall(tc));
        self
    }

    /// Append data to a DataBlock identified by `block_id`.
    pub fn append_data_block(
        &mut self,
        block_id: &str,
        data: &[u8],
        media_type: &str,
        _name: Option<&str>,
    ) -> &mut Self {
        let is_audio = media_type.starts_with("audio/");

        for block in &mut self.content {
            if let ContentBlock::Data(db) = block
                && db.id == block_id
            {
                if is_audio {
                    if let DataSource::Base64(bs) = &db.source {
                        let existing_bytes = base64::engine::general_purpose::STANDARD
                            .decode(&bs.data)
                            .unwrap_or_default();
                        let mut combined = existing_bytes;
                        combined.extend_from_slice(data);
                        if let DataSource::Base64(bs_mut) = &mut db.source {
                            bs_mut.data =
                                base64::engine::general_purpose::STANDARD.encode(&combined);
                        }
                    }
                } else {
                    db.source = DataSource::Base64(Base64Source {
                        data: base64::engine::general_purpose::STANDARD.encode(data),
                        media_type: media_type.to_string(),
                    });
                }
                return self;
            }
        }

        let mut db = DataBlock::new(DataSource::Base64(Base64Source {
            data: base64::engine::general_purpose::STANDARD.encode(data),
            media_type: media_type.to_string(),
        }));
        db.id = block_id.to_string();
        self.content.push(ContentBlock::Data(db));
        self
    }

    /// Merge another ChatResponse into this one by block_id matching.
    pub fn append_chat_response(&mut self, other: &ChatResponse) -> &mut Self {
        for other_block in &other.content {
            let (other_bid, _other_block_type) = match other_block {
                ContentBlock::Text(t) => (Some(t.id.clone()), "text"),
                ContentBlock::Thinking(t) => (Some(t.id.clone()), "thinking"),
                ContentBlock::Hint(h) => (Some(h.id.clone()), "hint"),
                ContentBlock::Data(d) => (Some(d.id.clone()), "data"),
                ContentBlock::ToolCall(t) => (Some(t.id.clone()), "tool_call"),
                ContentBlock::ToolResult(t) => (Some(t.id.clone()), "tool_result"),
                ContentBlock::Unknown => (None, "unknown"),
            };

            let mut matched = false;
            if let Some(bid) = other_bid {
                for self_block in &mut self.content {
                    match (self_block, other_block) {
                        (ContentBlock::Text(st), ContentBlock::Text(ot)) => {
                            if st.id == *bid {
                                st.text.push_str(&ot.text);
                                matched = true;
                                break;
                            }
                        }
                        (ContentBlock::Thinking(st), ContentBlock::Thinking(ot)) => {
                            if st.id == *bid {
                                st.thinking.push_str(&ot.thinking);
                                for (k, v) in &ot.extras {
                                    st.extras.insert(k.clone(), v.clone());
                                }
                                matched = true;
                                break;
                            }
                        }
                        (ContentBlock::ToolCall(st), ContentBlock::ToolCall(ot)) => {
                            if st.id == *bid {
                                st.input.push_str(&ot.input);
                                matched = true;
                                break;
                            }
                        }
                        (ContentBlock::Data(st), ContentBlock::Data(ot)) => {
                            if st.id == *bid {
                                match (&st.source, &ot.source) {
                                    (DataSource::Base64(ss), DataSource::Base64(os))
                                        if ss.media_type.starts_with("audio/") =>
                                    {
                                        let existing = base64::engine::general_purpose::STANDARD
                                            .decode(&ss.data)
                                            .unwrap_or_default();
                                        let incoming = base64::engine::general_purpose::STANDARD
                                            .decode(&os.data)
                                            .unwrap_or_default();
                                        let mut combined = existing;
                                        combined.extend_from_slice(&incoming);
                                        if let DataSource::Base64(ss_mut) = &mut st.source {
                                            ss_mut.data = base64::engine::general_purpose::STANDARD
                                                .encode(&combined);
                                        }
                                    }
                                    _ => {
                                        st.source = ot.source.clone();
                                    }
                                }
                                matched = true;
                                break;
                            }
                        }
                        _ => continue,
                    }
                }
            }

            if !matched {
                self.content.push(other_block.clone());
            }
        }

        if self.usage.is_none() && other.usage.is_some() {
            self.usage = other.usage.clone();
        } else if let (Some(s_usage), Some(o_usage)) = (&mut self.usage, &other.usage) {
            s_usage.input_tokens += o_usage.input_tokens;
            s_usage.output_tokens += o_usage.output_tokens;
            s_usage.cache_creation_input_tokens += o_usage.cache_creation_input_tokens;
            s_usage.cache_input_tokens += o_usage.cache_input_tokens;
            s_usage.time += o_usage.time;
        }

        self
    }

    /// Get concatenated text content from all TextBlocks.
    pub fn get_text_content(&self, separator: &str) -> String {
        self.content
            .iter()
            .filter_map(|block| {
                if let ContentBlock::Text(tb) = block {
                    Some(tb.text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<&str>>()
            .join(separator)
    }
}

/// Structured output response from `generate_structured_output()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredResponse {
    pub content: JsonValue,
    pub id: String,
    pub created_at: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub usage: Option<ChatUsage>,
    #[serde(default)]
    pub metadata: HashMap<String, JsonValue>,
    #[serde(default)]
    pub finished_reason: FinishedReason,
}

impl Default for StructuredResponse {
    fn default() -> Self {
        Self {
            content: JsonValue::Null,
            id: uuid::Uuid::new_v4().as_simple().to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            response_type: "structured_response".to_string(),
            usage: None,
            metadata: HashMap::new(),
            finished_reason: FinishedReason::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finished_reason_serialization() {
        assert_eq!(
            serde_json::to_string(&FinishedReason::Completed).unwrap(),
            r#""completed""#
        );
        assert_eq!(
            serde_json::to_string(&FinishedReason::Interrupted).unwrap(),
            r#""interrupted""#
        );
    }

    #[test]
    fn test_append_text_same_block_id() {
        let mut resp = ChatResponse::default();
        resp.append_text("Hel", Some("b1"));
        resp.append_text("lo", Some("b1"));
        assert_eq!(resp.get_text_content(""), "Hello");
        assert_eq!(resp.content.len(), 1);
    }

    #[test]
    fn test_append_text_different_block_id() {
        let mut resp = ChatResponse::default();
        resp.append_text("First", Some("b1"));
        resp.append_text("Second", Some("b2"));
        assert_eq!(resp.get_text_content(" "), "First Second");
        assert_eq!(resp.content.len(), 2);
    }

    #[test]
    fn test_append_thinking_accumulate() {
        let mut resp = ChatResponse::default();
        resp.append_thinking("Think step 1. ", Some("t1"), HashMap::new());
        let mut extras = HashMap::new();
        extras.insert("signature".to_string(), serde_json::json!("sig123"));
        resp.append_thinking("Think step 2.", Some("t1"), extras);
        assert_eq!(resp.content.len(), 1);
        if let ContentBlock::Thinking(tb) = &resp.content[0] {
            assert_eq!(tb.thinking, "Think step 1. Think step 2.");
            assert_eq!(
                tb.extras.get("signature").unwrap(),
                &serde_json::json!("sig123")
            );
        } else {
            panic!("Expected ThinkingBlock");
        }
    }

    #[test]
    fn test_append_tool_call_accumulate() {
        let mut resp = ChatResponse::default();
        resp.append_tool_call("tc1", "search", r#"{"q":"#, HashMap::new());
        resp.append_tool_call("tc1", "search", r#""test"}"#, HashMap::new());
        assert_eq!(resp.content.len(), 1);
        if let ContentBlock::ToolCall(tc) = &resp.content[0] {
            assert_eq!(tc.input, r#"{"q":"test"}"#);
            assert_eq!(tc.name, "search");
        } else {
            panic!("Expected ToolCallBlock");
        }
    }

    #[test]
    fn test_append_data_block_audio_concat() {
        let mut resp = ChatResponse::default();
        resp.append_data_block("d1", b"hello", "audio/pcm16", None);
        resp.append_data_block("d1", b"world", "audio/pcm16", None);
        if let ContentBlock::Data(db) = &resp.content[0]
            && let DataSource::Base64(bs) = &db.source
        {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(&bs.data)
                .unwrap();
            assert_eq!(decoded, b"helloworld");
        }
    }

    #[test]
    fn test_append_chat_response_merge() {
        let mut a = ChatResponse::default();
        a.append_text("Hello", Some("t1"));
        let mut b = ChatResponse::default();
        b.append_text(" World", Some("t1"));
        b.append_text("Extra", Some("t2"));
        b.usage = Some(ChatUsage::default());
        a.append_chat_response(&b);
        assert_eq!(a.get_text_content(""), "Hello WorldExtra");
        assert!(a.usage.is_some());
    }

    #[test]
    fn test_chat_response_serde_round_trip() {
        let mut resp = ChatResponse::default();
        resp.append_text("Hello", None);
        resp.is_last = true;
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ChatResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.response_type, "chat_response");
        assert!(parsed.is_last);
    }

    #[test]
    fn test_structured_response_serde_round_trip() {
        let sr = StructuredResponse {
            content: serde_json::json!({"result": "ok"}),
            ..Default::default()
        };
        let json = serde_json::to_string(&sr).unwrap();
        let parsed: StructuredResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.response_type, "structured_response");
        assert_eq!(parsed.content, serde_json::json!({"result": "ok"}));
    }
}
