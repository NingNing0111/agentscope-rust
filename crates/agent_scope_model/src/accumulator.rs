//! StreamAccumulator — O(n) accumulation of streaming ChatResponse deltas.

use std::collections::HashMap;

use agent_scope_message::{
    Base64Source, ContentBlock, DataBlock, DataSource, TextBlock, ThinkingBlock, ToolCallBlock,
};
use base64::Engine;
use serde_json::Value as JsonValue;

use crate::response::{ChatResponse, FinishedReason};
use crate::usage::ChatUsage;

// ── Internal Accumulators ───────────────────────────────────────────────

#[derive(Debug, Clone)]
struct AccTextBlock {
    text: Vec<String>,
    id: String,
    created_at: String,
}

impl AccTextBlock {
    fn from_block(block: &TextBlock) -> Self {
        Self {
            text: vec![block.text.clone()],
            id: block.id.clone(),
            created_at: block.created_at.clone(),
        }
    }
    fn append(&mut self, block: &TextBlock) {
        self.text.push(block.text.clone());
    }
    fn build(self) -> TextBlock {
        TextBlock {
            text: self.text.concat(),
            id: self.id,
            created_at: self.created_at,
            finished_at: None,
        }
    }
}

#[derive(Debug, Clone)]
struct AccThinkingBlock {
    thinking: Vec<String>,
    extras: HashMap<String, JsonValue>,
    id: String,
    created_at: String,
}

impl AccThinkingBlock {
    fn from_block(block: &ThinkingBlock) -> Self {
        Self {
            thinking: vec![block.thinking.clone()],
            extras: block.extras.clone(),
            id: block.id.clone(),
            created_at: block.created_at.clone(),
        }
    }
    fn append(&mut self, block: &ThinkingBlock) {
        self.thinking.push(block.thinking.clone());
        for (k, v) in &block.extras {
            self.extras.insert(k.clone(), v.clone());
        }
    }
    fn build(self) -> ThinkingBlock {
        ThinkingBlock {
            thinking: self.thinking.concat(),
            id: self.id,
            created_at: self.created_at,
            finished_at: None,
            extras: self.extras,
        }
    }
}

#[derive(Debug, Clone)]
struct AccToolCallBlock {
    input: Vec<String>,
    name: String,
    id: String,
    #[allow(dead_code)]
    created_at: String,
}

impl AccToolCallBlock {
    fn from_block(block: &ToolCallBlock) -> Self {
        Self {
            input: vec![block.input.clone()],
            name: block.name.clone(),
            id: block.id.clone(),
            created_at: block.created_at.clone(),
        }
    }
    fn append(&mut self, block: &ToolCallBlock) {
        self.input.push(block.input.clone());
        if !block.name.is_empty() && self.name.is_empty() {
            self.name = block.name.clone();
        }
        // Update id when it arrives in a later SSE chunk (DashScope streams
        // send id after the first chunk without it)
        if !block.id.is_empty() {
            self.id = block.id.clone();
        }
    }
    fn build(self) -> ToolCallBlock {
        ToolCallBlock::new(self.id, self.name, self.input.concat())
    }
}

#[derive(Debug, Clone)]
struct AccBase64Source {
    data: Vec<Vec<u8>>,
    media_type: String,
}

impl AccBase64Source {
    fn from_source(source: &Base64Source) -> Self {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&source.data)
            .unwrap_or_default();
        Self {
            data: vec![decoded],
            media_type: source.media_type.clone(),
        }
    }
    fn append(&mut self, source: &Base64Source) {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&source.data)
            .unwrap_or_default();
        self.data.push(decoded);
    }
    fn build(self) -> Base64Source {
        let combined: Vec<u8> = self.data.into_iter().flatten().collect();
        Base64Source {
            data: base64::engine::general_purpose::STANDARD.encode(&combined),
            media_type: self.media_type,
        }
    }
}

#[derive(Debug, Clone)]
enum AccDataSource {
    Audio(AccBase64Source),
    Other(DataSource),
}

#[derive(Debug, Clone)]
struct AccDataBlock {
    source: AccDataSource,
    id: String,
    created_at: String,
    name: Option<String>,
}

