//! Streaming reactor — progressive model stream processing (Feature 008 US1/US2/US3).
//! Events are emitted to the caller in real-time as the model produces chunks.
//!
//! US2: Tool calls detected progressively via block-type transition heuristic.
//! When a chunk transitions from ToolCall blocks → non-ToolCall blocks (or stream ends),
//! tool calls are considered complete (ToolCallEnd emitted). Execution happens after the
//! full model stream is consumed to avoid dropping remaining model chunks (P0-3 fix).
//!
//! Key protocol rules:
//! - TextBlock: one Start → N Deltas → one End per block_id (P1-9 fix)
//! - ToolCallBlock: one Start → N Deltas → one End, executed at stream/iteration end
//! - All events within a single ReAct iteration come from one continuous model stream

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use agent_scope_event::{
    AgentEvent, EventBase, ExceedMaxItersEvent, ModelCallEndEvent, ModelCallStartEvent,
    ReplyEndEvent, RequireUserConfirmEvent, TextBlockDeltaEvent, TextBlockEndEvent,
    TextBlockStartEvent, ThinkingBlockDeltaEvent, ThinkingBlockEndEvent, ThinkingBlockStartEvent,
    ToolCallDeltaEvent, ToolCallEndEvent, ToolCallStartEvent, ToolResultEndEvent,
    ToolResultStartEvent, ToolResultTextDeltaEvent, UserInterruptEvent,
};
use agent_scope_message::{
    ContentBlock, Msg, Role, TextBlock, ThinkingBlock, ToolCallBlock, ToolCallState, ToolOutput,
    ToolResultBlock, ToolResultState,
};
use agent_scope_model::{ChatResponse, ChatUsage, ModelCallResult, StreamAccumulator};
use agent_scope_tool::{ToolError, ToolExecOutput};
use agent_scope_types::{ErrorInfo, ErrorType, ReplyFinishedReason};
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::agent_error::AgentError;
use crate::permission::{PermissionBehavior, PermissionEngine};
use crate::react_agent::AgentInner;
use crate::stream_handle::StreamHandle;
use crate::tool_feedback::tool_error_feedback;

