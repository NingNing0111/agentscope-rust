use agent_scope_agent::{AgentError, PlannerError, PlannerErrorCategory};

#[test]
fn planner_error_display_includes_category() {
    let err = PlannerError::new(PlannerErrorCategory::MalformedPlan, "bad plan");
    assert_eq!(err.to_string(), "MalformedPlan: bad plan");
}

#[test]
fn unsupported_error_is_not_retryable() {
    let err = PlannerError::unsupported("parallel DAG execution");
    assert_eq!(err.category, PlannerErrorCategory::UnsupportedCapability);
    assert!(!err.retryable);
    assert!(err.message.contains("parallel DAG"));
}

#[test]
fn planner_error_converts_to_agent_error() {
    let err = PlannerError::new(PlannerErrorCategory::InvalidGoal, "empty goal");
    let agent_err = AgentError::from(err);
    assert!(matches!(agent_err, AgentError::ValidationError { .. }));
}

#[test]
fn agent_error_converts_to_planner_error() {
    let agent_err = AgentError::NoContentToReply;
    let planner_err = PlannerError::from(agent_err);
    assert_eq!(planner_err.category, PlannerErrorCategory::InvalidGoal);
}

#[test]
fn permission_agent_error_maps_to_permission_planner_error() {
    let agent_err = AgentError::PermissionDenied {
        tool_name: "write_file".into(),
        reason: "blocked".into(),
    };
    let planner_err = PlannerError::from(agent_err);
    assert_eq!(planner_err.category, PlannerErrorCategory::PermissionDenied);
    assert!(!planner_err.retryable);
}
