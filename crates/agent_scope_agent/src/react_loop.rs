//! Reasoning→Acting loop — the core iteration logic for ReActAgent.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_scope_event::{
    AgentEvent, EventBase, ExceedMaxItersEvent, ModelCallEndEvent, ModelCallStartEvent,
    ReplyEndEvent, ReplyStartEvent, TextBlockDeltaEvent, TextBlockEndEvent, TextBlockStartEvent,
    ThinkingBlockDeltaEvent, ThinkingBlockEndEvent, ThinkingBlockStartEvent,
    ToolCallEndEvent, ToolCallStartEvent, ToolResultEndEvent, ToolResultStartEvent,
    ToolResultTextDeltaEvent, UserInterruptEvent,
};
use agent_scope_message::{
    ContentBlock, Msg, Role, TextBlock, ToolOutput, ToolResultBlock, ToolResultState,
};
use agent_scope_model::{ChatResponse, ModelCallResult, StreamAccumulator};
use agent_scope_state::AgentState;
use agent_scope_tool::{ToolError, ToolExecOutput, ToolKit};
use agent_scope_types::ReplyFinishedReason;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::agent_error::AgentError;
use crate::config::{ContextConfig, ReActConfig};
use crate::context_compression::compress_context;
use crate::middleware::Middleware;

/// Aggregated context for `run_react_loop`, grouping the parameters
/// that are threaded through every reasoning→acting iteration.
pub(crate) struct ReactLoopContext<'a> {
    pub agent_name: &'a str,
    pub session_id: &'a str,
    pub reply_id: &'a str,
    pub system_prompt: &'a str,
    pub react_config: &'a ReActConfig,
    pub context_config: &'a ContextConfig,
    pub model: &'a Arc<dyn agent_scope_model::ChatModel>,
    pub toolkit: &'a Option<ToolKit>,
    pub middlewares: &'a [Arc<dyn Middleware>],
    pub state: &'a std::sync::RwLock<AgentState>,
    pub interrupted: &'a AtomicBool,
    /// Cancellation token — checked via `select!` during model calls and stream
    /// consumption to interrupt in-progress LLM API calls.
    pub cancel_token: &'a CancellationToken,
}

#[derive(Debug)]
enum LoopOutcome {
    Text(Vec<Msg>),
    ToolCalls(Vec<agent_scope_message::ToolCallBlock>),
    Empty,
}