pub(crate) async fn run_streaming_loop(
    inner: Arc<AgentInner>,
    session_id: String,
    reply_id: String,
    system_prompt: String,
    stream_handle: StreamHandle,
    event_tx: mpsc::Sender<AgentEvent>,
    cancel_token: CancellationToken,
) {
    let base = EventBase::new;
    let mut cur_iter: u32 = 0;

    if stream_handle.is_cancelled() {
        return;
    }
    emit_start(&event_tx, &session_id, &reply_id, &inner.config.name, base).await;

    loop {
        // Consume the interrupted flag so it only affects the current reply:
        // the next `reply_stream()` runs normally with a fresh token (matches
        // `interrupt()`'s documented contract).
        let was_interrupted = inner.interrupted.swap(false, Ordering::SeqCst);
        if stream_handle.is_cancelled() || was_interrupted {
            emit_interrupted(&event_tx, &reply_id, &session_id, base).await;
            let result = Ok(Msg::new(
                inner.config.name.clone(),
                vec![ContentBlock::Text(TextBlock::new(
                    inner.react_config.interruption_message.clone(),
                ))],
                Role::Assistant,
            )
            .unwrap_or_else(|_| {
                Msg::new(
                    inner.config.name.clone(),
                    vec![ContentBlock::Text(TextBlock::new("Interrupted".into()))],
                    Role::Assistant,
                )
                .unwrap()
            }));
            invoke_post_reply(&inner, &result).await;
            return;
        }
        if cur_iter >= inner.react_config.max_iters {
            emit_max_iters(&event_tx, &reply_id, &session_id, &inner.config.name, base).await;
            let result = {
                let text = collect_context_text(&inner);
                Ok(Msg::new(
                    inner.config.name.clone(),
                    vec![ContentBlock::Text(TextBlock::new(text))],
                    Role::Assistant,
                )
                .unwrap_or_else(|_| {
                    Msg::new(
                        inner.config.name.clone(),
                        vec![ContentBlock::Text(TextBlock::new("".into()))],
                        Role::Assistant,
                    )
                    .unwrap()
                }))
            };
            invoke_post_reply(&inner, &result).await;
            return;
        }
        cur_iter += 1;

        // Prep messages + hooks
        let messages = { inner.state.read().unwrap().context.clone() };
        let mut hook_messages = messages.clone();
        if !system_prompt.is_empty()
            && let Ok(system_msg) =
                agent_scope_message::factory::system_msg("system", &system_prompt)
        {
            hook_messages.insert(0, system_msg);
        }
        let mut hook_tools = inner
            .config
            .toolkit
            .as_ref()
            .map(|tk| tk.get_tool_schemas());
        for mw in inner.middlewares.iter() {
            if let Err(e) = mw
                .pre_reasoning(&inner.config.name, &mut hook_messages, &mut hook_tools)
                .await
            {
                emit_error_end(&event_tx, &reply_id, &session_id, &e.to_string(), base).await;
                let result = Err(AgentError::ValidationError {
                    message: e.to_string(),
                });
                invoke_post_reply(&inner, &result).await;
                return;
            }
        }
        let tool_schemas = inner
            .config
            .toolkit
            .as_ref()
            .map(|tk| tk.get_tool_schemas());

        // Token count computed once, shared between compression and the
        // first-iteration runtime-state injection context-length dimension.
        let token_count = inner
            .config
            .model
            .count_tokens(&hook_messages, tool_schemas.as_deref());
        let context_size = inner.config.model.context_size();

        // Compression
        if inner.context_config.enable {
            let trigger = (context_size as f64 * inner.context_config.trigger_ratio) as usize;
            if token_count > trigger
                && let Err(e) = crate::context_compression::compress_context(
                    &inner.config.model,
                    &inner.state,
                    &inner.context_config,
                    &session_id,
                )
                .await
            {
                emit_error_end(&event_tx, &reply_id, &session_id, &e.to_string(), base).await;
                let result = Err(AgentError::ContextCompressionFailed {
                    reason: e.to_string(),
                });
                invoke_post_reply(&inner, &result).await;
                return;
            }
        }

        // Runtime-state injection (Feature 026) — evaluated each iteration so a
        // compression that removed the task-tool / time traces re-triggers the
        // relevant dimensions. Aligns with Python `_inject_runtime_state`.
        let injection_event = crate::runtime_injection::maybe_inject_runtime_state(
            &inner.state,
            &inner.config.name,
            &inner.config.injection_config,
            chrono::Utc::now().fixed_offset(),
            cur_iter,
            // The context-length dimension is only evaluated on the first
            // iteration, where the token count is meaningful.
            (cur_iter == 1).then_some(token_count),
            context_size,
            inner.context_config.trigger_ratio,
            inner.config.task_tools_enabled,
        );
        if let Some(evt) = injection_event {
            let _ = event_tx.send(AgentEvent::HintBlock(evt)).await;
        }

        // ModelCallStart
        let _ = event_tx
            .send(AgentEvent::ModelCallStart(ModelCallStartEvent {
                base: base(),
                reply_id: reply_id.clone(),
                model_name: inner.config.model.model_name().into(),
            }))
            .await;

        let call_future = inner
            .config
            .model
            .call(&hook_messages, tool_schemas.as_deref(), None);
        let result = tokio::select! {
            r = call_future => r,
            _ = cancel_token.cancelled() => {
                // Consume the flag so the interruption only affects the current
                // reply (see the top-of-loop swap above).
                inner.interrupted.store(false, Ordering::SeqCst);
                emit_interrupted(&event_tx, &reply_id, &session_id, base).await;
                let result = Ok(Msg::new(
                    inner.config.name.clone(),
                    vec![ContentBlock::Text(TextBlock::new(
                        inner.react_config.interruption_message.clone(),
                    ))],
                    Role::Assistant,
                ).unwrap_or_else(|_| {
                    Msg::new(
                        inner.config.name.clone(),
                        vec![ContentBlock::Text(TextBlock::new("Interrupted".into()))],
                        Role::Assistant,
                    ).unwrap()
                }));
                invoke_post_reply(&inner, &result).await;
                return;
            }
        };

        match result {
            Ok(ModelCallResult::Complete(response)) => {
                // Non-streaming path: emit text events → ModelCallEnd → tool events
                // FR-003: Text content blocks go between ModelCallStart/End,
                // tool call events go between ModelCallEnd and ReplyEnd.
                emit_text_events_only(&response, &event_tx, &reply_id, base).await;
                emit_model_call_end_with_usage(&event_tx, &reply_id, &response.usage, base).await;
                for mw in inner.middlewares.iter() {
                    let _ = mw.post_reasoning(&inner.config.name, &response).await;
                }
                // Process the response: write text to context or execute tool calls
                if process_response_and_continue(&inner, &response, &event_tx, &reply_id, base)
                    .await
                    .is_done()
                {
                    let result = {
                        let text = collect_context_text(&inner);
                        Ok(Msg::new(
                            inner.config.name.clone(),
                            vec![ContentBlock::Text(TextBlock::new(text))],
                            Role::Assistant,
                        )
                        .unwrap_or_else(|_| {
                            Msg::new(
                                inner.config.name.clone(),
                                vec![ContentBlock::Text(TextBlock::new("".into()))],
                                Role::Assistant,
                            )
                            .unwrap()
                        }))
                    };
                    invoke_post_reply(&inner, &result).await;
                    emit_completed_reply_end(&event_tx, &session_id, &reply_id, base).await;
                    return;
                }
                // Tool calls were executed — continue loop
            }
            Ok(ModelCallResult::Stream(mut stream)) => {
                // Streaming path: consume ALL chunks progressively
                let outcome = consume_stream_progressive(
                    &mut stream,
                    &stream_handle,
                    &event_tx,
                    &reply_id,
                    &session_id,
                    &cancel_token,
                    base,
                )
                .await;

                match outcome {
                    StreamOutcome::Normal {
                        response,
                        usage,
                        tool_calls,
                    } => {
                        // Emit ModelCallEnd if we haven't already (is_last-based)
                        emit_model_call_end_with_usage(&event_tx, &reply_id, &usage, base).await;
                        for mw in inner.middlewares.iter() {
                            let _ = mw.post_reasoning(&inner.config.name, &response).await;
                        }

                        if !tool_calls.is_empty() {
                            // Write text blocks from the response to context
                            add_text_to_context(&inner, &response);
                            // Store assistant tool_call message BEFORE tool results
                            add_tool_calls_to_context(&inner, &tool_calls);
                            // Execute all detected tool calls
                            execute_tool_calls(
                                &inner,
                                &tool_calls,
                                &event_tx,
                                &reply_id,
                                &stream_handle,
                                base,
                            )
                            .await;
                            // Continue ReAct loop — model will be called again
                        } else {
                            // No tool calls — write text to context and end
                            add_text_to_context(&inner, &response);
                            let result = {
                                let text = collect_context_text(&inner);
                                Ok(Msg::new(
                                    inner.config.name.clone(),
                                    vec![ContentBlock::Text(TextBlock::new(text))],
                                    Role::Assistant,
                                )
                                .unwrap_or_else(|_| {
                                    Msg::new(
                                        inner.config.name.clone(),
                                        vec![ContentBlock::Text(TextBlock::new("".into()))],
                                        Role::Assistant,
                                    )
                                    .unwrap()
                                }))
                            };
                            invoke_post_reply(&inner, &result).await;
                            emit_completed_reply_end(&event_tx, &session_id, &reply_id, base).await;
                            return;
                        }
                    }
                    StreamOutcome::Error { message } => {
                        emit_error_end(&event_tx, &reply_id, &session_id, &message, base).await;
                        let result: Result<Msg, AgentError> = Err(AgentError::ModelError {
                            source: agent_scope_model::ModelError::ApiError {
                                status: 0,
                                message,
                                provider: inner.config.model.model_name().into(),
                            },
                        });
                        invoke_post_reply(&inner, &result).await;
                        return;
                    }
                    StreamOutcome::Cancelled => {
                        // Consume the flag so an interruption only affects the
                        // current reply (see the top-of-loop swap above).
                        inner.interrupted.store(false, Ordering::SeqCst);
                        let result: Result<Msg, AgentError> = Err(AgentError::CancellationError {
                            reply_id: reply_id.clone(),
                        });
                        invoke_post_reply(&inner, &result).await;
                        return;
                    }
                }
            }
            Err(e) => {
                emit_error_end(&event_tx, &reply_id, &session_id, &e.to_string(), base).await;
                let result: Result<Msg, AgentError> = Err(AgentError::ModelError { source: e });
                invoke_post_reply(&inner, &result).await;
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Stream consumption outcomes
// ---------------------------------------------------------------------------

enum StreamOutcome {
    /// Full stream consumed — response, usage data, and any detected tool calls.
    Normal {
        response: Box<ChatResponse>,
        /// Usage from the final chunk (or None if stream ended without one).
        usage: Option<ChatUsage>,
        /// Tool calls detected during stream (complete at stream end or
        /// at block-type transition).
        tool_calls: Vec<ToolCallBlock>,
    },
    /// Chunk-level error from the model stream.
    Error { message: String },
    /// Stream was cancelled (consumer dropped EventStream).
    Cancelled,
}

// ---------------------------------------------------------------------------
// Progressive stream consumption (US1 + US2)
// ---------------------------------------------------------------------------

/// Active block tracking for progressive event emission.
struct BlockTracker {
    /// Text blocks whose lifecycle hasn't ended yet: block_id → (emitted_start, text)
    text_blocks: HashMap<String, (bool, Vec<String>)>,
    /// Thinking blocks being accumulated: block_id → (emitted_start, texts)
    thinking_blocks: HashMap<String, (bool, Vec<String>)>,
    /// Tool call blocks being accumulated: block_id → ToolCallBlock
    tool_blocks: HashMap<String, ToolCallBlock>,
    /// Tool calls finalized (complete) with their block IDs for ordering
    completed_tool_ids: Vec<String>,
}

impl BlockTracker {
    fn new() -> Self {
        Self {
            text_blocks: HashMap::new(),
            thinking_blocks: HashMap::new(),
            tool_blocks: HashMap::new(),
            completed_tool_ids: Vec::new(),
        }
    }
}

async fn consume_stream_progressive(
    stream: &mut (
             impl futures::Stream<Item = Result<ChatResponse, agent_scope_model::ModelError>>
             + std::marker::Unpin
         ),
    stream_handle: &StreamHandle,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    session_id: &str,
    cancel_token: &CancellationToken,
    base: fn() -> EventBase,
) -> StreamOutcome {
    let mut acc = StreamAccumulator::new();
    let mut tracker = BlockTracker::new();
    let mut final_usage: Option<ChatUsage> = None;

    loop {
        // Check for stream_handle cancellation before each poll.
        if stream_handle.is_cancelled() {
            let _ = event_tx
                .send(AgentEvent::ReplyEnd(ReplyEndEvent {
                    base: base(),
                    session_id: session_id.into(),
                    reply_id: reply_id.into(),
                    finished_reason: ReplyFinishedReason::Interrupted,
                    error: None,
                }))
                .await;
            return StreamOutcome::Cancelled;
        }

        // Use select! to allow CancellationToken-based interruption during stream poll.
        let chunk_result = tokio::select! {
            r = stream.next() => r,
            _ = cancel_token.cancelled() => {
                close_active_tool_blocks(&mut tracker, event_tx, reply_id, base).await;
                close_all_text_blocks(&mut tracker, event_tx, reply_id, base).await;
                close_all_thinking_blocks(&mut tracker, event_tx, reply_id, base).await;
                let _ = event_tx
                    .send(AgentEvent::ReplyEnd(ReplyEndEvent {
                        base: base(),
                        session_id: session_id.into(),
                        reply_id: reply_id.into(),
                        finished_reason: ReplyFinishedReason::Interrupted,
                        error: None,
                    }))
                    .await;
                return StreamOutcome::Cancelled;
            }
        };

        match chunk_result {
            Some(Ok(chunk)) => {
                let is_last = chunk.is_last;
                let usage = chunk.usage.clone();

                // Process each content block in the chunk
                for block in &chunk.content {
                    match block {
                        ContentBlock::ToolCall(tc) => {
                            // Defense-in-depth (P1-12): if this tool call was already
                            // finalized (ToolCallEnd emitted), silently drop any further
                            // chunks for it. Without this guard, late-arriving deltas
                            // would appear after ToolCallEnd.
                            if tracker.completed_tool_ids.contains(&tc.id) {
                                continue;
                            }

                            // Check if this block was previously a text block or
                            // thinking block (block-type transition: close their lifecycles)
                            let had_text = !tracker.text_blocks.is_empty();
                            let had_thinking = !tracker.thinking_blocks.is_empty();

                            if let Some(existing) = tracker.tool_blocks.get_mut(&tc.id) {
                                // Subsequent chunk for same tool call — emit delta
                                let _ = event_tx
                                    .send(AgentEvent::ToolCallDelta(ToolCallDeltaEvent {
                                        base: base(),
                                        reply_id: reply_id.into(),
                                        tool_call_id: tc.id.clone(),
                                        delta: tc.input.clone(),
                                    }))
                                    .await;
                                existing.input.push_str(&tc.input);
                                if !tc.name.is_empty() && existing.name.is_empty() {
                                    existing.name = tc.name.clone();
                                }
                            } else {
                                // First chunk for this tool call — emit start.
                                // Close thinking and text blocks BEFORE ToolCallStart
                                // so the event sequence reads:
                                //   ThinkingBlockEnd → ToolCallStart → ToolCallDelta...
                                // instead of:
                                //   ToolCallStart → ToolCallEnd → ThinkingBlockEnd
                                if had_thinking {
                                    close_all_thinking_blocks(
                                        &mut tracker,
                                        event_tx,
                                        reply_id,
                                        base,
                                    )
                                    .await;
                                }
                                if had_text {
                                    close_all_text_blocks(&mut tracker, event_tx, reply_id, base)
                                        .await;
                                }

                                let _ = event_tx
                                    .send(AgentEvent::ToolCallStart(ToolCallStartEvent {
                                        base: base(),
                                        reply_id: reply_id.into(),
                                        tool_call_id: tc.id.clone(),
                                        tool_call_name: tc.name.clone(),
                                    }))
                                    .await;
                                // Emit delta for the first chunk's input (P0-11 fix:
                                // first-chunk args were previously dropped from display)
                                if !tc.input.is_empty() {
                                    let _ = event_tx
                                        .send(AgentEvent::ToolCallDelta(ToolCallDeltaEvent {
                                            base: base(),
                                            reply_id: reply_id.into(),
                                            tool_call_id: tc.id.clone(),
                                            delta: tc.input.clone(),
                                        }))
                                        .await;
                                }
                                tracker.tool_blocks.insert(tc.id.clone(), tc.clone());
                            }
                        }
                        ContentBlock::Text(tb) => {
                            // Check if this signals a tool→text or thinking→text transition:
                            // close active tool and thinking blocks first so their End events
                            // appear before TextBlockStart.
                            close_active_tool_blocks(&mut tracker, event_tx, reply_id, base).await;
                            if !tracker.thinking_blocks.is_empty() {
                                close_all_thinking_blocks(&mut tracker, event_tx, reply_id, base)
                                    .await;
                            }

                            process_text_block_chunk(&mut tracker, tb, event_tx, reply_id, base)
                                .await;
                        }
                        ContentBlock::Thinking(thb) => {
                            // Close any active tool blocks first — if the model goes
                            // tool_call → thinking (common in DashScope thinking mode),
                            // ToolCallEnd must appear before ThinkingBlockDelta.
                            if !tracker.tool_blocks.is_empty() {
                                close_active_tool_blocks(&mut tracker, event_tx, reply_id, base)
                                    .await;
                            }

                            process_thinking_block_chunk(
                                &mut tracker,
                                thb,
                                event_tx,
                                reply_id,
                                base,
                            )
                            .await;
                        }
                        _ => {
                            // Unknown block type → close all active blocks (transition)
                            close_active_tool_blocks(&mut tracker, event_tx, reply_id, base).await;
                            close_all_text_blocks(&mut tracker, event_tx, reply_id, base).await;
                            close_all_thinking_blocks(&mut tracker, event_tx, reply_id, base).await;
                        }
                    }
                }

                // On stream end (is_last or EOF): close all active blocks
                if is_last {
                    close_active_tool_blocks(&mut tracker, event_tx, reply_id, base).await;
                    close_all_text_blocks(&mut tracker, event_tx, reply_id, base).await;
                    close_all_thinking_blocks(&mut tracker, event_tx, reply_id, base).await;
                }

                // Track usage from the last chunk that carries it
                if usage.is_some() {
                    final_usage = usage;
                }

                acc.append_chat_response(&chunk);

                // If is_last, stop consuming (model signals stream complete)
                if is_last {
                    break;
                }
            }
            Some(Err(e)) => {
                // Close all tracking before returning error
                close_active_tool_blocks(&mut tracker, event_tx, reply_id, base).await;
                close_all_text_blocks(&mut tracker, event_tx, reply_id, base).await;
                close_all_thinking_blocks(&mut tracker, event_tx, reply_id, base).await;
                return StreamOutcome::Error {
                    message: e.to_string(),
                };
            }
            None => {
                // Stream ended naturally (EOF without is_last)
                break;
            }
        }
    }

    // Stream ended (EOF or is_last) — close any remaining open blocks
    close_active_tool_blocks(&mut tracker, event_tx, reply_id, base).await;
    close_all_text_blocks(&mut tracker, event_tx, reply_id, base).await;
    close_all_thinking_blocks(&mut tracker, event_tx, reply_id, base).await;

    // Collect finalized tool calls in order
    let tool_calls: Vec<ToolCallBlock> = tracker
        .completed_tool_ids
        .iter()
        .filter_map(|id| tracker.tool_blocks.remove(id))
        .collect();

    StreamOutcome::Normal {
        response: Box::new(acc.build()),
        usage: final_usage,
        tool_calls,
    }
}

// ---------------------------------------------------------------------------
// Block lifecycle helpers
// ---------------------------------------------------------------------------

/// Emit TextBlockEnd for all active text blocks, clearing them.
async fn close_all_text_blocks(
    tracker: &mut BlockTracker,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    base: fn() -> EventBase,
) {
    for block_id in tracker.text_blocks.keys().cloned().collect::<Vec<_>>() {
        let accumulated = tracker
            .text_blocks
            .remove(&block_id)
            .map(|(_, texts)| texts.concat())
            .filter(|s| !s.is_empty());
        let _ = event_tx
            .send(AgentEvent::TextBlockEnd(TextBlockEndEvent {
                base: base(),
                reply_id: reply_id.into(),
                block_id,
                text: accumulated,
            }))
            .await;
    }
    tracker.text_blocks.clear();
}

/// Finalize all active tool call blocks: emit ToolCallEnd and move to completed list.
/// Note: blocks are NOT removed from tracker.tool_blocks here — they are needed for
/// final collection in consume_stream_progressive(). Subsequent ToolCallDelta chunks
/// for completed blocks are guarded by a check in the ToolCall handler (P1-12).
async fn close_active_tool_blocks(
    tracker: &mut BlockTracker,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    base: fn() -> EventBase,
) {
    let ids: Vec<String> = tracker.tool_blocks.keys().cloned().collect();
    for id in ids {
        if !tracker.completed_tool_ids.contains(&id) {
            let input = tracker
                .tool_blocks
                .get(&id)
                .map(|tc| tc.input.clone())
                .filter(|s| !s.is_empty());
            let _ = event_tx
                .send(AgentEvent::ToolCallEnd(ToolCallEndEvent {
                    base: base(),
                    reply_id: reply_id.into(),
                    tool_call_id: id.clone(),
                    input,
                }))
                .await;
            tracker.completed_tool_ids.push(id);
        }
    }
}

/// Process a text block chunk with proper lifecycle:
/// Start on first occurrence → Delta on each chunk → End on close.
async fn process_text_block_chunk(
    tracker: &mut BlockTracker,
    tb: &TextBlock,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    base: fn() -> EventBase,
) {
    if tb.text.is_empty() {
        return;
    }
    let bid = tb.id.clone();
    match tracker.text_blocks.get_mut(&bid) {
        Some((already_started, texts)) => {
            // Subsequent chunk — emit only Delta
            let _ = event_tx
                .send(AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
                    base: base(),
                    reply_id: reply_id.into(),
                    block_id: bid,
                    delta: tb.text.clone(),
                }))
                .await;
            // Mark start emitted if it wasn't already
            *already_started = true;
            texts.push(tb.text.clone());
        }
        None => {
            // First chunk for this block_id — emit Start + Delta
            let _ = event_tx
                .send(AgentEvent::TextBlockStart(TextBlockStartEvent {
                    base: base(),
                    reply_id: reply_id.into(),
                    block_id: bid.clone(),
                }))
                .await;
            let _ = event_tx
                .send(AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
                    base: base(),
                    reply_id: reply_id.into(),
                    block_id: bid.clone(),
                    delta: tb.text.clone(),
                }))
                .await;
            tracker
                .text_blocks
                .insert(bid, (true, vec![tb.text.clone()]));
        }
    }
}

