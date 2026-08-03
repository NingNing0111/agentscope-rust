//! Structured, actionable feedback for failed tool calls.
//!
//! When a tool call fails (invalid arguments, handler error, ...), the agent
//! writes a concise feedback string back to its context and surfaces it in the
//! event stream. The text tells the model the tool was NOT executed and how to
//! retry — preventing the common failure mode where the model "pretends" the
//! tool succeeded or silently stops.

use agent_scope_tool::ToolError;

/// Build a concise, actionable error feedback string for a failed tool call.
///
/// * `tool_name` — the tool that failed.
/// * `err` — the underlying [`ToolError`].
/// * `retries` — consecutive failure count for this tool this session. When it
///   reaches 2+, the guidance escalates to suggest a strategy change (writing
///   large file content in chunks) instead of blindly re-issuing the call.
pub(crate) fn tool_error_feedback(tool_name: &str, err: &ToolError, retries: u32) -> String {
    let mut out = format!("Tool error: tool '{tool_name}' was NOT executed.\nDetails: {err}");
    if retries >= 2 {
        out.push_str(
            "\nGuidance: re-issue the tool call with one complete, well-formed JSON \
             argument (balanced braces, closed strings, not truncated). If the argument \
             is large file content, prefer writing a placeholder file with Write, then \
             appending via Edit in chunks.",
        );
    } else {
        out.push_str(
            "\nGuidance: re-issue the tool call with one complete, well-formed JSON \
             argument (balanced braces, closed strings, not truncated).",
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_tool::ToolError;

    #[test]
    fn feedback_names_tool_and_execution_status() {
        let err = ToolError::InvalidInput {
            tool_name: "Write".into(),
            reason: "parse error".into(),
        };
        let text = tool_error_feedback("Write", &err, 0);
        assert!(text.contains("tool 'Write' was NOT executed"), "{text}");
        assert!(text.contains("well-formed JSON"), "{text}");
    }

    #[test]
    fn feedback_escalates_after_repeated_failures() {
        let err = ToolError::Execution {
            tool_name: "Write".into(),
            reason: "boom".into(),
        };
        let text = tool_error_feedback("Write", &err, 3);
        assert!(text.contains("in chunks"), "{text}");
        assert!(text.contains("placeholder file"), "{text}");
    }

    #[test]
    fn feedback_does_not_escalate_early() {
        let err = ToolError::Execution {
            tool_name: "Bash".into(),
            reason: "boom".into(),
        };
        let text = tool_error_feedback("Bash", &err, 1);
        assert!(!text.contains("in chunks"), "{text}");
    }
}
