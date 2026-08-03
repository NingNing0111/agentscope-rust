use agent_scope_event::AgentEvent;

use crate::error::PiResult;

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub show_events: bool,
    pub show_json_events: bool,
}

#[derive(Debug, Default)]
pub struct RenderedTurn {
    pub text: String,
    pub events: Vec<AgentEvent>,
}

pub fn render_event(
    event: AgentEvent,
    config: &RenderConfig,
    turn: &mut RenderedTurn,
) -> PiResult<()> {
    if config.show_json_events {
        println!("{}", serde_json::to_string(&event)?);
    }
    match &event {
        AgentEvent::TextBlockDelta(delta) => {
            print!("{}", delta.delta);
            turn.text.push_str(&delta.delta);
        }
        AgentEvent::ToolCallStart(start) if config.show_events => {
            println!("\n→ tool {} ({})", start.tool_call_name, start.tool_call_id);
        }
        AgentEvent::ToolResultEnd(end) if config.show_events => {
            println!("← tool result {} {:?}", end.tool_call_id, end.state);
        }
        AgentEvent::RequireUserConfirm(confirm) if config.show_events => {
            println!(
                "? confirmation required for {} tool call(s)",
                confirm.tool_calls.len()
            );
        }
        AgentEvent::ReplyEnd(_) => {
            println!();
        }
        _ if config.show_events => {
            println!("event: {}", event_name(&event));
        }
        _ => {}
    }
    turn.events.push(event);
    Ok(())
}

pub fn event_name(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::ReplyStart(_) => "REPLY_START",
        AgentEvent::ReplyEnd(_) => "REPLY_END",
        AgentEvent::ModelCallStart(_) => "MODEL_CALL_START",
        AgentEvent::ModelCallEnd(_) => "MODEL_CALL_END",
        AgentEvent::TextBlockStart(_) => "TEXT_BLOCK_START",
        AgentEvent::TextBlockDelta(_) => "TEXT_BLOCK_DELTA",
        AgentEvent::TextBlockEnd(_) => "TEXT_BLOCK_END",
        AgentEvent::DataBlockStart(_) => "DATA_BLOCK_START",
        AgentEvent::DataBlockDelta(_) => "DATA_BLOCK_DELTA",
        AgentEvent::DataBlockEnd(_) => "DATA_BLOCK_END",
        AgentEvent::ThinkingBlockStart(_) => "THINKING_BLOCK_START",
        AgentEvent::ThinkingBlockDelta(_) => "THINKING_BLOCK_DELTA",
        AgentEvent::ThinkingBlockEnd(_) => "THINKING_BLOCK_END",
        AgentEvent::HintBlock(_) => "HINT_BLOCK",
        AgentEvent::ToolCallStart(_) => "TOOL_CALL_START",
        AgentEvent::ToolCallDelta(_) => "TOOL_CALL_DELTA",
        AgentEvent::ToolCallEnd(_) => "TOOL_CALL_END",
        AgentEvent::ToolResultStart(_) => "TOOL_RESULT_START",
        AgentEvent::ToolResultTextDelta(_) => "TOOL_RESULT_TEXT_DELTA",
        AgentEvent::ToolResultDataDelta(_) => "TOOL_RESULT_DATA_DELTA",
        AgentEvent::ToolResultEnd(_) => "TOOL_RESULT_END",
        AgentEvent::ExceedMaxIters(_) => "EXCEED_MAX_ITERS",
        AgentEvent::RequireUserConfirm(_) => "REQUIRE_USER_CONFIRM",
        AgentEvent::UserConfirmResult(_) => "USER_CONFIRM_RESULT",
        AgentEvent::UserInterrupt(_) => "USER_INTERRUPT",
        AgentEvent::RequireExternalExecution(_) => "REQUIRE_EXTERNAL_EXECUTION",
        AgentEvent::ExternalExecutionResult(_) => "EXTERNAL_EXECUTION_RESULT",
        AgentEvent::Custom(_) => "CUSTOM",
        AgentEvent::SessionCreated(_) => "SESSION_CREATED",
        AgentEvent::SessionClosed(_) => "SESSION_CLOSED",
        AgentEvent::SessionSaved(_) => "SESSION_SAVED",
        AgentEvent::SessionLoaded(_) => "SESSION_LOADED",
        AgentEvent::SessionTrimmed(_) => "SESSION_TRIMMED",
    }
}

#[allow(dead_code)]
pub fn truncate_visible(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push_str(&format!("\n... truncated to {limit} characters"));
    out
}