/// Emit ThinkingBlockEnd for all active thinking blocks, clearing them.
async fn close_all_thinking_blocks(
    tracker: &mut BlockTracker,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    base: fn() -> EventBase,
) {
    for block_id in tracker.thinking_blocks.keys().cloned().collect::<Vec<_>>() {
        let accumulated = tracker
            .thinking_blocks
            .remove(&block_id)
            .map(|(_, texts)| texts.concat())
            .filter(|s| !s.is_empty());
        let _ = event_tx
            .send(AgentEvent::ThinkingBlockEnd(ThinkingBlockEndEvent {
                base: base(),
                reply_id: reply_id.into(),
                block_id,
                thinking: accumulated,
            }))
            .await;
    }
    tracker.thinking_blocks.clear();
}

/// Process a thinking block chunk with proper lifecycle:
/// Start on first occurrence → Delta on each chunk → End on close.
async fn process_thinking_block_chunk(
    tracker: &mut BlockTracker,
    thb: &ThinkingBlock,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    base: fn() -> EventBase,
) {
    if thb.thinking.is_empty() {
        return;
    }
    let bid = thb.id.clone();
    match tracker.thinking_blocks.get_mut(&bid) {
        Some((already_started, texts)) => {
            // Subsequent chunk — emit only Delta
            let _ = event_tx
                .send(AgentEvent::ThinkingBlockDelta(ThinkingBlockDeltaEvent {
                    base: base(),
                    reply_id: reply_id.into(),
                    block_id: bid,
                    delta: thb.thinking.clone(),
                }))
                .await;
            *already_started = true;
            texts.push(thb.thinking.clone());
        }
        None => {
            // First chunk for this block_id — emit Start + Delta
            let _ = event_tx
                .send(AgentEvent::ThinkingBlockStart(ThinkingBlockStartEvent {
                    base: base(),
                    reply_id: reply_id.into(),
                    block_id: bid.clone(),
                }))
                .await;
            let _ = event_tx
                .send(AgentEvent::ThinkingBlockDelta(ThinkingBlockDeltaEvent {
                    base: base(),
                    reply_id: reply_id.into(),
                    block_id: bid.clone(),
                    delta: thb.thinking.clone(),
                }))
                .await;
            tracker
                .thinking_blocks
                .insert(bid, (true, vec![thb.thinking.clone()]));
        }
    }
}

