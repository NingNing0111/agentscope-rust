//! AppendEvent trait — apply streaming events to Msg for incremental construction.

use agent_scope_message::{
    BlockType, ContentBlock, DataBlock, DataSource, HintBlock, Msg, TextBlock, ThinkingBlock,
    ToolCallBlock, ToolCallState, ToolOutput,
};
use agent_scope_types::ReplyFinishedReason;

use crate::AgentEvent;

/// Error when applying an event to a Msg.
#[derive(Debug, Clone)]
pub enum AppendEventError {
    ReplyIdMismatch {
        event_reply_id: String,
        msg_id: String,
    },
    BlockNotFound {
        block_type: BlockType,
        block_id: String,
    },
    UnknownEventType(String),
}

/// Trait for applying streaming events to a Msg.
pub trait AppendEvent {
    fn append_event(&mut self, event: &AgentEvent) -> Result<(), AppendEventError>;

    // Helper methods
    fn get_block_index(&self, block_id: &str, bt: BlockType) -> Result<usize, AppendEventError>;
}

fn find_block(
    content: &[ContentBlock],
    block_id: &str,
    bt: BlockType,
) -> Result<usize, AppendEventError> {
    content
        .iter()
        .position(|b| {
            let id = match b {
                ContentBlock::Text(tb) => &tb.id,
                ContentBlock::Thinking(tb) => &tb.id,
                ContentBlock::Hint(hb) => &hb.id,
                ContentBlock::Data(db) => &db.id,
                ContentBlock::ToolCall(tc) => &tc.id,
                ContentBlock::ToolResult(tr) => &tr.id,
                ContentBlock::Unknown => return false,
            };
            b.block_type() == bt && id == block_id
        })
        .ok_or_else(|| AppendEventError::BlockNotFound {
            block_type: bt,
            block_id: block_id.to_string(),
        })
}

fn event_reply_id(event: &AgentEvent) -> Option<&str> {
    match event {
        AgentEvent::ReplyStart(e) => Some(&e.reply_id),
        AgentEvent::ReplyEnd(e) => Some(&e.reply_id),
        AgentEvent::ModelCallStart(e) => Some(&e.reply_id),
        AgentEvent::ModelCallEnd(e) => Some(&e.reply_id),
        AgentEvent::TextBlockStart(e) => Some(&e.reply_id),
        AgentEvent::TextBlockDelta(e) => Some(&e.reply_id),
        AgentEvent::TextBlockEnd(e) => Some(&e.reply_id),
        AgentEvent::DataBlockStart(e) => Some(&e.reply_id),
        AgentEvent::DataBlockDelta(e) => Some(&e.reply_id),
        AgentEvent::DataBlockEnd(e) => Some(&e.reply_id),
        AgentEvent::ThinkingBlockStart(e) => Some(&e.reply_id),
        AgentEvent::ThinkingBlockDelta(e) => Some(&e.reply_id),
        AgentEvent::ThinkingBlockEnd(e) => Some(&e.reply_id),
        AgentEvent::HintBlock(e) => Some(&e.reply_id),
        AgentEvent::ToolCallStart(e) => Some(&e.reply_id),
        AgentEvent::ToolCallDelta(e) => Some(&e.reply_id),
        AgentEvent::ToolCallEnd(e) => Some(&e.reply_id),
        AgentEvent::ToolResultStart(e) => Some(&e.reply_id),
        AgentEvent::ToolResultTextDelta(e) => Some(&e.reply_id),
        AgentEvent::ToolResultDataDelta(e) => Some(&e.reply_id),
        AgentEvent::ToolResultEnd(e) => Some(&e.reply_id),
        AgentEvent::ExceedMaxIters(e) => Some(&e.reply_id),
        AgentEvent::RequireUserConfirm(e) => Some(&e.reply_id),
        AgentEvent::UserConfirmResult(e) => Some(&e.reply_id),
        AgentEvent::UserInterrupt(e) => Some(&e.reply_id),
        AgentEvent::RequireExternalExecution(e) => Some(&e.reply_id),
        AgentEvent::ExternalExecutionResult(e) => Some(&e.reply_id),
        AgentEvent::Custom(_)
        | AgentEvent::SessionCreated(_)
        | AgentEvent::SessionClosed(_)
        | AgentEvent::SessionSaved(_)
        | AgentEvent::SessionLoaded(_)
        | AgentEvent::SessionTrimmed(_) => None,
    }
}

impl AppendEvent for Msg {
    fn get_block_index(&self, block_id: &str, bt: BlockType) -> Result<usize, AppendEventError> {
        find_block(&self.content, block_id, bt)
    }

