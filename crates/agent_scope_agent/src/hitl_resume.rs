//! Event-driven HITL resume logic (Feature 032).
//!
//! Implements the host-injected event dispatch that Python's `_agent.py`
//! handles in `_check_incoming_event` / `_handle_incoming_event`:
//!
//! - **Validation** (`check_incoming_event`): the agent must actually be
//!   waiting for the injected event type, all tool-call ids must match, and
//!   the event's `reply_id` must match the paused reply.
//! - **Handling** (`handle_incoming_event`): confirmed tools are executed
//!   (and any accepted `rules` are adopted into the shared permission engine),
//!   denied tools produce a `DENIED` tool result, and external execution
//!   results are appended to the context.
//!
//! After the event is handled the caller continues the *same* reasoning-acting
//! loop — no new `ReplyStart`, the paused `reply_id` is kept.

use std::collections::HashSet;
use std::sync::Arc;

use agent_scope_event::{
    AgentEvent, EventBase, ExternalExecutionResultEvent, ToolCallDeltaEvent, ToolCallEndEvent,
    ToolCallStartEvent, ToolResultEndEvent, ToolResultStartEvent, ToolResultTextDeltaEvent,
    UserConfirmResultEvent,
};
use agent_scope_message::{ContentBlock, Msg, Role, ToolCallBlock, ToolCallState};
use agent_scope_tool::{ToolError, ToolExecOutput};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::agent_error::AgentError;
use crate::event_input::EventInput;
use crate::permission::PermissionRule;
use crate::react_agent::{AgentInner, get_awaiting_tool_calls};
use crate::stream_handle::StreamHandle;
use crate::streaming_reactor;

/// Validate an incoming HITL event against the agent's current waiting state.
///
/// Mirrors Python `_check_incoming_event` (FR-007/FR-008/FR-015) plus the
/// Rust-specific `reply_id` check (FR-010). Returns an `AgentError` describing
/// the mismatch; the agent state machine is never mutated on failure.
pub(crate) fn check_incoming_event(
    inner: &Arc<AgentInner>,
    event: &EventInput,
) -> Result<(), AgentError> {
    let awaiting = get_awaiting_tool_calls(inner);
    let awaiting_confirmations: HashSet<&str> = awaiting
        .iter()
        .filter(|tc| tc.state == ToolCallState::Asking)
        .map(|tc| tc.id.as_str())
        .collect();
    let awaiting_external: HashSet<&str> = awaiting
        .iter()
        .filter(|tc| tc.state == ToolCallState::Submitted)
        .map(|tc| tc.id.as_str())
        .collect();

    match event {
        EventInput::Confirm(e) => {
            if awaiting_confirmations.is_empty() {
                return Err(validation(
                    "Agent is not waiting for user confirmation, but received UserConfirmResultEvent",
                ));
            }
            let extra_ids: Vec<&str> = e
                .confirm_results
                .iter()
                .map(|cr| cr.tool_call.id.as_str())
                .filter(|id| !awaiting_confirmations.contains(id))
                .collect();
            if !extra_ids.is_empty() {
                return Err(validation(format!(
                    "Received UserConfirmResultEvent with tool call ids {extra_ids:?} that are not waiting for confirmation."
                )));
            }
        }
        EventInput::ExternalResult(e) => {
            if awaiting_external.is_empty() {
                return Err(validation(
                    "Agent is not waiting for external execution result, but received ExternalExecutionResultEvent",
                ));
            }
            let extra_ids: Vec<&str> = e
                .execution_results
                .iter()
                .map(|tr| tr.id.as_str())
                .filter(|id| !awaiting_external.contains(id))
                .collect();
            if !extra_ids.is_empty() {
                return Err(validation(format!(
                    "Received ExternalExecutionResultEvent with tool call ids {extra_ids:?} that are not waiting for external execution results."
                )));
            }
        }
        EventInput::Interrupt(_) => {
            // No validation needed — interrupt is a silent no-op when idle.
            return Ok(());
        }
    }

    // FR-010: the event must reference the paused reply. Only meaningful when
    // the agent is actually waiting (the checks above already passed).
    let paused_reply_id = {
        let state = inner.state.read().unwrap_or_else(|e| e.into_inner());
        state.reply_context.reply_id.clone()
    };
    let event_reply_id = match event {
        EventInput::Confirm(e) => e.reply_id.as_str(),
        EventInput::ExternalResult(e) => e.reply_id.as_str(),
        EventInput::Interrupt(e) => e.reply_id.as_str(),
    };
    if !paused_reply_id.is_empty() && event_reply_id != paused_reply_id {
        return Err(validation(format!(
            "reply_id mismatch: the event references reply '{}' but the paused reply is '{}'",
            event_reply_id, paused_reply_id
        )));
    }

    Ok(())
}