impl AccDataBlock {
    fn from_block(block: &DataBlock) -> Self {
        let source = match &block.source {
            DataSource::Base64(bs) if bs.media_type.starts_with("audio/") => {
                AccDataSource::Audio(AccBase64Source::from_source(bs))
            }
            other => AccDataSource::Other(other.clone()),
        };
        Self {
            source,
            id: block.id.clone(),
            created_at: block.created_at.clone(),
            name: block.name.clone(),
        }
    }
    fn append(&mut self, block: &DataBlock) {
        match (&mut self.source, &block.source) {
            (AccDataSource::Audio(acc), DataSource::Base64(bs))
                if bs.media_type.starts_with("audio/") =>
            {
                acc.append(bs)
            }
            _ => {
                self.source = match &block.source {
                    DataSource::Base64(bs) if bs.media_type.starts_with("audio/") => {
                        AccDataSource::Audio(AccBase64Source::from_source(bs))
                    }
                    other => AccDataSource::Other(other.clone()),
                };
            }
        }
        if block.name.is_some() {
            self.name = block.name.clone();
        }
    }
    fn build(self) -> DataBlock {
        let source = match self.source {
            AccDataSource::Audio(audio) => DataSource::Base64(audio.build()),
            AccDataSource::Other(src) => src,
        };
        DataBlock {
            source,
            id: self.id,
            created_at: self.created_at,
            name: self.name,
            finished_at: None,
        }
    }
}

#[derive(Debug, Clone)]
enum AccBlock {
    Text(AccTextBlock),
    Thinking(AccThinkingBlock),
    ToolCall(AccToolCallBlock),
    Data(AccDataBlock),
}

impl AccBlock {
    fn type_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Thinking(_) => "thinking",
            Self::ToolCall(_) => "tool_call",
            Self::Data(_) => "data",
        }
    }
}