// ---------------------------------------------------------------------------
// Response processing (Complete path)
// ---------------------------------------------------------------------------

/// Process the response from a Complete model call.
/// Returns Done if the reply should end (text-only response with no tool calls).
/// Returns Continue if tool calls were executed (loop should iterate).
async fn process_response_and_continue(
    inner: &Arc<AgentInner>,
    response: &ChatResponse,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    base: fn() -> EventBase,
) -> Outcome {
    // Check for tool calls
    let tool_calls: Vec<_> = response
        .content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::ToolCall(tc) = block {
                Some(tc.clone())
            } else {
                None
            }
        })
        .collect();

    if !tool_calls.is_empty() {
        // Emit tool call events + execute them
        add_text_to_context(inner, response);
        // Store assistant tool_call message BEFORE tool results.
        // OpenAI-compatible APIs require:
        //   assistant(tool_calls=[...])  →  tool(result, tool_call_id=...)
        add_tool_calls_to_context(inner, &tool_calls);
        let dummy_handle = StreamHandle::new_dummy();
        for tc in &tool_calls {
            let _ = event_tx
                .send(AgentEvent::ToolCallStart(ToolCallStartEvent {
                    base: base(),
                    reply_id: reply_id.into(),
                    tool_call_id: tc.id.clone(),
                    tool_call_name: tc.name.clone(),
                }))
                .await;
            let _ = event_tx
                .send(AgentEvent::ToolCallDelta(ToolCallDeltaEvent {
                    base: base(),
                    reply_id: reply_id.into(),
                    tool_call_id: tc.id.clone(),
                    delta: tc.input.clone(),
                }))
                .await;
            let _ = event_tx
                .send(AgentEvent::ToolCallEnd(ToolCallEndEvent {
                    base: base(),
                    reply_id: reply_id.into(),
                    tool_call_id: tc.id.clone(),
                    input: Some(tc.input.clone()),
                }))
                .await;
        }
        execute_tool_calls(inner, &tool_calls, event_tx, reply_id, &dummy_handle, base).await;
        return Outcome::Continue;
    }

    // Text-only response: write to context, return Done
    add_text_to_context(inner, response);
    Outcome::Done
}

