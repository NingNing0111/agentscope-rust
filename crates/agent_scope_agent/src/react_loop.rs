//! Reasoning→Acting loop — the core iteration logic for ReActAgent.

use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_scope_event::{
    AgentEvent, EventBase, ExceedMaxItersEvent, ModelCallEndEvent, ModelCallStartEvent,
    ReplyEndEvent, ReplyStartEvent, RequireUserConfirmEvent, TextBlockDeltaEvent,
    TextBlockEndEvent, TextBlockStartEvent, ThinkingBlockDeltaEvent, ThinkingBlockEndEvent,
    ThinkingBlockStartEvent, ToolCallEndEvent, ToolCallStartEvent, ToolResultEndEvent,
    ToolResultStartEvent, ToolResultTextDeltaEvent, UserInterruptEvent,
};
use agent_scope_message::{
    ContentBlock, Msg, Role, TextBlock, ToolCallState, ToolOutput, ToolResultBlock, ToolResultState,
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
use crate::context_compression::{compress_context, truncate_context};
use crate::middleware::Middleware;
use crate::permission::PermissionBehavior;
use crate::tool_feedback::tool_error_feedback;

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
    /// Shared mutable permission engine (Feature 032). Read on every check so
    /// rules adopted at resume time take effect immediately.
    pub permission_engine: &'a Arc<RwLock<crate::permission::PermissionEngine>>,
    pub middlewares: &'a [Arc<dyn Middleware>],
    pub state: &'a std::sync::RwLock<AgentState>,
    pub interrupted: &'a AtomicBool,
    /// Whether the built-in task planning tools and their reminder injection
    /// are enabled for this agent.
    pub task_tools_enabled: bool,
    /// Runtime-state injection configuration (Feature 026). Controls the time,
    /// task and context-length dimensions.
    pub injection_config: &'a crate::config::InjectionConfig,
    /// Cancellation token — checked via `select!` during model calls and stream
    /// consumption to interrupt in-progress LLM API calls.
    pub cancel_token: &'a CancellationToken,
}

