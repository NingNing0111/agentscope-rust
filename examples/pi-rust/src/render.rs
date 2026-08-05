use std::collections::HashMap;
use std::path::PathBuf;

use agent_scope_event::{AgentEvent, ToolResultEndEvent};
use agent_scope_message::ToolResultState;

use crate::error::PiResult;
use crate::tools::{ToolResultShape, approval_fingerprint};

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub show_events: bool,
    pub show_json_events: bool,
    /// Workspace root, used to normalize Write paths into approval fingerprints.
    pub cwd: PathBuf,
}

/// Cap for buffered tool-output text (per tool call) used to build result
/// lines; prevents unbounded accumulation of huge tool outputs.
const TOOL_OUTPUT_BUFFER_CAP: usize = 2000;

/// Character limit for the excerpt shown in a tool result line.
const TOOL_RESULT_EXCERPT_LIMIT: usize = 200;

/// An operation the host should offer for approval after a tool was denied
/// with `confirmation_required`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmationCandidate {
    pub tool_name: String,
    /// Exact fingerprint the tool checks against its approvals set.
    pub fingerprint: String,
    /// Human-readable description, e.g. `[Bash] $ rm -rf x`.
    pub description: String,
}

#[derive(Debug, Default)]
pub struct RenderedTurn {
    pub text: String,
    pub events: Vec<AgentEvent>,
    /// Human-readable tool activity lines, one per call/result pair.
    pub tool_lines: Vec<String>,
    /// Map of tool_call_id → tool name, captured at `ToolCallStart`.
    #[doc(hidden)]
    pub tool_call_names: HashMap<String, String>,
    /// Map of tool_call_id → raw input JSON, captured at `ToolCallEnd`.
    #[doc(hidden)]
    pub tool_call_inputs: HashMap<String, String>,
    /// Accumulated (capped) tool output text per tool_call_id.
    #[doc(hidden)]
    pub tool_outputs: HashMap<String, String>,
    /// Operations denied with `confirmation_required` in this turn.
    pub confirmation_candidates: Vec<ConfirmationCandidate>,
    /// Whether the turn was interrupted by the host (Ctrl+C).
    pub interrupted: bool,
}

/// Process one agent event: collect turn state and return the text the line
/// REPL should print for it (empty vec = nothing to print).
///
/// The returned chunks preserve the original `println!`/`print!` semantics
/// (`TextBlockDelta` yields a `print` block without trailing newline; most
/// other events yield a newline-terminated line). The TUI ignores the return
/// value and renders events itself; confirmation-candidate collection and the
/// turn fields are shared by both frontends.
pub fn render_event(
    event: AgentEvent,
    config: &RenderConfig,
    turn: &mut RenderedTurn,
) -> PiResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    if config.show_json_events {
        out.push(format!("{}\n", serde_json::to_string(&event)?));
    }
    match &event {
        AgentEvent::TextBlockDelta(delta) => {
            out.push(delta.delta.clone());
            turn.text.push_str(&delta.delta);
        }
        AgentEvent::ToolCallStart(start) => {
            // `ToolCallEnd` does not carry the tool name — remember it here
            // regardless of event verbosity (confirmation collection needs it).
            turn.tool_call_names
                .insert(start.tool_call_id.clone(), start.tool_call_name.clone());
            if config.show_events {
                out.push(format!(
                    "\n→ tool {} ({})\n",
                    start.tool_call_name, start.tool_call_id
                ));
            }
        }
        AgentEvent::ToolCallEnd(end) => {
            // Raw input JSON is needed to derive approval fingerprints later.
            if let Some(input) = &end.input {
                turn.tool_call_inputs
                    .insert(end.tool_call_id.clone(), input.clone());
            }
            if !config.show_events {
                let name = turn
                    .tool_call_names
                    .get(&end.tool_call_id)
                    .map(String::as_str)
                    .unwrap_or("?");
                let line = tool_call_summary(name, end.input.as_deref());
                turn.tool_lines.push(line.clone());
                out.push(format!("\n{line}\n"));
            }
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
        AgentEvent::ToolResultEnd(end) => {
            collect_confirmation_candidates(turn, config, end);
            if config.show_events {
                out.push(format!(
                    "← tool result {} {:?}\n",
                    end.tool_call_id, end.state
                ));
            } else {
                let line = tool_result_line(&turn.tool_outputs, end);
                turn.tool_lines.push(line.clone());
                out.push(format!("  {line}\n"));
            }
        }
        AgentEvent::UserInterrupt(_) => {
            turn.interrupted = true;
            out.push("\n[interrupted]\n".to_string());
        }
        AgentEvent::RequireUserConfirm(confirm) if config.show_events => {
            out.push(format!(
                "? confirmation required for {} tool call(s)\n",
                confirm.tool_calls.len()
            ));
        }
        AgentEvent::ReplyEnd(_) => {
            out.push("\n".to_string());
        }
        _ if config.show_events => {
            out.push(format!("event: {}\n", event_name(&event)));
        }
        _ => {}
    }
    turn.events.push(event);
    Ok(out)
}

/// Collect operations the host should offer for approval.
///
/// A `Denied` tool result whose output is a `ToolResultShape` with
/// `error.code == "confirmation_required"` becomes a [`ConfirmationCandidate`]
/// (deduplicated by value). This runs regardless of `show_events`, so the REPL
/// can always offer approvals.
fn collect_confirmation_candidates(
    turn: &mut RenderedTurn,
    config: &RenderConfig,
    end: &ToolResultEndEvent,
) {
    if end.state != ToolResultState::Denied {
        return;
    }
    // The output carries the full ToolResultShape JSON (batch and streaming
    // paths both attach the tool output text to ToolResultEnd).
    let text = end
        .output
        .as_deref()
        .or_else(|| turn.tool_outputs.get(&end.tool_call_id).map(String::as_str))
        .unwrap_or("");
    let Ok(shape) = serde_json::from_str::<ToolResultShape>(text) else {
        return;
    };
    let Some(error) = &shape.error else { return };
    if error.code != "confirmation_required" {
        return;
    }
    let Some(tool_name) = turn.tool_call_names.get(&end.tool_call_id) else {
        return;
    };
    let Some(input_raw) = turn.tool_call_inputs.get(&end.tool_call_id) else {
        return;
    };
    let Ok(input_json) = serde_json::from_str::<serde_json::Value>(input_raw) else {
        return;
    };
    let Some(fingerprint) = approval_fingerprint(tool_name, &input_json, &config.cwd) else {
        return;
    };
    let description = match tool_name.as_str() {
        "Bash" => input_json
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(|command| format!("[Bash] $ {command}"))
            .unwrap_or_else(|| "[Bash]".to_string()),
        "Write" => input_json
            .get("path")
            .and_then(serde_json::Value::as_str)
            .map(|path| format!("[Write] {path}"))
            .unwrap_or_else(|| "[Write]".to_string()),
        _ => format!("[{tool_name}]"),
    };
    let candidate = ConfirmationCandidate {
        tool_name: tool_name.clone(),
        fingerprint,
        description,
    };
    if !turn.confirmation_candidates.contains(&candidate) {
        turn.confirmation_candidates.push(candidate);
    }
}

/// Build a compact one-line summary of a tool call for display, extracting the
/// key field from the (often large) JSON argument without leaking content.
///
/// Malformed JSON is common for LLM tool-call streams, so a parse failure
/// falls back to a truncated raw excerpt.
pub fn tool_call_summary(name: &str, input: Option<&str>) -> String {
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
pub fn tool_result_line(
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
            cwd: PathBuf::from("."),
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