// ---------------------------------------------------------------------------
// Tool execution
// ---------------------------------------------------------------------------

async fn execute_tool_calls(
    inner: &Arc<AgentInner>,
    tool_calls: &[ToolCallBlock],
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    stream_handle: &StreamHandle,
    _base: fn() -> EventBase,
) {
    for tc in tool_calls {
        let mut tc_mut = tc.clone();
        for mw in inner.middlewares.iter() {
            if let Err(e) = mw.pre_acting(&inner.config.name, &mut tc_mut).await {
                emit_denied_tool_result(
                    event_tx,
                    reply_id,
                    inner,
                    &tc_mut,
                    &format!("Permission denied by middleware: {e}"),
                    _base,
                )
                .await;
                continue;
            }
        }

        let permission_input = serde_json::from_str(&tc_mut.input)
            .unwrap_or_else(|_| serde_json::Value::String(tc_mut.input.clone()));
        let permission_engine =
            PermissionEngine::with_context(inner.config.permission_context.clone());
        let decision = permission_engine.check_decision(&tc_mut.name, &permission_input);
        match decision.behavior {
            PermissionBehavior::Deny => {
                emit_denied_tool_result(
                    event_tx,
                    reply_id,
                    inner,
                    &tc_mut,
                    &decision.message,
                    _base,
                )
                .await;
                continue;
            }
            PermissionBehavior::Ask => {
                tc_mut.state = ToolCallState::Asking;
                emit_require_user_confirm(event_tx, reply_id, &tc_mut, _base).await;
                emit_denied_tool_result(
                    event_tx,
                    reply_id,
                    inner,
                    &tc_mut,
                    &decision.message,
                    _base,
                )
                .await;
                continue;
            }
            PermissionBehavior::Allow | PermissionBehavior::Passthrough => {
                tc_mut.state = ToolCallState::Allowed;
            }
        }

        let exec_result = if let Some(ref tk) = inner.config.toolkit {
            tk.call_tool(&tc_mut).await
        } else {
            Err(ToolError::NotFound {
                tool_name: tc_mut.name.clone(),
            })
        };

        // Track the consecutive failure streak so error feedback can escalate
        // (e.g. suggest chunked writes) instead of blindly retrying.
        let retries = match &exec_result {
            Ok(_) => {
                inner
                    .state
                    .write()
                    .unwrap()
                    .record_tool_success(&tc_mut.name);
                0
            }
            Err(_) => inner
                .state
                .write()
                .unwrap()
                .record_tool_failure(&tc_mut.name),
        };

        // Call post_acting for Complete results (P2-11 fix).
        // Extract chunk before .await to avoid holding a borrow on exec_result
        // (ToolExecOutput::Stream is not Sync, making the future !Send).
        let post_acting_chunk = match &exec_result {
            Ok(ToolExecOutput::Complete(chunk)) => Some(chunk.clone()),
            _ => None,
        };
        if let Some(chunk) = post_acting_chunk {
            let result_clone = ToolExecOutput::Complete(chunk);
            for mw in inner.middlewares.iter() {
                let _ = mw.post_acting(&inner.config.name, &result_clone).await;
            }
        }

        // Emit tool result events and collect output text for context
        let output_text = emit_tool_result_and_collect(
            event_tx,
            reply_id,
            &tc_mut,
            exec_result,
            retries,
            stream_handle,
            _base,
        )
        .await;

        // Call post_acting on success (P2-11 fix).
        // We do this after the result was consumed by emit_tool_result_and_collect
        // to avoid holding a reference across an await boundary.
        // post_acting only fires for the Complete variant since Stream is consumed.
        // The batch path (react_loop.rs) follows the same pattern.

        // Feed tool result to context with the ACTUAL collected output
        add_tool_result_to_context(inner, &tc_mut, &output_text);
    }
}