#[derive(Debug)]
enum LoopOutcome {
    Text(Vec<Msg>),
    ToolCalls {
        tool_calls: Vec<agent_scope_message::ToolCallBlock>,
        /// Text blocks that accompanied the tool calls — emitted, appended to
        /// the context and accumulated so they are not silently dropped (the
        /// streaming path already keeps them).
        text_msgs: Vec<Msg>,
    },
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
        if ctx.interrupted.swap(false, Ordering::SeqCst) {
            // Consume the flag so an interruption only affects the current
            // reply: the next `reply()` call runs normally with a fresh token
            // (matches `interrupt()`'s documented contract).
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
                    finished_reason: ReplyFinishedReason::ExceedMaxIters,
                    error: None,
                }))
                .await;
            return Ok(build_final_msg(&accumulated_texts));
        }

        cur_iter += 1;

        // Snapshot the current context purely for the token count used by the
        // compression trigger and the first-iteration context-length dimension.
        // Computed here (before compression/injection mutate the state) so the
        // count reflects the pre-compression context, matching Python.
        let count_messages = {
            let state_read = ctx.state.read().unwrap_or_else(|e| e.into_inner());
            state_read.context.clone()
        };
        let mut count_msgs = count_messages;
        if !ctx.system_prompt.is_empty()
            && let Ok(system_msg) =
                agent_scope_message::factory::system_msg("system", ctx.system_prompt)
        {
            count_msgs.insert(0, system_msg);
        }
        let tool_schemas = ctx.toolkit.as_ref().map(|tk| tk.get_tool_schemas());
        let token_count = ctx.model.count_tokens(&count_msgs, tool_schemas.as_deref());
        let context_size = ctx.model.context_size();
        let trigger = (context_size as f64 * ctx.context_config.trigger_ratio) as usize;

        // Context compression check — mirrors Python `_compress_memory_if_needed()`.
        // Mutates `ctx.state.context` (drains the oldest messages). The token
        // count was computed above (with system prompt + tool schemas) and is
        // passed through so the trigger decision matches this call's budget.
        if ctx.context_config.enable && token_count > trigger {
            let result = compress_context(
                ctx.model,
                ctx.state,
                ctx.context_config,
                ctx.session_id,
                token_count,
            )
            .await;
            if let Err(e) = result {
                tracing::warn!(
                    error = %e,
                    "Context compression failed, falling back to truncation"
                );
                let mut state_write = ctx.state.write().unwrap_or_else(|e| e.into_inner());
                let max_messages = (state_write.context.len() / 2).max(10);
                truncate_context(&mut state_write, ctx.context_config, max_messages);
            }
        }

        // Runtime-state injection (Feature 026) — evaluated each iteration so a
        // compression that removed the task-tool / time traces re-triggers the
        // relevant dimensions. Aligns with Python `_inject_runtime_state`.
        let injection_event = crate::runtime_injection::maybe_inject_runtime_state(
            ctx.state,
            ctx.agent_name,
            ctx.injection_config,
            chrono::Utc::now().fixed_offset(),
            cur_iter,
            // The context-length dimension is only evaluated on the first
            // iteration, where the token count is meaningful.
            (cur_iter == 1).then_some(token_count),
            context_size,
            ctx.context_config.trigger_ratio,
            ctx.task_tools_enabled,
        );
        if let Some(evt) = injection_event {
            let _ = event_tx.send(AgentEvent::HintBlock(evt)).await;
        }

        // Build the actual call messages from the NOW-updated state. Building
        // after compression + injection means the current model call sees the
        // compressed context and the injected runtime hint — previously the
        // pre-compression clone was sent, so on the triggering iteration the
        // injection never reached the model.
        let messages = {
            let state_read = ctx.state.read().unwrap_or_else(|e| e.into_inner());
            state_read.context.clone()
        };
        let mut hook_messages = messages;
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

        let _ = event_tx
            .send(AgentEvent::ModelCallStart(ModelCallStartEvent {
                base: base(),
                reply_id: ctx.reply_id.into(),
                model_name: ctx.model.model_name().into(),
            }))
            .await;

        // Use select! to allow cancellation during the model call.
        let call_future = ctx.model.call(&hook_messages, hook_tools.as_deref(), None);
        let result = tokio::select! {
            r = call_future => r,
            _ = ctx.cancel_token.cancelled() => {
                // Consume the flag so the interruption only affects the current
                // reply (see the top-of-loop check above).
                ctx.interrupted.store(false, Ordering::SeqCst);
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

        // `interrupt()` may have landed between the `select!` resolution and
        // this point (select is unbiased, so when both are ready it may pick
        // the call branch even though the token was cancelled). Re-check the
        // flag and treat it as an interruption rather than a completed reply,
        // so the flag never leaks into the next reply (audit A9).
        if ctx.interrupted.load(std::sync::atomic::Ordering::SeqCst) {
            ctx.interrupted.store(false, Ordering::SeqCst);
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
        let result = result?;

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
                            // Stream ended or cancelled — check which. Consume
                            // `interrupted` so a stream-poll interrupt cannot
                            // leak into the next reply.
                            if ctx.cancel_token.is_cancelled()
                                || ctx.interrupted.swap(false, Ordering::SeqCst)
                            {
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
                                        text: Some(tb.text.clone()),
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
                                        thinking: Some(thb.thinking.clone()),
                                    }))
                                    .await;
                            }
                            _ => {}
                        }
                    }
                }

                {
                    let mut state_write = ctx.state.write().unwrap_or_else(|e| e.into_inner());
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

            LoopOutcome::ToolCalls {
                tool_calls,
                text_msgs,
            } => {
                // Emit and persist any text that accompanied the tool calls so
                // it isn't silently dropped (mirrors the streaming path, which
                // appends text blocks via add_text_to_context).
                for msg in &text_msgs {
                    for block in &msg.content {
                        if let ContentBlock::Text(tb) = block {
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
                                    text: Some(tb.text.clone()),
                                }))
                                .await;
                            accumulated_texts.push(tb.text.clone());
                        }
                    }
                }
                // Store the assistant message with tool calls to context FIRST.
                // OpenAI-compatible APIs require: assistant(tool_calls) → tool(result).
                // Without the assistant message, the model doesn't know which
                // tool call the result corresponds to.
                {
                    let mut state_write = ctx.state.write().unwrap_or_else(|e| e.into_inner());
                    // Persist the accompanying text as its own assistant message
                    // so it survives compression / is visible to later turns.
                    for msg in &text_msgs {
                        state_write.context.push(msg.clone());
                    }
                    let tc_blocks: Vec<ContentBlock> = tool_calls
                        .iter()
                        .map(|tc| ContentBlock::ToolCall(tc.clone()))
                        .collect();
                    if let Ok(assistant_msg) =
                        Msg::new(ctx.agent_name.into(), tc_blocks, Role::Assistant)
                    {
                        state_write.context.push(assistant_msg);
                    }
                }

                for tc in &tool_calls {
                    // Honor cancellation: if the reply was interrupted or the
                    // stream dropped, stop executing side-effectful tools (audit
                    // A4).
                    if ctx.cancel_token.is_cancelled() {
                        break;
                    }
                    let mut tc_mut = tc.clone();
                    for mw in ctx.middlewares.iter() {
                        mw.pre_acting(ctx.agent_name, &mut tc_mut).await?;
                    }
                    if ctx.cancel_token.is_cancelled()
                        || ctx.interrupted.swap(false, Ordering::SeqCst)
                    {
                        break;
                    }

                    let permission_input = serde_json::from_str(&tc_mut.input)
                        .unwrap_or_else(|_| serde_json::Value::String(tc_mut.input.clone()));
                    // The guard is scoped so it is dropped before any `.await`
                    // (RwLockReadGuard is not Send).
                    let decision = {
                        let permission_engine = ctx
                            .permission_engine
                            .read()
                            .unwrap_or_else(|e| e.into_inner());
                        permission_engine.check_decision(&tc_mut.name, &permission_input)
                    };
                    match decision.behavior {
                        PermissionBehavior::Deny => {
                            emit_permission_denied_result(
                                event_tx,
                                ctx.reply_id,
                                ctx.agent_name,
                                ctx.state,
                                &tc_mut,
                                &decision.message,
                                base,
                            )
                            .await;
                            continue;
                        }
                        PermissionBehavior::Ask => {
                            // Feature 032 (Python-aligned): Ask pauses the reply.
                            // Mark the tool_call `asking`, persist that state into
                            // context (so resume can match it), emit the confirm
                            // event and STOP — no denied result is fed back.
                            tc_mut.state = ToolCallState::Asking;
                            tc_mut.suggested_rules = decision
                                .suggested_rules
                                .clone()
                                .unwrap_or_default()
                                .into_iter()
                                .map(super::streaming_reactor::to_message_permission_rule)
                                .collect();
                            update_tool_call_state_in_context(ctx.state, &tc_mut);
                            emit_require_user_confirm(event_tx, ctx.reply_id, &tc_mut, base).await;
                            return Ok(build_final_msg(&accumulated_texts));
                        }
                        PermissionBehavior::Allow | PermissionBehavior::Passthrough => {
                            tc_mut.state = ToolCallState::Allowed;
                        }
                    }

                    // External tool (Feature 032, FR-013): submit the call and
                    // end the reply instead of executing it in-process. The
                    // tool_call is marked `submitted` in context so a later
                    // `ExternalExecutionResultEvent` resume can match it.
                    if tc_mut.state == ToolCallState::Allowed
                        && ctx
                            .toolkit
                            .as_ref()
                            .is_some_and(|tk| tk.is_external_tool(&tc_mut.name))
                    {
                        tc_mut.state = ToolCallState::Submitted;
                        update_tool_call_state_in_context(ctx.state, &tc_mut);
                        let _ = event_tx
                            .send(AgentEvent::RequireExternalExecution(
                                agent_scope_event::RequireExternalExecutionEvent {
                                    base: base(),
                                    reply_id: ctx.reply_id.into(),
                                    tool_calls: vec![tc_mut.clone()],
                                },
                            ))
                            .await;
                        return Ok(build_final_msg(&accumulated_texts));
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

                    // Track the consecutive failure streak so error feedback can
                    // escalate (e.g. suggest chunked writes) instead of blindly
                    // retrying. Mirrors the streaming path.
                    let retries = match &exec_result {
                        Ok(_) => {
                            ctx.state
                                .write()
                                .unwrap_or_else(|e| e.into_inner())
                                .record_tool_success(&tc_mut.name);
                            0
                        }
                        Err(_) => ctx
                            .state
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .record_tool_failure(&tc_mut.name),
                    };

                    let _ = event_tx
                        .send(AgentEvent::ToolCallEnd(ToolCallEndEvent {
                            base: base(),
                            reply_id: ctx.reply_id.into(),
                            tool_call_id: tc_mut.id.clone(),
                            input: Some(tc_mut.input.clone()),
                        }))
                        .await;

                    match exec_result {
                        Ok(ToolExecOutput::Stream(mut stream)) => {
                            // A tool that streams its output (e.g. progressive
                            // task progress). Previously this branch was dropped
                            // entirely, which left the tool_call unpaired in the
                            // context (audit A7). Consume the stream, collect the
                            // text, and write a single ToolResult back — the
                            // batch path cannot emit progress deltas in real time,
                            // but it must not lose the output.
                            let mut collected = String::new();
                            let mut final_state = ToolResultState::Success;
                            let mut is_done = false;
                            while !is_done {
                                let chunk_result = tokio::select! {
                                    r = stream.next() => r,
                                    _ = ctx.cancel_token.cancelled() => None,
                                };
                                match chunk_result {
                                    Some(Ok(chunk)) => {
                                        let txt = match &chunk.output {
                                            ToolOutput::Text(t) => t.clone(),
                                            _ => "[blocks]".into(),
                                        };
                                        collected.push_str(&txt);
                                        if chunk.is_last {
                                            is_done = true;
                                        }
                                    }
                                    Some(Err(_)) => {
                                        final_state = ToolResultState::Error;
                                        is_done = true;
                                    }
                                    None => {
                                        if ctx.cancel_token.is_cancelled()
                                            || ctx.interrupted.swap(false, Ordering::SeqCst)
                                        {
                                            final_state = ToolResultState::Interrupted;
                                            collected.clear();
                                        }
                                        is_done = true;
                                    }
                                }
                            }

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
                                    delta: collected.clone(),
                                }))
                                .await;
                            let _ = event_tx
                                .send(AgentEvent::ToolResultEnd(ToolResultEndEvent {
                                    base: base(),
                                    reply_id: ctx.reply_id.into(),
                                    tool_call_id: tc_mut.id.clone(),
                                    state: final_state.clone(),
                                    metadata: std::collections::HashMap::new(),
                                    output: Some(collected.clone()),
                                }))
                                .await;

                            // (post_acting is only invoked for `Complete`
                            // results, matching the streaming path.)

                            {
                                let mut state_write =
                                    ctx.state.write().unwrap_or_else(|e| e.into_inner());
                                let mut trb = ToolResultBlock::new(
                                    tc_mut.id.clone(),
                                    tc_mut.name.clone(),
                                    ToolOutput::Text(collected),
                                );
                                // Reflect the actual execution outcome so the
                                // persisted context matches the emitted events
                                // instead of always being `Running` (round-5 H2).
                                trb.state = final_state.clone();
                                if let Ok(msg) = Msg::new(
                                    ctx.agent_name.into(),
                                    vec![ContentBlock::ToolResult(trb)],
                                    Role::Assistant,
                                ) {
                                    state_write.context.push(msg);
                                }
                            }
                        }
                        Ok(ToolExecOutput::Complete(chunk)) => {
                            let result_state = chunk.state.clone();
                            let output_text = match &chunk.output {
                                ToolOutput::Text(t) => t.clone(),
                                ToolOutput::Blocks(_) => "[blocks]".into(),
                            };
                            let output = match &chunk.output {
                                ToolOutput::Text(t) => Some(t.clone()),
                                ToolOutput::Blocks(_) => None,
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
                                    state: result_state.clone(),
                                    metadata: std::collections::HashMap::new(),
                                    output,
                                }))
                                .await;

                            let result_clone = ToolExecOutput::Complete(chunk.clone());
                            for mw in ctx.middlewares.iter() {
                                mw.post_acting(ctx.agent_name, &result_clone).await?;
                            }

                            {
                                let mut state_write =
                                    ctx.state.write().unwrap_or_else(|e| e.into_inner());
                                let mut trb = ToolResultBlock::new(
                                    tc_mut.id.clone(),
                                    tc_mut.name.clone(),
                                    ToolOutput::Text(output_text),
                                );
                                // Persist the outcome reported by the tool
                                // (`chunk.state`) rather than the hardcoded
                                // `Running` default (round-5 H2).
                                trb.state = result_state;
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
                            // Emit an actionable feedback delta so the failure is
                            // visible in the event stream (batch path).
                            let feedback = tool_error_feedback(&tc_mut.name, &tool_err, retries);
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
                                    delta: feedback.clone(),
                                }))
                                .await;
                            let _ = event_tx
                                .send(AgentEvent::ToolResultEnd(ToolResultEndEvent {
                                    base: base(),
                                    reply_id: ctx.reply_id.into(),
                                    tool_call_id: tc_mut.id.clone(),
                                    state: ToolResultState::Error,
                                    metadata: std::collections::HashMap::new(),
                                    output: Some(feedback.clone()),
                                }))
                                .await;

                            {
                                let mut state_write =
                                    ctx.state.write().unwrap_or_else(|e| e.into_inner());
                                let mut trb = ToolResultBlock::new(
                                    tc_mut.id.clone(),
                                    tc_mut.name.clone(),
                                    ToolOutput::Text(feedback),
                                );
                                // Error path: the event stream already reported
                                // `Error`; persist the same state instead of the
                                // `Running` default (round-5 H2 follow-up).
                                trb.state = ToolResultState::Error;
                                if let Ok(msg) = Msg::new(
                                    ctx.agent_name.into(),
                                    vec![ContentBlock::ToolResult(trb)],
                                    Role::Assistant,
                                ) {
                                    state_write.context.push(msg);
                                }
                            }
                        }
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