// ── StreamAccumulator ───────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct StreamAccumulator {
    /// Content blocks in first-seen order. A `Vec` (rather than a `HashMap`)
    /// preserves the order in which blocks arrived, so the rebuilt response
    /// keeps the streaming chunk order (thinking → text → tool_call, ...).
    blocks: Vec<(String, AccBlock)>,
    id: Option<String>,
    usage: Option<ChatUsage>,
    finished_reason: FinishedReason,
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append_chat_response(&mut self, delta: &ChatResponse) {
        if !delta.id.is_empty() {
            self.id = Some(delta.id.clone());
        }
        if delta.usage.is_some() {
            self.usage = delta.usage.clone();
        }
        if delta.finished_reason != FinishedReason::Completed {
            self.finished_reason = delta.finished_reason.clone();
        }

        for block in &delta.content {
            let (bid, new_acc): (String, Option<AccBlock>) = match block {
                ContentBlock::Text(tb) => (
                    tb.id.clone(),
                    Some(AccBlock::Text(AccTextBlock::from_block(tb))),
                ),
                ContentBlock::Thinking(tb) => (
                    tb.id.clone(),
                    Some(AccBlock::Thinking(AccThinkingBlock::from_block(tb))),
                ),
                ContentBlock::ToolCall(tc) => (
                    tc.id.clone(),
                    Some(AccBlock::ToolCall(AccToolCallBlock::from_block(tc))),
                ),
                ContentBlock::Data(db) => (
                    db.id.clone(),
                    Some(AccBlock::Data(AccDataBlock::from_block(db))),
                ),
                ContentBlock::Hint(_) | ContentBlock::ToolResult(_) | ContentBlock::Unknown => {
                    continue;
                }
            };

            if let Some(acc) = new_acc {
                match self.blocks.iter_mut().find(|(bid2, _)| *bid2 == bid) {
                    Some((_, existing)) => {
                        if existing.type_name() != acc.type_name() {
                            eprintln!(
                                "WARNING: Block type changed for id '{}' from '{}' to '{}'. Dropping old accumulator.",
                                bid,
                                existing.type_name(),
                                acc.type_name()
                            );
                            if let Some(slot) = self
                                .blocks
                                .iter_mut()
                                .find(|(bid2, _)| *bid2 == bid)
                            {
                                slot.1 = acc;
                            }
                        } else {
                            match (existing, block) {
                                (AccBlock::Text(a), ContentBlock::Text(tb)) => a.append(tb),
                                (AccBlock::Thinking(a), ContentBlock::Thinking(tb)) => a.append(tb),
                                (AccBlock::ToolCall(a), ContentBlock::ToolCall(tc)) => a.append(tc),
                                (AccBlock::Data(a), ContentBlock::Data(db)) => a.append(db),
                                _ => {}
                            }
                        }
                    }
                    None => {
                        self.blocks.push((bid, acc));
                    }
                }
            }
        }
    }

    pub fn build(self) -> ChatResponse {
        let content: Vec<ContentBlock> = self
            .blocks
            .into_iter()
            .map(|(_, acc)| match acc {
                AccBlock::Text(a) => ContentBlock::Text(a.build()),
                AccBlock::Thinking(a) => ContentBlock::Thinking(a.build()),
                AccBlock::ToolCall(a) => ContentBlock::ToolCall(a.build()),
                AccBlock::Data(a) => ContentBlock::Data(a.build()),
            })
            .collect();

        ChatResponse {
            content,
            is_last: true,
            id: self.id.unwrap_or_default(),
            created_at: chrono::Utc::now().to_rfc3339(),
            response_type: "chat_response".to_string(),
            usage: self.usage,
            finished_reason: self.finished_reason,
            metadata: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_delta(text: &str, block_id: &str) -> ChatResponse {
        let mut cr = ChatResponse::default();
        cr.append_text(text, Some(block_id));
        cr
    }

    #[test]
    fn test_text_streaming() {
        let mut acc = StreamAccumulator::new();
        acc.append_chat_response(&text_delta("Hel", "t1"));
        acc.append_chat_response(&text_delta("lo", "t1"));
        let result = acc.build();
        assert!(result.is_last);
        assert_eq!(result.get_text_content(""), "Hello");
    }

    #[test]
    fn test_thinking_streaming_with_extras() {
        let mut acc = StreamAccumulator::new();
        let mut d1 = ChatResponse::default();
        d1.append_thinking("step 1", Some("th1"), HashMap::new());
        let mut d2 = ChatResponse::default();
        let mut extras = HashMap::new();
        extras.insert("sig".to_string(), serde_json::json!("abc"));
        d2.append_thinking(" step 2", Some("th1"), extras);
        acc.append_chat_response(&d1);
        acc.append_chat_response(&d2);
        let result = acc.build();
        if let ContentBlock::Thinking(tb) = &result.content[0] {
            assert_eq!(tb.thinking, "step 1 step 2");
            assert_eq!(tb.extras.get("sig").unwrap(), &serde_json::json!("abc"));
        } else {
            panic!("Expected ThinkingBlock");
        }
    }

    #[test]
    fn test_tool_call_accumulation() {
        let mut acc = StreamAccumulator::new();
        let mut d1 = ChatResponse::default();
        d1.append_tool_call("tc1", "search", r#"{"q":"#, HashMap::new());
        let mut d2 = ChatResponse::default();
        d2.append_tool_call("tc1", "search", r#""test"}"#, HashMap::new());
        acc.append_chat_response(&d1);
        acc.append_chat_response(&d2);
        let result = acc.build();
        if let ContentBlock::ToolCall(tc) = &result.content[0] {
            assert_eq!(tc.input, r#"{"q":"test"}"#);
        } else {
            panic!("Expected ToolCallBlock");
        }
    }

    #[test]
    fn test_usage_and_id_propagation() {
        let mut acc = StreamAccumulator::new();
        acc.append_chat_response(&ChatResponse::default());
        let with = ChatResponse {
            id: "resp-1".to_string(),
            usage: Some(ChatUsage {
                input_tokens: 10,
                output_tokens: 5,
                ..Default::default()
            }),
            ..Default::default()
        };
        acc.append_chat_response(&with);
        let result = acc.build();
        assert_eq!(result.id, "resp-1");
        assert_eq!(result.usage.unwrap().input_tokens, 10);
    }
}