async fn emit_require_user_confirm(
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    tool_call: &ToolCallBlock,
    base: fn() -> EventBase,
) {
    let _ = event_tx
        .send(AgentEvent::RequireUserConfirm(RequireUserConfirmEvent {
            base: base(),
            reply_id: reply_id.into(),
            tool_calls: vec![tool_call.clone()],
        }))
        .await;
}

async fn emit_denied_tool_result(
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    inner: &Arc<AgentInner>,
    tool_call: &ToolCallBlock,
    message: &str,
    base: fn() -> EventBase,
) {
    let _ = event_tx
        .send(AgentEvent::ToolResultStart(ToolResultStartEvent {
            base: base(),
            reply_id: reply_id.into(),
            tool_call_id: tool_call.id.clone(),
            tool_call_name: tool_call.name.clone(),
        }))
        .await;
    let _ = event_tx
        .send(AgentEvent::ToolResultTextDelta(ToolResultTextDeltaEvent {
            base: base(),
            reply_id: reply_id.into(),
            tool_call_id: tool_call.id.clone(),
            delta: message.to_string(),
        }))
        .await;
    let _ = event_tx
        .send(AgentEvent::ToolResultEnd(ToolResultEndEvent {
            base: base(),
            reply_id: reply_id.into(),
            tool_call_id: tool_call.id.clone(),
            state: ToolResultState::Denied,
            metadata: std::collections::HashMap::new(),
            output: Some(message.to_string()),
        }))
        .await;
    add_tool_result_to_context(inner, tool_call, message);
}

fn add_tool_result_to_context(inner: &Arc<AgentInner>, tc: &ToolCallBlock, output_text: &str) {
    let trb = ToolResultBlock::new(
        tc.id.clone(),
        tc.name.clone(),
        ToolOutput::Text(output_text.to_string()),
    );
    if let Ok(msg) = Msg::new(
        inner.config.name.clone(),
        vec![ContentBlock::ToolResult(trb)],
        Role::Assistant,
    ) {
        inner.state.write().unwrap().context.push(msg);
    }
}

fn add_text_to_context(inner: &Arc<AgentInner>, response: &ChatResponse) {
    for block in &response.content {
        if let ContentBlock::Text(tb) = block
            && let Ok(msg) = Msg::new(
                inner.config.name.clone(),
                vec![ContentBlock::Text(tb.clone())],
                Role::Assistant,
            )
        {
            inner.state.write().unwrap().context.push(msg);
        }
    }
}

/// Store tool call blocks as an assistant message in context.
///
/// OpenAI-compatible APIs require the assistant message containing tool calls
/// to appear BEFORE the subsequent tool result messages in the conversation
/// history. This function writes the assistant(tool_calls) message to context.
fn add_tool_calls_to_context(inner: &Arc<AgentInner>, tool_calls: &[ToolCallBlock]) {
    if tool_calls.is_empty() {
        return;
    }
    let tc_blocks: Vec<ContentBlock> = tool_calls
        .iter()
        .map(|tc| ContentBlock::ToolCall(tc.clone()))
        .collect();
    if let Ok(msg) = Msg::new(inner.config.name.clone(), tc_blocks, Role::Assistant) {
        inner.state.write().unwrap().context.push(msg);
    }
}

// ---------------------------------------------------------------------------
// Middleware helpers (P2-11 fix)
// ---------------------------------------------------------------------------

/// Collect all text from the agent's context to build a result `Msg` for
/// `post_reply` middleware hooks when the streaming reply completes normally.
fn collect_context_text(inner: &Arc<AgentInner>) -> String {
    let state = inner.state.read().unwrap();
    let mut texts = Vec::new();
    for msg in &state.context {
        for block in &msg.content {
            if let ContentBlock::Text(tb) = block {
                texts.push(tb.text.clone());
            }
        }
    }
    texts.join("")
}