/// Apply the validated event to the agent state: execute confirmed tools,
/// reject denied ones, or append external execution results. Emits the
/// corresponding tool result events on `event_tx`.
pub(crate) async fn handle_incoming_event(
    inner: &Arc<AgentInner>,
    event: &EventInput,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    stream_handle: &StreamHandle,
    cancel_token: &CancellationToken,
) {
    match event {
        EventInput::Confirm(e) => {
            handle_confirm(inner, e, event_tx, reply_id, stream_handle, cancel_token).await;
        }
        EventInput::ExternalResult(e) => {
            handle_external_result(inner, e, event_tx, reply_id).await;
        }
        EventInput::Interrupt(_) => {}
    }
}

/// Process a `UserConfirmResultEvent`: for each matching asking tool call,
/// execute it (adopting any accepted rules) or reject it with a `DENIED`
/// result — aligned with Python `_handle_incoming_event`.
async fn handle_confirm(
    inner: &Arc<AgentInner>,
    event: &UserConfirmResultEvent,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    stream_handle: &StreamHandle,
    cancel_token: &CancellationToken,
) {
    // Map confirmed results by tool call id for O(1) lookup.
    let mut confirmed: std::collections::HashMap<String, &agent_scope_event::ConfirmResult> = event
        .confirm_results
        .iter()
        .map(|cr| (cr.tool_call.id.clone(), cr))
        .collect();

    // Process the awaiting asking tool calls in context order (Python iterates
    // the tail assistant message's tool_call blocks).
    let awaiting = get_awaiting_tool_calls(inner);
    for tc in &awaiting {
        if tc.state != ToolCallState::Asking {
            continue;
        }
        let Some(confirmation) = confirmed.remove(&tc.id) else {
            continue;
        };

        if confirmation.confirmed {
            // Mark the tool call allowed in context (Python
            // `_update_tool_call_state(ALLOWED)`) so awaiting detection no
            // longer reports it as pending.
            set_tool_call_state(inner, &tc.id, ToolCallState::Allowed);

            // Adopt any accepted rules into the shared engine (FR-009). A
            // tool-wide allow rule clears the matching ask rule so later calls
            // are not asked again (US3 / "always allow").
            if let Some(rules) = &confirmation.rules {
                for rule in rules {
                    if let Some(engine_rule) = to_engine_permission_rule(rule) {
                        inner
                            .permission_engine
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .adopt_allow_rule(engine_rule);
                    }
                }
            }

            // Execute the confirmed tool call. The confirmation may carry a
            // user-modified name/input (Python allows modification on resume);
            // use the confirmation's tool call for that reason.
            execute_confirmed_tool(
                inner,
                &confirmation.tool_call,
                event_tx,
                reply_id,
                stream_handle,
                cancel_token,
            )
            .await;
        } else {
            // Rejected: tool must NOT execute; emit a DENIED tool result with
            // the aligned <system-reminder> rejection text (FR-006) and mark
            // the tool call finished.
            let message = format!(
                "<system-reminder>The execution of tool \"{}\" is denied by user!</system-reminder>",
                tc.name
            );
            streaming_reactor::emit_denied_tool_result(
                event_tx,
                reply_id,
                inner,
                tc,
                &message,
                EventBase::new,
            )
            .await;
            set_tool_call_state(inner, &tc.id, ToolCallState::Finished);
        }
    }
}

/// Process an `ExternalExecutionResultEvent`: append each execution result to
/// the context and mark the corresponding submitted tool call finished
/// (aligned with Python `_handle_incoming_event`, FR-014).
async fn handle_external_result(
    inner: &Arc<AgentInner>,
    event: &ExternalExecutionResultEvent,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
) {
    for tr in &event.execution_results {
        // Emit the result lifecycle events, then persist it to context.
        let _ = event_tx
            .send(AgentEvent::ToolResultStart(ToolResultStartEvent {
                base: EventBase::new(),
                reply_id: reply_id.into(),
                tool_call_id: tr.id.clone(),
                tool_call_name: tr.name.clone(),
            }))
            .await;
        let text = match &tr.output {
            agent_scope_message::ToolOutput::Text(t) => t.clone(),
            agent_scope_message::ToolOutput::Blocks(_) => "[blocks]".into(),
        };
        let _ = event_tx
            .send(AgentEvent::ToolResultTextDelta(ToolResultTextDeltaEvent {
                base: EventBase::new(),
                reply_id: reply_id.into(),
                tool_call_id: tr.id.clone(),
                delta: text,
            }))
            .await;
        let _ = event_tx
            .send(AgentEvent::ToolResultEnd(ToolResultEndEvent {
                base: EventBase::new(),
                reply_id: reply_id.into(),
                tool_call_id: tr.id.clone(),
                state: tr.state.clone(),
                metadata: std::collections::HashMap::new(),
                output: match &tr.output {
                    agent_scope_message::ToolOutput::Text(t) => Some(t.clone()),
                    _ => None,
                },
            }))
            .await;

        append_tool_result_to_context(inner, tr.clone());
        set_tool_call_state(inner, &tr.id, ToolCallState::Finished);
    }
}