    fn append_event(&mut self, event: &AgentEvent) -> Result<(), AppendEventError> {
        use agent_scope_message::Base64Source;

        if let Some(reply_id) = event_reply_id(event)
            && reply_id != self.id
        {
            return Err(AppendEventError::ReplyIdMismatch {
                event_reply_id: reply_id.to_string(),
                msg_id: self.id.clone(),
            });
        }

        match event {
            AgentEvent::ReplyStart(_e) => {}

            AgentEvent::ReplyEnd(e) => {
                self.finished_reason = Some(e.finished_reason.clone());
                self.finished_at = Some(e.base.created_at.clone());
                if let Some(ref error) = e.error {
                    self.error = Some(error.clone());
                }
            }

            AgentEvent::ModelCallStart(_e) => {}

            AgentEvent::ModelCallEnd(e) => {
                let usage = self.usage.get_or_insert(agent_scope_message::Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                });
                usage.input_tokens += e.input_tokens;
                usage.output_tokens += e.output_tokens;
            }

            AgentEvent::TextBlockStart(e) => {
                let mut block = TextBlock::new(String::new());
                block.id = e.block_id.clone();
                // Use the event timestamp so a persisted/replayed stream keeps
                // the original timeline (created_at must not post-date the
                // finished_at recorded from the event).
                block.created_at = e.base.created_at.clone();
                self.content.push(ContentBlock::Text(block));
            }

            AgentEvent::TextBlockDelta(e) => {
                let idx = find_block(&self.content, &e.block_id, BlockType::Text)?;
                if let ContentBlock::Text(ref mut tb) = self.content[idx] {
                    tb.text.push_str(&e.delta);
                }
            }

            AgentEvent::TextBlockEnd(e) => {
                let idx = find_block(&self.content, &e.block_id, BlockType::Text)?;
                if let ContentBlock::Text(ref mut tb) = self.content[idx] {
                    if let Some(ref text) = e.text {
                        tb.text = text.clone();
                    }
                    tb.finished_at = Some(e.base.created_at.clone());
                }
            }

            AgentEvent::DataBlockStart(e) => {
                let source = DataSource::Base64(Base64Source {
                    data: String::new(),
                    media_type: e.media_type.clone(),
                });
                let mut db = DataBlock::new(source);
                db.id = e.block_id.clone();
                db.created_at = e.base.created_at.clone();
                self.content.push(ContentBlock::Data(db));
            }

            AgentEvent::DataBlockDelta(e) => {
                let idx = find_block(&self.content, &e.block_id, BlockType::Data)?;
                if let ContentBlock::Data(ref mut db) = self.content[idx]
                    && let DataSource::Base64(ref mut bs) = db.source
                {
                    // Streaming audio/image deltas are fragments of a single
                    // base64 stream and may arrive unaligned to the 4-byte
                    // boundary, so decoding each fragment independently fails.
                    // The previous `unwrap_or_default()` silently dropped those
                    // bytes, corrupting the binary payload. Concatenate the
                    // fragments instead; the block is fully encoded once
                    // `DataBlockEnd` arrives.
                    bs.data.push_str(&e.data);
                }
            }

            AgentEvent::DataBlockEnd(e) => {
                let idx = find_block(&self.content, &e.block_id, BlockType::Data)?;
                if let ContentBlock::Data(ref mut db) = self.content[idx] {
                    db.finished_at = Some(e.base.created_at.clone());
                }
            }

            AgentEvent::ThinkingBlockStart(e) => {
                let mut tb = ThinkingBlock::new(String::new());
                tb.id = e.block_id.clone();
                tb.created_at = e.base.created_at.clone();
                self.content.push(ContentBlock::Thinking(tb));
            }

            AgentEvent::ThinkingBlockDelta(e) => {
                let idx = find_block(&self.content, &e.block_id, BlockType::Thinking)?;
                if let ContentBlock::Thinking(ref mut tb) = self.content[idx] {
                    tb.thinking.push_str(&e.delta);
                }
            }

            AgentEvent::ThinkingBlockEnd(e) => {
                let idx = find_block(&self.content, &e.block_id, BlockType::Thinking)?;
                if let ContentBlock::Thinking(ref mut tb) = self.content[idx] {
                    if let Some(ref thinking) = e.thinking {
                        tb.thinking = thinking.clone();
                    }
                    tb.finished_at = Some(e.base.created_at.clone());
                }
            }

            AgentEvent::HintBlock(e) => {
                let mut hb = HintBlock::new(e.hint.clone());
                hb.id = e.block_id.clone();
                hb.source = e.source.clone();
                hb.created_at = e.base.created_at.clone();
                self.content.push(ContentBlock::Hint(hb));
            }

            AgentEvent::ToolCallStart(e) => {
                let mut tc = ToolCallBlock::new(
                    e.tool_call_id.clone(),
                    e.tool_call_name.clone(),
                    String::new(),
                );
                tc.created_at = e.base.created_at.clone();
                self.content.push(ContentBlock::ToolCall(tc));
            }

            AgentEvent::ToolCallDelta(e) => {
                let idx = find_block(&self.content, &e.tool_call_id, BlockType::ToolCall)?;
                if let ContentBlock::ToolCall(ref mut tc) = self.content[idx] {
                    tc.input.push_str(&e.delta);
                }
            }

            AgentEvent::ToolCallEnd(e) => {
                let idx = find_block(&self.content, &e.tool_call_id, BlockType::ToolCall)?;
                if let ContentBlock::ToolCall(ref mut tc) = self.content[idx] {
                    if let Some(ref input) = e.input {
                        tc.input = input.clone();
                    }
                    tc.state = ToolCallState::Submitted;
                    tc.finished_at = Some(e.base.created_at.clone());
                }
            }

            AgentEvent::ToolResultStart(_e) => {
                // ToolResultBlock is created by the tool execution layer.
            }

            AgentEvent::ToolResultTextDelta(e) => {
                let idx = find_block(&self.content, &e.tool_call_id, BlockType::ToolResult)?;
                if let ContentBlock::ToolResult(ref mut tr) = self.content[idx]
                    && let ToolOutput::Text(ref mut t) = tr.output
                {
                    t.push_str(&e.delta);
                }
            }

            AgentEvent::ToolResultDataDelta(_e) => {
                // Data deltas for tool results handled by tool execution layer
            }

            AgentEvent::ToolResultEnd(e) => {
                let idx = find_block(&self.content, &e.tool_call_id, BlockType::ToolResult)?;
                if let ContentBlock::ToolResult(ref mut tr) = self.content[idx] {
                    if let Some(ref output) = e.output {
                        tr.output = ToolOutput::Text(output.clone());
                    }
                    tr.state = e.state.clone();
                    tr.metadata = e.metadata.clone();
                    tr.is_last = true;
                    tr.finished_at = Some(e.base.created_at.clone());
                }
            }

            AgentEvent::ExceedMaxIters(_e) => {
                self.finished_reason = Some(ReplyFinishedReason::ExceedMaxIters);
            }

            AgentEvent::UserInterrupt(_e) => {
                self.finished_reason = Some(ReplyFinishedReason::Interrupted);
            }

            // Interaction events are handled by Agent loop layer
            AgentEvent::RequireUserConfirm(_)
            | AgentEvent::UserConfirmResult(_)
            | AgentEvent::RequireExternalExecution(_)
            | AgentEvent::ExternalExecutionResult(_)
            | AgentEvent::Custom(_)
            | AgentEvent::SessionCreated(_)
            | AgentEvent::SessionClosed(_)
            | AgentEvent::SessionSaved(_)
            | AgentEvent::SessionLoaded(_)
            | AgentEvent::SessionTrimmed(_) => {}
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::EventBase;
    use crate::{
        ModelCallEndEvent, ReplyEndEvent, TextBlockDeltaEvent, TextBlockEndEvent,
        TextBlockStartEvent, ToolCallDeltaEvent, ToolCallEndEvent, ToolCallStartEvent,
        UserInterruptEvent,
    };
    use agent_scope_message::{BlockType, ContentBlock, Msg, Role, ToolCallState};
    use agent_scope_types::ReplyFinishedReason;

    fn make_base() -> EventBase {
        EventBase::new()
    }

    #[test]
    fn test_append_text_streaming_sequence() {
        let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
        msg.id = "reply-001".into();
        let base = make_base();

        msg.append_event(&AgentEvent::TextBlockStart(TextBlockStartEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
        }))
        .unwrap();

        msg.append_event(&AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            delta: "Hel".into(),
        }))
        .unwrap();

        msg.append_event(&AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            delta: "lo".into(),
        }))
        .unwrap();

        msg.append_event(&AgentEvent::TextBlockEnd(TextBlockEndEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "block-001".into(),
            text: None,
        }))
        .unwrap();

        assert_eq!(msg.get_text_content(" ").unwrap(), "Hello");
        assert!(msg.has_content_blocks(Some(BlockType::Text)));
    }

    #[test]
    fn test_append_tool_call_lifecycle() {
        let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
        msg.id = "reply-001".into();
        let base = make_base();

        msg.append_event(&AgentEvent::ToolCallStart(ToolCallStartEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-001".into(),
            tool_call_name: "search".into(),
        }))
        .unwrap();

        msg.append_event(&AgentEvent::ToolCallDelta(ToolCallDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-001".into(),
            delta: r#"{"q":""#.into(),
        }))
        .unwrap();

        msg.append_event(&AgentEvent::ToolCallDelta(ToolCallDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-001".into(),
            delta: r#"test"}"#.into(),
        }))
        .unwrap();

        msg.append_event(&AgentEvent::ToolCallEnd(ToolCallEndEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            tool_call_id: "tc-001".into(),
            input: None,
        }))
        .unwrap();

        if let ContentBlock::ToolCall(ref tc) = msg.content[0] {
            assert_eq!(tc.input, r#"{"q":"test"}"#);
            assert_eq!(tc.state, ToolCallState::Submitted);
        } else {
            panic!("expected ToolCall block");
        }
    }

    #[test]
    fn test_append_user_interrupt() {
        let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
        msg.id = "reply-001".into();
        msg.append_event(&AgentEvent::UserInterrupt(UserInterruptEvent {
            base: make_base(),
            reply_id: "reply-001".into(),
        }))
        .unwrap();
        assert!(matches!(
            msg.finished_reason,
            Some(ReplyFinishedReason::Interrupted)
        ));
    }

    #[test]
    fn test_append_model_call_end_accumulates_tokens() {
        let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
        msg.id = "reply-001".into();
        msg.append_event(&AgentEvent::ModelCallEnd(ModelCallEndEvent {
            base: make_base(),
            reply_id: "reply-001".into(),
            input_tokens: 100,
            output_tokens: 50,
            finished_reason: ReplyFinishedReason::Completed,
        }))
        .unwrap();

        assert_eq!(msg.usage.as_ref().unwrap().input_tokens, 100);
        assert_eq!(msg.usage.as_ref().unwrap().output_tokens, 50);

        msg.append_event(&AgentEvent::ModelCallEnd(ModelCallEndEvent {
            base: make_base(),
            reply_id: "reply-001".into(),
            input_tokens: 50,
            output_tokens: 25,
            finished_reason: ReplyFinishedReason::Completed,
        }))
        .unwrap();

        assert_eq!(msg.usage.as_ref().unwrap().input_tokens, 150);
        assert_eq!(msg.usage.as_ref().unwrap().output_tokens, 75);
    }

    #[test]
    fn test_append_reply_end_sets_finished_reason() {
        let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
        msg.id = "reply-001".into();
        msg.append_event(&AgentEvent::ReplyEnd(ReplyEndEvent {
            base: make_base(),
            session_id: "s-1".into(),
            reply_id: "reply-001".into(),
            finished_reason: ReplyFinishedReason::Completed,
            error: None,
        }))
        .unwrap();
        assert!(matches!(
            msg.finished_reason,
            Some(ReplyFinishedReason::Completed)
        ));
    }

    #[test]
    fn test_append_data_block_delta_concatenates_base64_fragments() {
        // E2: streaming base64 fragments may arrive unaligned to the 4-byte
        // boundary; they must be concatenated, not per-fragment-decoded.
        use crate::{DataBlockDeltaEvent, DataBlockStartEvent};
        use agent_scope_message::DataSource;

        let mut msg = Msg::new("agent".into(), vec![], Role::Assistant).unwrap();
        msg.id = "reply-001".into();
        let base = make_base();

        msg.append_event(&AgentEvent::DataBlockStart(DataBlockStartEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "d-1".into(),
            media_type: "image/png".into(),
        }))
        .unwrap();

        // base64 of "AB" split at an unaligned boundary: "QU" (2 chars, not
        // 4-aligned) followed by "I=" (decodes only together as "QUI=").
        let delta1 = "QU";
        let delta2 = "I=";
        msg.append_event(&AgentEvent::DataBlockDelta(DataBlockDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "d-1".into(),
            data: delta1.into(),
            media_type: "image/png".into(),
        }))
        .unwrap();
        msg.append_event(&AgentEvent::DataBlockDelta(DataBlockDeltaEvent {
            base: base.clone(),
            reply_id: "reply-001".into(),
            block_id: "d-1".into(),
            data: delta2.into(),
            media_type: "image/png".into(),
        }))
        .unwrap();

        let ContentBlock::Data(db) = &msg.content[0] else {
            panic!("expected a Data block");
        };
        let DataSource::Base64(bs) = &db.source else {
            panic!("expected base64 source");
        };
        // The fragments must be preserved verbatim (concatenated).
        assert_eq!(bs.data, "QUI=");
        // And the concatenated base64 decodes to the original bytes.
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&bs.data)
            .unwrap();
        assert_eq!(decoded, b"AB");
    }
}