/// Call `post_reply` on every registered middleware.
///
/// Error results from middleware hooks are logged (via tracing) but not
/// propagated — unlike the batch path, streaming exit paths must emit
/// terminal events even if middleware errors occur.
async fn invoke_post_reply(inner: &Arc<AgentInner>, result: &Result<Msg, AgentError>) {
    for mw in inner.middlewares.iter() {
        if let Err(e) = mw.post_reply(&inner.config.name, result).await {
            tracing::warn!(
                agent_name = %inner.config.name,
                error = %e,
                "post_reply middleware hook failed"
            );
        }
    }

    // Auto-persist the latest state at every reply end — normal, interrupted,
    // max-iters, and tool-error paths all flow through here (spec FR-006).
    //
    // The save runs on a short-lived background task rather than awaiting here:
    // blocking on disk I/O would delay the reactor's exit and keep
    // `is_streaming` set, breaking the "agent is immediately reusable after
    // the stream ends" guarantee. The save is serialized via `persist_lock`, so
    // it can never race a following reply's save on the same session file.
    crate::react_agent::spawn_persist_after_reply(inner);
}

// ---------------------------------------------------------------------------
// Outcome
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum Outcome {
    Done,
    Continue,
}

impl Outcome {
    fn is_done(&self) -> bool {
        matches!(*self, Outcome::Done)
    }
}

// ---------------------------------------------------------------------------
// Event emission helpers
// ---------------------------------------------------------------------------

async fn emit_start(
    tx: &mpsc::Sender<AgentEvent>,
    sid: &str,
    rid: &str,
    name: &str,
    base: fn() -> EventBase,
) {
    let _ = tx
        .send(AgentEvent::ReplyStart(agent_scope_event::ReplyStartEvent {
            base: base(),
            session_id: sid.into(),
            reply_id: rid.into(),
            name: name.into(),
            role: "assistant".into(),
        }))
        .await;
}

async fn emit_interrupted(
    tx: &mpsc::Sender<AgentEvent>,
    rid: &str,
    sid: &str,
    base: fn() -> EventBase,
) {
    let _ = tx
        .send(AgentEvent::UserInterrupt(UserInterruptEvent {
            base: base(),
            reply_id: rid.into(),
        }))
        .await;
    let _ = tx
        .send(AgentEvent::ReplyEnd(ReplyEndEvent {
            base: base(),
            session_id: sid.into(),
            reply_id: rid.into(),
            finished_reason: ReplyFinishedReason::Interrupted,
            error: None,
        }))
        .await;
}

async fn emit_max_iters(
    tx: &mpsc::Sender<AgentEvent>,
    rid: &str,
    sid: &str,
    name: &str,
    base: fn() -> EventBase,
) {
    let _ = tx
        .send(AgentEvent::ExceedMaxIters(ExceedMaxItersEvent {
            base: base(),
            reply_id: rid.into(),
            name: name.into(),
        }))
        .await;
    let _ = tx
        .send(AgentEvent::ReplyEnd(ReplyEndEvent {
            base: base(),
            session_id: sid.into(),
            reply_id: rid.into(),
            finished_reason: ReplyFinishedReason::Completed,
            error: None,
        }))
        .await;
}

/// Emit ReplyEnd with completed state — normal text-only reply completion.
async fn emit_completed_reply_end(
    tx: &mpsc::Sender<AgentEvent>,
    sid: &str,
    rid: &str,
    base: fn() -> EventBase,
) {
    let _ = tx
        .send(AgentEvent::ReplyEnd(ReplyEndEvent {
            base: base(),
            session_id: sid.into(),
            reply_id: rid.into(),
            finished_reason: ReplyFinishedReason::Completed,
            error: None,
        }))
        .await;
}

/// Emit ReplyEnd with an error (P2-10 fix: use Error finished_reason).
async fn emit_error_end(
    tx: &mpsc::Sender<AgentEvent>,
    rid: &str,
    sid: &str,
    msg: &str,
    base: fn() -> EventBase,
) {
    let _ = tx
        .send(AgentEvent::ReplyEnd(ReplyEndEvent {
            base: base(),
            session_id: sid.into(),
            reply_id: rid.into(),
            finished_reason: ReplyFinishedReason::Error,
            error: Some(ErrorInfo {
                error_type: ErrorType::Internal,
                message: msg.into(),
            }),
        }))
        .await;
}

async fn emit_model_call_end_with_usage(
    tx: &mpsc::Sender<AgentEvent>,
    rid: &str,
    usage: &Option<ChatUsage>,
    base: fn() -> EventBase,
) {
    let it = usage.as_ref().map_or(0, |u| u.input_tokens);
    let ot = usage.as_ref().map_or(0, |u| u.output_tokens);
    let _ = tx
        .send(AgentEvent::ModelCallEnd(ModelCallEndEvent {
            base: base(),
            reply_id: rid.into(),
            input_tokens: it,
            output_tokens: ot,
            finished_reason: ReplyFinishedReason::Completed,
        }))
        .await;
}