/// Run the ReAct loop, emitting events through `event_tx`.
///
/// This is the **batch** / `reply()` path — it accumulates internally but
/// sends events through an mpsc channel so `do_reply` can collect them.
pub(crate) async fn run_react_loop(
    ctx: ReactLoopContext<'_>,
    event_tx: &mpsc::Sender<AgentEvent>,
) -> Result<Msg, AgentError> {
    let base = || EventBase::new();

    let _ = event_tx
        .send(AgentEvent::ReplyStart(ReplyStartEvent {
            base: base(),
            session_id: ctx.session_id.into(),
            reply_id: ctx.reply_id.into(),
            name: ctx.agent_name.into(),
            role: "assistant".into(),
        }))
        .await;

    let mut accumulated_texts: Vec<String> = Vec::new();
    let mut cur_iter: u32 = 0;

    loop {
        if ctx.interrupted.load(Ordering::SeqCst) {
            let _ = event_tx
                .send(AgentEvent::UserInterrupt(UserInterruptEvent {
                    base: base(),
                    reply_id: ctx.reply_id.into(),
                }))
                .await;
            let _ = event_tx
                .send(AgentEvent::ReplyEnd(ReplyEndEvent {
                    base: base(),
                    session_id: ctx.session_id.into(),
                    reply_id: ctx.reply_id.into(),
                    finished_reason: ReplyFinishedReason::Interrupted,
                    error: None,
                }))
                .await;
            return Ok(build_interruption_msg(
                &ctx.react_config.interruption_message,
            ));
        }

        if cur_iter >= ctx.react_config.max_iters {
            let _ = event_tx
                .send(AgentEvent::ExceedMaxIters(ExceedMaxItersEvent {
                    base: base(),
                    reply_id: ctx.reply_id.into(),
                    name: ctx.agent_name.into(),
                }))
                .await;
            let _ = event_tx
                .send(AgentEvent::ReplyEnd(ReplyEndEvent {
                    base: base(),
                    session_id: ctx.session_id.into(),
                    reply_id: ctx.reply_id.into(),
                    finished_reason: ReplyFinishedReason::Completed,
                    error: None,
                }))
                .await;
            return Ok(build_final_msg(&accumulated_texts));
        }

        cur_iter += 1;

        let messages = {
            let state_read = ctx.state.read().unwrap();
            state_read.context.clone()
        };

        let mut hook_messages = messages.clone();
        if !ctx.system_prompt.is_empty()
            && let Ok(system_msg) =
                agent_scope_message::factory::system_msg("system", ctx.system_prompt)
        {
            hook_messages.insert(0, system_msg);
        }
        let mut hook_tools = ctx.toolkit.as_ref().map(|tk| tk.get_tool_schemas());
        for mw in ctx.middlewares.iter() {
            mw.pre_reasoning(ctx.agent_name, &mut hook_messages, &mut hook_tools)
                .await?;
        }

        let tool_schemas = ctx.toolkit.as_ref().map(|tk| tk.get_tool_schemas());

        // Context compression check — mirrors Python `_compress_memory_if_needed()`
        if ctx.context_config.enable {
            let token_count = ctx
                .model
                .count_tokens(&hook_messages, tool_schemas.as_deref());
            let context_size = ctx.model.context_size();
            let trigger = (context_size as f64 * ctx.context_config.trigger_ratio) as usize;
            if token_count > trigger {
                compress_context(ctx.model, ctx.state, ctx.context_config, ctx.session_id).await?;
            }
        }

        let _ = event_tx
            .send(AgentEvent::ModelCallStart(ModelCallStartEvent {
                base: base(),
                reply_id: ctx.reply_id.into(),
                model_name: ctx.model.model_name().into(),
            }))
            .await;

        // Use select! to allow cancellation during the model call.
        let call_future = ctx
            .model
            .call(&hook_messages, tool_schemas.as_deref(), None);
        let result = tokio::select! {
            r = call_future => r?,
            _ = ctx.cancel_token.cancelled() => {
                let _ = event_tx
                    .send(AgentEvent::UserInterrupt(UserInterruptEvent {
                        base: base(),
                        reply_id: ctx.reply_id.into(),
                    }))
                    .await;
                let _ = event_tx
                    .send(AgentEvent::ReplyEnd(ReplyEndEvent {
                        base: base(),
                        session_id: ctx.session_id.into(),
                        reply_id: ctx.reply_id.into(),
                        finished_reason: ReplyFinishedReason::Interrupted,
                        error: None,
                    }))
                    .await;
                return Ok(build_interruption_msg(
                    &ctx.react_config.interruption_message,
                ));
            }
        };

        // Accumulate stream chunks into a complete response via StreamAccumulator,
        // so DashScope (default stream=true) works with ReActAgent without change.
        // Each stream poll is also cancellable.
        let response = match result {
            ModelCallResult::Complete(resp) => resp,
            ModelCallResult::Stream(mut stream) => {
                let mut acc = StreamAccumulator::new();
                loop {
                    let chunk_result = tokio::select! {
                        r = stream.next() => r,
                        _ = ctx.cancel_token.cancelled() => None,
                    };
                    match chunk_result {
                        Some(Ok(chunk)) => {
                            acc.append_chat_response(&chunk);
                        }
                        Some(Err(e)) => return Err(e.into()),
                        None => {
                            // Stream ended or cancelled — check which
                            if ctx.interrupted.load(Ordering::SeqCst) {
                                let _ = event_tx
                                    .send(AgentEvent::UserInterrupt(UserInterruptEvent {
                                        base: base(),
                                        reply_id: ctx.reply_id.into(),
                                    }))
                                    .await;
                                let _ = event_tx
                                    .send(AgentEvent::ReplyEnd(ReplyEndEvent {
                                        base: base(),
                                        session_id: ctx.session_id.into(),
                                        reply_id: ctx.reply_id.into(),
                                        finished_reason: ReplyFinishedReason::Interrupted,
                                        error: None,
                                    }))
                                    .await;
                                return Ok(build_interruption_msg(
                                    &ctx.react_config.interruption_message,
                                ));
                            }
                            break;
                        }
                    }
                }
                acc.build()
            }
        };

        let _ = event_tx
            .send(AgentEvent::ModelCallEnd(ModelCallEndEvent {
                base: base(),
                reply_id: ctx.reply_id.into(),
                input_tokens: response.usage.as_ref().map_or(0, |u| u.input_tokens),
                output_tokens: response.usage.as_ref().map_or(0, |u| u.output_tokens),
                finished_reason: ReplyFinishedReason::Completed,
            }))
            .await;

        for mw in ctx.middlewares.iter() {
            mw.post_reasoning(ctx.agent_name, &response).await?;
        }

        let outcome = classify_response(&response);

        match outcome {
            LoopOutcome::Text(ref text_msgs) => {
                for msg in text_msgs {
                    for block in &msg.content {
                        match block {
                            ContentBlock::Text(tb) => {
                                let block_id = uuid::Uuid::new_v4().as_simple().to_string();
                                let _ = event_tx
                                    .send(AgentEvent::TextBlockStart(TextBlockStartEvent {
                                        base: base(),
                                        reply_id: ctx.reply_id.into(),
                                        block_id: block_id.clone(),
                                    }))
                                    .await;
                                let _ = event_tx
                                    .send(AgentEvent::TextBlockDelta(TextBlockDeltaEvent {
                                        base: base(),
                                        reply_id: ctx.reply_id.into(),
                                        block_id: block_id.clone(),
                                        delta: tb.text.clone(),
                                    }))
                                    .await;
                                let _ = event_tx
                                    .send(AgentEvent::TextBlockEnd(TextBlockEndEvent {
                                        base: base(),
                                        reply_id: ctx.reply_id.into(),
                                        block_id,
                                    }))
                                    .await;
                                accumulated_texts.push(tb.text.clone());
                            }
                            ContentBlock::Thinking(thb) => {
                                if thb.thinking.is_empty() {
                                    continue;
                                }
                                let block_id = uuid::Uuid::new_v4().as_simple().to_string();
                                let _ = event_tx
                                    .send(AgentEvent::ThinkingBlockStart(ThinkingBlockStartEvent {
                                        base: base(),
                                        reply_id: ctx.reply_id.into(),
                                        block_id: block_id.clone(),
                                    }))
                                    .await;
                                let _ = event_tx
                                    .send(AgentEvent::ThinkingBlockDelta(ThinkingBlockDeltaEvent {
                                        base: base(),
                                        reply_id: ctx.reply_id.into(),
                                        block_id: block_id.clone(),
                                        delta: thb.thinking.clone(),
                                    }))
                                    .await;
                                let _ = event_tx
                                    .send(AgentEvent::ThinkingBlockEnd(ThinkingBlockEndEvent {
                                        base: base(),
                                        reply_id: ctx.reply_id.into(),
                                        block_id,
                                    }))
                                    .await;
                            }
                            _ => {}
                        }
                    }
                }

                {
                    let mut state_write = ctx.state.write().unwrap();
                    for msg in text_msgs {
                        state_write.context.push(msg.clone());
                    }
                }

                let _ = event_tx
                    .send(AgentEvent::ReplyEnd(ReplyEndEvent {
                        base: base(),
                        session_id: ctx.session_id.into(),
                        reply_id: ctx.reply_id.into(),
                        finished_reason: ReplyFinishedReason::Completed,
                        error: None,
                    }))
                    .await;

                return Ok(build_final_msg(&accumulated_texts));
            }

            LoopOutcome::ToolCalls(tool_calls) => {
                // Store the assistant message with tool calls to context FIRST.
                // OpenAI-compatible APIs require: assistant(tool_calls) → tool(result).
                // Without the assistant message, the model doesn't know which
                // tool call the result corresponds to.
                {
                    let mut state_write = ctx.state.write().unwrap();
                    let tc_blocks: Vec<ContentBlock> = tool_calls
                        .iter()
                        .map(|tc| ContentBlock::ToolCall(tc.clone()))
                        .collect();
                    if let Ok(assistant_msg) = Msg::new(
                        ctx.agent_name.into(),
                        tc_blocks,
                        Role::Assistant,
                    ) {
                        state_write.context.push(assistant_msg);
                    }
                }

                for tc in &tool_calls {
                    let mut tc_mut = tc.clone();
                    for mw in ctx.middlewares.iter() {
                        mw.pre_acting(ctx.agent_name, &mut tc_mut).await?;
                    }

                    let _ = event_tx
                        .send(AgentEvent::ToolCallStart(ToolCallStartEvent {
                            base: base(),
                            reply_id: ctx.reply_id.into(),
                            tool_call_id: tc_mut.id.clone(),
                            tool_call_name: tc_mut.name.clone(),
                        }))
                        .await;

                    let exec_result = if let Some(tk) = ctx.toolkit {
                        tk.call_tool(&tc_mut).await
                    } else {
                        Err(ToolError::NotFound {
                            tool_name: tc_mut.name.clone(),
                        })
                    };

                    let _ = event_tx
                        .send(AgentEvent::ToolCallEnd(ToolCallEndEvent {
                            base: base(),
                            reply_id: ctx.reply_id.into(),
                            tool_call_id: tc_mut.id.clone(),
                        }))
                        .await;

                    match exec_result {
                        Ok(ToolExecOutput::Complete(chunk)) => {
                            let result_state = chunk.state.clone();
                            let output_text = match &chunk.output {
                                ToolOutput::Text(t) => t.clone(),
                                ToolOutput::Blocks(_) => "[blocks]".into(),
                            };

                            let _ = event_tx
                                .send(AgentEvent::ToolResultStart(ToolResultStartEvent {
                                    base: base(),
                                    reply_id: ctx.reply_id.into(),
                                    tool_call_id: tc_mut.id.clone(),
                                    tool_call_name: tc_mut.name.clone(),
                                }))
                                .await;
                            let _ = event_tx
                                .send(AgentEvent::ToolResultTextDelta(ToolResultTextDeltaEvent {
                                    base: base(),
                                    reply_id: ctx.reply_id.into(),
                                    tool_call_id: tc_mut.id.clone(),
                                    delta: output_text.clone(),
                                }))
                                .await;
                            let _ = event_tx
                                .send(AgentEvent::ToolResultEnd(ToolResultEndEvent {
                                    base: base(),
                                    reply_id: ctx.reply_id.into(),
                                    tool_call_id: tc_mut.id.clone(),
                                    state: result_state,
                                    metadata: std::collections::HashMap::new(),
                                }))
                                .await;

                            let result_clone = ToolExecOutput::Complete(chunk.clone());
                            for mw in ctx.middlewares.iter() {
                                mw.post_acting(ctx.agent_name, &result_clone).await?;
                            }

                            {
                                let mut state_write = ctx.state.write().unwrap();
                                let trb = ToolResultBlock::new(
                                    tc_mut.id.clone(),
                                    tc_mut.name.clone(),
                                    ToolOutput::Text(output_text),
                                );
                                if let Ok(msg) = Msg::new(
                                    ctx.agent_name.into(),
                                    vec![ContentBlock::ToolResult(trb)],
                                    Role::Assistant,
                                ) {
                                    state_write.context.push(msg);
                                }
                            }
                        }
                        Err(tool_err) => {
                            let _ = event_tx
                                .send(AgentEvent::ToolResultStart(ToolResultStartEvent {
                                    base: base(),
                                    reply_id: ctx.reply_id.into(),
                                    tool_call_id: tc_mut.id.clone(),
                                    tool_call_name: tc_mut.name.clone(),
                                }))
                                .await;
                            let _ = event_tx
                                .send(AgentEvent::ToolResultEnd(ToolResultEndEvent {
                                    base: base(),
                                    reply_id: ctx.reply_id.into(),
                                    tool_call_id: tc_mut.id.clone(),
                                    state: ToolResultState::Error,
                                    metadata: std::collections::HashMap::new(),
                                }))
                                .await;

                            {
                                let mut state_write = ctx.state.write().unwrap();
                                let trb = ToolResultBlock::new(
                                    tc_mut.id.clone(),
                                    tc_mut.name.clone(),
                                    ToolOutput::Text(format!("Error: {tool_err}")),
                                );
                                if let Ok(msg) = Msg::new(
                                    ctx.agent_name.into(),
                                    vec![ContentBlock::ToolResult(trb)],
                                    Role::Assistant,
                                ) {
                                    state_write.context.push(msg);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            LoopOutcome::Empty => {
                let _ = event_tx
                    .send(AgentEvent::ReplyEnd(ReplyEndEvent {
                        base: base(),
                        session_id: ctx.session_id.into(),
                        reply_id: ctx.reply_id.into(),
                        finished_reason: ReplyFinishedReason::Completed,
                        error: None,
                    }))
                    .await;
                return Ok(build_final_msg(&accumulated_texts));
            }
        }
    }
}

fn classify_response(response: &ChatResponse) -> LoopOutcome {
    let mut text_msgs = Vec::new();
    let mut tool_calls = Vec::new();

    for block in &response.content {
        match block {
            ContentBlock::Text(tb) => {
                if let Ok(msg) = Msg::new(
                    "assistant".into(),
                    vec![ContentBlock::Text(tb.clone())],
                    Role::Assistant,
                ) {
                    text_msgs.push(msg);
                }
            }
            ContentBlock::ToolCall(tc) => {
                tool_calls.push(tc.clone());
            }
            _ => {}
        }
    }

    if !tool_calls.is_empty() {
        LoopOutcome::ToolCalls(tool_calls)
    } else if !text_msgs.is_empty() {
        LoopOutcome::Text(text_msgs)
    } else {
        LoopOutcome::Empty
    }
}

fn build_final_msg(texts: &[String]) -> Msg {
    let combined = texts.join("");
    if combined.is_empty() {
        Msg::new(
            "assistant".into(),
            vec![ContentBlock::Text(TextBlock::new("".into()))],
            Role::Assistant,
        )
        .unwrap()
    } else {
        Msg::new(
            "assistant".into(),
            vec![ContentBlock::Text(TextBlock::new(combined))],
            Role::Assistant,
        )
        .unwrap()
    }
}

fn build_interruption_msg(message: &str) -> Msg {
    Msg::new(
        "assistant".into(),
        vec![ContentBlock::Text(TextBlock::new(message.into()))],
        Role::Assistant,
    )
    .unwrap()
}
