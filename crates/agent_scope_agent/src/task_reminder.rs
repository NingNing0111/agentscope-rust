//! Task reminder injection — the task dimension of the Python runtime-state
//! injection (`agentscope/src/agentscope/agent/_agent.py::_inject_runtime_state`,
//! upstream commit `9d1026fa`).
//!
//! When unfinished tasks exist and the conversation context no longer shows
//! task-tool activity (e.g. the tool calls were compressed away), a hint block
//! is appended to the persistent context reminding the agent of the remaining
//! task counts. Awareness detection re-uses a fixed source identifier so the
//! reminder is only injected once until a compression removes it.
//!
//! Feature 026 introduced the unified [`runtime_injection`](crate::runtime_injection)
//! pipeline that adds time and context-length dimensions. This module keeps the
//! Feature 024 task-only behavior byte-for-byte identical so existing tests and
//! call sites don't regress; the unified pipeline is used by the loop call
//! sites (react_loop / streaming_reactor) for the full three-dimensional
//! injection.

use std::sync::RwLock;

use agent_scope_message::{ContentBlock, HintBlock, HintBlockItem, HintContent, Msg, Role};
use agent_scope_state::{AgentState, TaskState};

use crate::task_tools::TASK_TOOL_NAMES;

/// Source identifier of the injected hint block, used to detect whether a
/// previous injection already exists in the context. Aligns with the Python
/// `InjectionConfig.injection_source`.
pub(crate) const TASK_REMINDER_SOURCE: &str = r#"{"label": "System", "sublabel": "Runtime State"}"#;

/// Template wrapping the injected runtime-state fields. Aligns with the
/// Python `InjectionConfig.template`; `{runtime_state}` is replaced with the
/// `<tasks>...</tasks>` field.
const TASK_REMINDER_TEMPLATE: &str = "<system-reminder>Treat the following as the ground truth at this point of the conversation. Anything stated earlier is outdated, and a later reminder, if any, supersedes this one:\n{runtime_state}\n</system-reminder>";

/// Evaluate and inject the unfinished-task reminder into the agent context.
///
/// Returns `true` when a reminder was injected. Called once per reasoning
/// iteration in both the batch and streaming loops. The evaluation and the
/// append happen under a single write lock so concurrent tool execution cannot
/// interleave a duplicate injection.
pub fn maybe_inject_task_reminder(state: &RwLock<AgentState>, agent_name: &str) -> bool {
    let mut state = state.write().unwrap_or_else(|e| e.into_inner());

    let mut in_progress = 0usize;
    let mut pending = 0usize;
    for task in &state.tasks_context.tasks {
        match task.state {
            TaskState::Pending => pending += 1,
            TaskState::InProgress => in_progress += 1,
            TaskState::Completed => {}
        }
    }
    if pending == 0 && in_progress == 0 {
        return false;
    }

    if context_aware_of_tasks(&state.context) {
        return false;
    }

    let tasks_text = format!(
        "You have {in_progress} in-progress tasks and {pending} pending tasks. Use `TaskList` to view them if you don't know."
    );
    let hint =
        TASK_REMINDER_TEMPLATE.replace("{runtime_state}", &format!("<tasks>{tasks_text}</tasks>"));
    let block = HintBlock {
        hint: HintContent::Text(hint),
        source: Some(TASK_REMINDER_SOURCE.to_string()),
        id: agent_scope_utils::id::generate_id(),
        created_at: chrono::Utc::now().to_rfc3339(),
        finished_at: None,
    };

    if let Ok(msg) = Msg::new(
        agent_name.into(),
        vec![ContentBlock::Hint(block)],
        Role::Assistant,
    ) {
        state.context.push(msg);
        true
    } else {
        false
    }
}

/// Whether the context already shows task-tool activity or a previous tasks
/// reminder, scanning assistant messages in reverse (matches the Python scan).
fn context_aware_of_tasks(context: &[Msg]) -> bool {
    for msg in context.iter().rev() {
        if msg.role != Role::Assistant {
            continue;
        }
        for block in msg.content.iter().rev() {
            match block {
                ContentBlock::ToolCall(tc) if TASK_TOOL_NAMES.contains(&tc.name.as_str()) => {
                    return true;
                }
                ContentBlock::Hint(hb)
                    if hb.source.as_deref() == Some(TASK_REMINDER_SOURCE)
                        && hint_text(hb).contains("<tasks>") =>
                {
                    return true;
                }
                _ => {}
            }
        }
    }
    false
}

/// Extract the plain-text content of a hint block.
fn hint_text(hb: &HintBlock) -> String {
    match &hb.hint {
        HintContent::Text(t) => t.clone(),
        HintContent::Blocks(items) => items
            .iter()
            .filter_map(|item| match item {
                HintBlockItem::Text(t) => Some(t.text.clone()),
                HintBlockItem::Data(_) => None,
            })
            .collect::<Vec<_>>()
            .join(""),
    }
}
