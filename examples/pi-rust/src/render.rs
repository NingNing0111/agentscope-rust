use std::collections::HashMap;

use agent_scope_event::AgentEvent;
use agent_scope_message::ToolResultState;

use crate::error::PiResult;

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub show_events: bool,
    pub show_json_events: bool,
}

/// Cap for buffered tool-output text (per tool call) used to build result
/// lines; prevents unbounded accumulation of huge tool outputs.
const TOOL_OUTPUT_BUFFER_CAP: usize = 2000;

/// Character limit for the excerpt shown in a tool result line.
const TOOL_RESULT_EXCERPT_LIMIT: usize = 200;

#[derive(Debug, Default)]
pub struct RenderedTurn {
    pub text: String,
    pub events: Vec<AgentEvent>,
    /// Human-readable tool activity lines, one per call/result pair.
    pub tool_lines: Vec<String>,
    /// Map of tool_call_id → tool name, captured at `ToolCallStart`.
    #[doc(hidden)]
    pub tool_call_names: HashMap<String, String>,
    /// Accumulated (capped) tool output text per tool_call_id.
    #[doc(hidden)]
    pub tool_outputs: HashMap<String, String>,
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
        AgentEvent::ToolCallStart(start) => {
            // `ToolCallEnd` does not carry the tool name — remember it here.
            turn.tool_call_names
                .insert(start.tool_call_id.clone(), start.tool_call_name.clone());
        }
        AgentEvent::ToolCallEnd(end) if !config.show_events => {
            let name = turn
                .tool_call_names
                .get(&end.tool_call_id)
                .map(String::as_str)
                .unwrap_or("?");
            let line = tool_call_summary(name, end.input.as_deref());
            turn.tool_lines.push(line.clone());
            println!("\n{line}");
        }
        AgentEvent::ToolResultTextDelta(delta) if !config.show_events => {
            // Buffer tool output (capped) so the result line can show a
            // meaningful excerpt of what the tool actually returned.
            let entry = turn
                .tool_outputs
                .entry(delta.tool_call_id.clone())
                .or_default();
            if entry.chars().count() < TOOL_OUTPUT_BUFFER_CAP {
                entry.push_str(&delta.delta);
            }
        }
        AgentEvent::ToolResultEnd(end) if config.show_events => {
            println!("← tool result {} {:?}", end.tool_call_id, end.state);
        }
        AgentEvent::ToolResultEnd(end) => {
            let line = tool_result_line(&turn.tool_outputs, end);
            turn.tool_lines.push(line.clone());
            println!("  {line}");
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

/// Build a compact one-line summary of a tool call for display, extracting the
/// key field from the (often large) JSON argument without leaking content.
///
/// Malformed JSON is common for LLM tool-call streams, so a parse failure
/// falls back to a truncated raw excerpt.
fn tool_call_summary(name: &str, input: Option<&str>) -> String {
    let input = input.unwrap_or("");
    let value = serde_json::from_str::<serde_json::Value>(input);
    let Some(value) = value.ok() else {
        return format!("[{name}] {}", excerpt(input, 120));
    };
    match name {
        "Bash" => {
            let command = value.get("command").and_then(|v| v.as_str()).unwrap_or("");
            format!("[Bash] $ {command}")
        }
        "Write" => {
            let path = value.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let len = value
                .get("content")
                .and_then(|v| v.as_str())
                .map_or(0, |s| s.chars().count());
            format!("[Write] {path} ({len} chars)")
        }
        "Edit" => {
            let path = value.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("[Edit] {path}")
        }
        "Read" => {
            let path = value.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("[Read] {path}")
        }
        "TaskCreate" => {
            let subject = value.get("subject").and_then(|v| v.as_str()).unwrap_or("");
            format!("[TaskCreate] {subject}")
        }
        "TaskUpdate" => {
            let subject = value.get("subject").and_then(|v| v.as_str()).unwrap_or("");
            if subject.is_empty() {
                let task_id = value.get("task_id").and_then(|v| v.as_str()).unwrap_or("?");
                let status = value.get("status").and_then(|v| v.as_str()).unwrap_or("");
                format!("[TaskUpdate] task {task_id} → {status}")
            } else {
                format!("[TaskUpdate] {subject}")
            }
        }
        _ => format!("[{name}] {}", excerpt(input, 120)),
    }
}

/// Build the result line for a finished tool call: `→ success` or `→ error:
/// <excerpt>`.
fn tool_result_line(
    tool_outputs: &HashMap<String, String>,
    end: &agent_scope_event::ToolResultEndEvent,
) -> String {
    let text = tool_outputs
        .get(&end.tool_call_id)
        .map(String::as_str)
        .unwrap_or_else(|| end.output.as_deref().unwrap_or(""));
    match end.state {
        ToolResultState::Success => "→ success".to_string(),
        ToolResultState::Denied => {
            format!("→ denied: {}", excerpt(text, TOOL_RESULT_EXCERPT_LIMIT))
        }
        ToolResultState::Error => format!("→ error: {}", excerpt(text, TOOL_RESULT_EXCERPT_LIMIT)),
        _ => format!("→ {}", excerpt(text, TOOL_RESULT_EXCERPT_LIMIT)),
    }
}

/// Collapse whitespace and truncate `text` to `limit` characters.
fn excerpt(text: &str, limit: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= limit {
        return collapsed;
    }
    let mut out: String = collapsed.chars().take(limit).collect();
    out.push_str("… (truncated)");
    out
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

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_event::{
        EventBase, ToolCallEndEvent, ToolCallStartEvent, ToolResultEndEvent, ToolResultStartEvent,
        ToolResultTextDeltaEvent,
    };

    fn base() -> EventBase {
        EventBase::new()
    }

    fn config() -> RenderConfig {
        RenderConfig {
            show_events: false,
            show_json_events: false,
        }
    }

    fn render_all(events: Vec<AgentEvent>, turn: &mut RenderedTurn, cfg: &RenderConfig) {
        for event in events {
            render_event(event, cfg, turn).unwrap();
        }
    }

    #[test]
    fn bash_call_renders_one_call_and_one_result_line() {
        let mut turn = RenderedTurn::default();
        let events = vec![
            AgentEvent::ToolCallStart(ToolCallStartEvent {
                base: base(),
                reply_id: "r".into(),
                tool_call_id: "tc-1".into(),
                tool_call_name: "Bash".into(),
            }),
            AgentEvent::ToolCallEnd(ToolCallEndEvent {
                base: base(),
                reply_id: "r".into(),
                tool_call_id: "tc-1".into(),
                input: Some(r#"{"command":"ls -la"}"#.into()),
            }),
            AgentEvent::ToolResultStart(ToolResultStartEvent {
                base: base(),
                reply_id: "r".into(),
                tool_call_id: "tc-1".into(),
                tool_call_name: "Bash".into(),
            }),
            AgentEvent::ToolResultTextDelta(ToolResultTextDeltaEvent {
                base: base(),
                reply_id: "r".into(),
                tool_call_id: "tc-1".into(),
                delta: "exit_code: 0\nstdout:\nfile1\nfile2\n".into(),
            }),
            AgentEvent::ToolResultEnd(ToolResultEndEvent {
                base: base(),
                reply_id: "r".into(),
                tool_call_id: "tc-1".into(),
                state: ToolResultState::Success,
                metadata: Default::default(),
                output: Some("exit_code: 0".into()),
            }),
        ];
        render_all(events, &mut turn, &config());

        assert_eq!(turn.tool_lines.len(), 2, "{:?}", turn.tool_lines);
        assert!(
            turn.tool_lines[0].contains("[Bash] $ ls -la"),
            "{:?}",
            turn.tool_lines
        );
        assert!(
            turn.tool_lines[1].contains("→ success"),
            "{:?}",
            turn.tool_lines
        );
        // Tool-output delta fragments must never leak into activity lines.
        assert!(!turn.tool_lines.iter().any(|l| l.contains("file1")));
    }

    #[test]
    fn write_call_summary_hides_content() {
        let summary = tool_call_summary(
            "Write",
            Some(r#"{"path":"out.txt","content":"SECRET_DATA"}"#),
        );
        assert!(summary.contains("[Write] out.txt (11 chars)"), "{summary}");
        assert!(!summary.contains("SECRET"), "{summary}");
    }

    #[test]
    fn malformed_input_falls_back_to_excerpt() {
        // The real bug: a trailing `}}` that would previously make the whole
        // call invisible. The activity line still renders via raw excerpt.
        let summary = tool_call_summary("Bash", Some(r#"{"command":"x"}}"#));
        assert!(summary.starts_with("[Bash] "), "{summary}");
    }

    #[test]
    fn error_result_line_is_truncated() {
        let mut outputs = HashMap::new();
        outputs.insert("tc-9".to_string(), "x".repeat(500));
        let end = ToolResultEndEvent {
            base: base(),
            reply_id: "r".into(),
            tool_call_id: "tc-9".into(),
            state: ToolResultState::Error,
            metadata: Default::default(),
            output: Some("y".repeat(300)),
        };
        let line = tool_result_line(&outputs, &end);
        assert!(line.starts_with("→ error: "), "{line}");
        assert!(line.contains("truncated"), "{line}");
        assert!(
            line.chars().count() <= 240,
            "excerpt too long: {}",
            line.chars().count()
        );
    }

    #[test]
    fn malformed_write_call_still_renders_activity_lines() {
        let mut turn = RenderedTurn::default();
        let events = vec![
            AgentEvent::ToolCallStart(ToolCallStartEvent {
                base: base(),
                reply_id: "r".into(),
                tool_call_id: "tc-w".into(),
                tool_call_name: "Write".into(),
            }),
            AgentEvent::ToolCallEnd(ToolCallEndEvent {
                base: base(),
                reply_id: "r".into(),
                tool_call_id: "tc-w".into(),
                input: Some(r#"{"path":"a.html","content":"x"}}"#.into()),
            }),
            AgentEvent::ToolResultStart(ToolResultStartEvent {
                base: base(),
                reply_id: "r".into(),
                tool_call_id: "tc-w".into(),
                tool_call_name: "Write".into(),
            }),
            AgentEvent::ToolResultEnd(ToolResultEndEvent {
                base: base(),
                reply_id: "r".into(),
                tool_call_id: "tc-w".into(),
                state: ToolResultState::Error,
                metadata: Default::default(),
                output: None,
            }),
        ];
        render_all(events, &mut turn, &config());

        assert_eq!(turn.tool_lines.len(), 2, "{:?}", turn.tool_lines);
        assert!(
            turn.tool_lines[0].starts_with("[Write] "),
            "{:?}",
            turn.tool_lines
        );
        assert!(
            turn.tool_lines[1].starts_with("→ error"),
            "{:?}",
            turn.tool_lines
        );
    }
}