/// Emit text AND thinking events for a Complete (non-streaming) response.
/// Tool call events are emitted separately by process_response_and_continue.
async fn emit_text_events_only(
    response: &ChatResponse,
    tx: &mpsc::Sender<AgentEvent>,
    rid: &str,
    base: fn() -> EventBase,
) {
    for block in &response.content {
        match block {
            ContentBlock::Text(tb) => {
                if tb.text.is_empty() {
                    continue;
                }
                let bid = tb.id.clone();
                let _ = tx
                    .send(AgentEvent::TextBlockStart(TextBlockStartEvent {
                        base: base(),
                        reply_id: rid.into(),
                        block_id: bid.clone(),
                    }))
                    .await;
                let _ = tx
                    .send(AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
                        base: base(),
                        reply_id: rid.into(),
                        block_id: bid.clone(),
                        delta: tb.text.clone(),
                    }))
                    .await;
                let _ = tx
                    .send(AgentEvent::TextBlockEnd(TextBlockEndEvent {
                        base: base(),
                        reply_id: rid.into(),
                        block_id: bid,
                        text: Some(tb.text.clone()),
                    }))
                    .await;
            }
            ContentBlock::Thinking(thb) => {
                if thb.thinking.is_empty() {
                    continue;
                }
                let bid = thb.id.clone();
                let _ = tx
                    .send(AgentEvent::ThinkingBlockStart(ThinkingBlockStartEvent {
                        base: base(),
                        reply_id: rid.into(),
                        block_id: bid.clone(),
                    }))
                    .await;
                let _ = tx
                    .send(AgentEvent::ThinkingBlockDelta(ThinkingBlockDeltaEvent {
                        base: base(),
                        reply_id: rid.into(),
                        block_id: bid.clone(),
                        delta: thb.thinking.clone(),
                    }))
                    .await;
                let _ = tx
                    .send(AgentEvent::ThinkingBlockEnd(ThinkingBlockEndEvent {
                        base: base(),
                        reply_id: rid.into(),
                        block_id: bid,
                        thinking: Some(thb.thinking.clone()),
                    }))
                    .await;
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tool result emission (US3: streaming tool output)
// ---------------------------------------------------------------------------

/// Emit tool result events and return the collected output text.
///
/// For `ToolExecOutput::Complete`: emits Start → Delta → End, returns the text.
/// For `ToolExecOutput::Stream`: emits events progressively, collects ALL output
/// text (not placeholder), returns concatenated text (P0-4 fix).
async fn emit_tool_result_and_collect(
    tx: &mpsc::Sender<AgentEvent>,
    rid: &str,
    tc: &ToolCallBlock,
    result: Result<ToolExecOutput, ToolError>,
    retries: u32,
    stream_handle: &StreamHandle,
    _base: fn() -> EventBase,
) -> String {
    let b = EventBase::new;
    match result {
        Ok(ToolExecOutput::Complete(chunk)) => {
            let st = chunk.state.clone();
            let text = match &chunk.output {
                ToolOutput::Text(t) => t.clone(),
                ToolOutput::Blocks(_) => "[blocks]".into(),
            };
            let output = match &chunk.output {
                ToolOutput::Text(t) => Some(t.clone()),
                // [blocks] output is a placeholder; omit complete output to avoid
                // misleading consumers (research Decision 5).
                ToolOutput::Blocks(_) => None,
            };

            let _ = tx
                .send(AgentEvent::ToolResultStart(ToolResultStartEvent {
                    base: b(),
                    reply_id: rid.into(),
                    tool_call_id: tc.id.clone(),
                    tool_call_name: tc.name.clone(),
                }))
                .await;
            let _ = tx
                .send(AgentEvent::ToolResultTextDelta(ToolResultTextDeltaEvent {
                    base: b(),
                    reply_id: rid.into(),
                    tool_call_id: tc.id.clone(),
                    delta: text.clone(),
                }))
                .await;
            let _ = tx
                .send(AgentEvent::ToolResultEnd(ToolResultEndEvent {
                    base: b(),
                    reply_id: rid.into(),
                    tool_call_id: tc.id.clone(),
                    state: st,
                    metadata: std::collections::HashMap::new(),
                    output,
                }))
                .await;

            text
        }
        Ok(ToolExecOutput::Stream(mut stream)) => {
            let _ = tx
                .send(AgentEvent::ToolResultStart(ToolResultStartEvent {
                    base: b(),
                    reply_id: rid.into(),
                    tool_call_id: tc.id.clone(),
                    tool_call_name: tc.name.clone(),
                }))
                .await;

            let mut collected = String::new();
            let mut final_state = ToolResultState::Success;
            let mut is_done = false;

            while !is_done {
                // Check cancellation between chunks
                if stream_handle.is_cancelled() {
                    let _ = tx
                        .send(AgentEvent::ToolResultEnd(ToolResultEndEvent {
                            base: b(),
                            reply_id: rid.into(),
                            tool_call_id: tc.id.clone(),
                            state: ToolResultState::Interrupted,
                            metadata: std::collections::HashMap::new(),
                            output: None,
                        }))
                        .await;
                    return collected;
                }

                match stream.next().await {
                    Some(Ok(chunk)) => {
                        let txt = match &chunk.output {
                            ToolOutput::Text(t) => t.clone(),
                            _ => "[blocks]".into(),
                        };
                        collected.push_str(&txt);
                        let _ = tx
                            .send(AgentEvent::ToolResultTextDelta(ToolResultTextDeltaEvent {
                                base: b(),
                                reply_id: rid.into(),
                                tool_call_id: tc.id.clone(),
                                delta: txt,
                            }))
                            .await;
                        if chunk.is_last {
                            is_done = true;
                        }
                    }
                    Some(Err(_)) => {
                        final_state = ToolResultState::Error;
                        is_done = true;
                    }
                    None => {
                        // Stream ended without is_last — treat as success
                        is_done = true;
                    }
                }
            }

            let _ = tx
                .send(AgentEvent::ToolResultEnd(ToolResultEndEvent {
                    base: b(),
                    reply_id: rid.into(),
                    tool_call_id: tc.id.clone(),
                    state: final_state,
                    metadata: std::collections::HashMap::new(),
                    output: Some(collected.clone()),
                }))
                .await;

            collected
        }
        Err(e) => {
            // Emit an actionable feedback delta so the failure is visible both
            // in the event stream and in the model's context.
            let feedback = tool_error_feedback(&tc.name, &e, retries);
            let _ = tx
                .send(AgentEvent::ToolResultStart(ToolResultStartEvent {
                    base: b(),
                    reply_id: rid.into(),
                    tool_call_id: tc.id.clone(),
                    tool_call_name: tc.name.clone(),
                }))
                .await;
            let _ = tx
                .send(AgentEvent::ToolResultTextDelta(ToolResultTextDeltaEvent {
                    base: b(),
                    reply_id: rid.into(),
                    tool_call_id: tc.id.clone(),
                    delta: feedback.clone(),
                }))
                .await;
            let _ = tx
                .send(AgentEvent::ToolResultEnd(ToolResultEndEvent {
                    base: b(),
                    reply_id: rid.into(),
                    tool_call_id: tc.id.clone(),
                    state: ToolResultState::Error,
                    metadata: std::collections::HashMap::new(),
                    output: Some(feedback.clone()),
                }))
                .await;
            feedback
        }
    }
}
