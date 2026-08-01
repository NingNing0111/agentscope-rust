use agent_scope_tool::{ToolError, ToolExecOutput};

#[test]
fn tool_error_display_includes_actionable_context() {
    let cases = [
        (
            ToolError::NotFound {
                tool_name: "missing".into(),
            },
            "tool 'missing' not found",
        ),
        (
            ToolError::InvalidInput {
                tool_name: "search".into(),
                reason: "expected object".into(),
            },
            "invalid input for tool 'search': expected object",
        ),
        (
            ToolError::Execution {
                tool_name: "calc".into(),
                reason: "divide by zero".into(),
            },
            "tool 'calc' execution failed: divide by zero",
        ),
        (
            ToolError::Interrupted {
                tool_name: "slow".into(),
            },
            "tool 'slow' was interrupted",
        ),
        (
            ToolError::SkillNotFound {
                skill_name: "docs".into(),
            },
            "skill 'docs' not found",
        ),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

#[test]
fn tool_exec_output_debug_hides_stream_internals() {
    use futures::stream;
    use std::pin::Pin;

    let stream = stream::empty();
    let output = ToolExecOutput::Stream(Pin::from(Box::new(stream)));

    assert_eq!(format!("{output:?}"), "Stream(\"<stream>\")");
}
