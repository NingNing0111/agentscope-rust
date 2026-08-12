use std::error::Error;
use std::time::Duration;

use agent_scope_agent::AgentError;
use agent_scope_model::ModelError;
use agent_scope_tool::ToolError;

#[test]
fn display_text_for_representative_agent_errors_is_stable() {
    let cases = [
        (
            AgentError::ValidationError {
                message: "bad input".into(),
            },
            "Validation error: bad input",
        ),
        (
            AgentError::TimeoutError {
                operation: "reply".into(),
                duration: Duration::from_secs(2),
            },
            "Timeout: reply after 2s",
        ),
        (
            AgentError::PermissionDenied {
                tool_name: "Bash".into(),
                reason: "requires approval".into(),
            },
            "Permission denied for tool 'Bash': requires approval",
        ),
        (
            AgentError::NoContentToReply,
            "No content to reply to — state context is empty",
        ),
        (
            AgentError::MaxItersExceeded { max_iters: 8 },
            "Max iterations (8) exceeded",
        ),
        (
            AgentError::AlreadyStreaming,
            "A streaming reply is already in progress",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none(), "unexpected source for {expected}");
    }
}

#[test]
fn wrapped_error_display_source_and_from_conversions_are_stable() {
    let model = ModelError::ValidationError {
        field: "messages".into(),
        message: "empty".into(),
    };
    let error = AgentError::from(model);
    assert_eq!(
        error.to_string(),
        "Model error: Validation error on 'messages': empty"
    );
    assert!(error.source().is_some());

    let tool = ToolError::NotFound {
        tool_name: "missing".into(),
    };
    let error = AgentError::from(tool);
    assert_eq!(error.to_string(), "Tool error: tool 'missing' not found");
    assert!(error.source().is_some());
}