/// Execute a confirmed tool call, emitting the full ToolCall + ToolResult
/// event sequence and appending the result to context.
async fn execute_confirmed_tool(
    inner: &Arc<AgentInner>,
    tc: &ToolCallBlock,
    event_tx: &mpsc::Sender<AgentEvent>,
    reply_id: &str,
    stream_handle: &StreamHandle,
    cancel_token: &CancellationToken,
) {
    let mut tc_mut = tc.clone();
    // pre_acting middleware hooks (mirrors the acting path). On middleware
    // rejection the tool is not executed and a DENIED result is emitted.
    for mw in inner.middlewares.iter() {
        if let Err(e) = mw.pre_acting(&inner.config.name, &mut tc_mut).await {
            streaming_reactor::emit_denied_tool_result(
                event_tx,
                reply_id,
                inner,
                &tc_mut,
                &format!("Permission denied by middleware: {e}"),
                EventBase::new,
            )
            .await;
            return;
        }
    }

    // Emit the ToolCall lifecycle before execution (Python's acting step).
    let _ = event_tx
        .send(AgentEvent::ToolCallStart(ToolCallStartEvent {
            base: EventBase::new(),
            reply_id: reply_id.into(),
            tool_call_id: tc_mut.id.clone(),
            tool_call_name: tc_mut.name.clone(),
        }))
        .await;
    let _ = event_tx
        .send(AgentEvent::ToolCallDelta(ToolCallDeltaEvent {
            base: EventBase::new(),
            reply_id: reply_id.into(),
            tool_call_id: tc_mut.id.clone(),
            delta: tc_mut.input.clone(),
        }))
        .await;
    let _ = event_tx
        .send(AgentEvent::ToolCallEnd(ToolCallEndEvent {
            base: EventBase::new(),
            reply_id: reply_id.into(),
            tool_call_id: tc_mut.id.clone(),
            input: Some(tc_mut.input.clone()),
        }))
        .await;

    let exec_result = if let Some(tk) = &inner.config.toolkit {
        tk.call_tool(&tc_mut).await
    } else {
        Err(ToolError::NotFound {
            tool_name: tc_mut.name.clone(),
        })
    };

    // Track the consecutive failure streak so error feedback escalates
    // consistently with the acting path.
    let retries = match &exec_result {
        Ok(_) => {
            inner
                .state
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .record_tool_success(&tc_mut.name);
            0
        }
        Err(_) => inner
            .state
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .record_tool_failure(&tc_mut.name),
    };

    // post_acting fires only for Complete results (matches the acting path).
    if let Ok(ToolExecOutput::Complete(chunk)) = &exec_result {
        let result_clone = ToolExecOutput::Complete(chunk.clone());
        for mw in inner.middlewares.iter() {
            let _ = mw.post_acting(&inner.config.name, &result_clone).await;
        }
    }

    let (output_text, result_state) = streaming_reactor::emit_tool_result_and_collect(
        event_tx,
        reply_id,
        &tc_mut,
        exec_result,
        retries,
        stream_handle,
        cancel_token,
    )
    .await;

    // Persist the result to context only when the stream is healthy (a
    // cancelled resume must not leave a half-written result).
    if !stream_handle.is_cancelled()
        && !inner.interrupted.load(std::sync::atomic::Ordering::SeqCst)
        && !cancel_token.is_cancelled()
    {
        streaming_reactor::add_tool_result_to_context(inner, &tc_mut, &output_text, result_state);
    }
}

/// Set the state of a tool call block in the context by id. The block lives in
/// the (tail) assistant message that carried the tool calls; it is searched
/// from the end because a tool result appended during handling becomes the
/// newest message.
fn set_tool_call_state(inner: &Arc<AgentInner>, id: &str, state: ToolCallState) {
    let mut state_guard = inner.state.write().unwrap_or_else(|e| e.into_inner());
    for msg in state_guard.context.iter_mut().rev() {
        for block in &mut msg.content {
            if let ContentBlock::ToolCall(existing) = block
                && existing.id == id
            {
                existing.state = state;
                return;
            }
        }
    }
}

/// Append a pre-built `ToolResultBlock` to the agent's context.
fn append_tool_result_to_context(
    inner: &Arc<AgentInner>,
    trb: agent_scope_message::ToolResultBlock,
) {
    if let Ok(msg) = Msg::new(
        inner.config.name.clone(),
        vec![ContentBlock::ToolResult(trb)],
        Role::Assistant,
    ) {
        inner
            .state
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .context
            .push(msg);
    }
}

/// Convert a message-level `PermissionRule` (the flattened placeholder carried
/// in `ConfirmResult.rules`) into the engine's rule type. Returns `None` when
/// the fields cannot be decoded — the rule is then skipped (a malformed rule
/// must not silently expand the permission surface).
fn to_engine_permission_rule(msg: &agent_scope_message::PermissionRule) -> Option<PermissionRule> {
    let value = serde_json::to_value(msg).ok()?;
    match serde_json::from_value::<PermissionRule>(value) {
        Ok(rule) => Some(rule),
        Err(_) => {
            tracing::warn!("ignoring malformed permission rule in ConfirmResult.rules");
            None
        }
    }
}

fn validation(message: impl Into<String>) -> AgentError {
    AgentError::ValidationError {
        message: message.into(),
    }
}