async fn emit_require_user_confirm(
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    tool_call: &agent_scope_message::ToolCallBlock,
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

/// Update the matching tool_call block in the tail assistant message's context
/// to reflect the new state (e.g. `asking` after an Ask decision). The block
/// was written before permission checking, so its state was the original
/// `Pending`; keeping it in sync lets resume match awaiting tool calls via
/// `get_awaiting_tool_calls` (mirrors the streaming path).
fn update_tool_call_state_in_context(
    state: &std::sync::RwLock<AgentState>,
    tc: &agent_scope_message::ToolCallBlock,
) {
    let mut state = state.write().unwrap_or_else(|e| e.into_inner());
    if let Some(last) = state.context.last_mut() {
        for block in &mut last.content {
            if let ContentBlock::ToolCall(existing) = block
                && existing.id == tc.id
            {
                existing.state = tc.state.clone();
                existing.suggested_rules = tc.suggested_rules.clone();
                break;
            }
        }
    }
}

async fn emit_permission_denied_result(
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    agent_name: &str,
    state: &std::sync::RwLock<AgentState>,
    tool_call: &agent_scope_message::ToolCallBlock,
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

    let mut trb = ToolResultBlock::new(
        tool_call.id.clone(),
        tool_call.name.clone(),
        ToolOutput::Text(message.to_string()),
    );
    trb.state = ToolResultState::Denied;
    if let Ok(msg) = Msg::new(
        agent_name.into(),
        vec![ContentBlock::ToolResult(trb)],
        Role::Assistant,
    ) {
        state
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .context
            .push(msg);
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
        LoopOutcome::ToolCalls {
            tool_calls,
            text_msgs,
        }
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
