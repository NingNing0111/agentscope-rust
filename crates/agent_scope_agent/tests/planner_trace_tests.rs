use agent_scope_agent::{
    Plan, PlannerOutcome, PlanningEvent, PlanningEventType, PlanningTrace, planner_safe_summary,
    redact_json,
};

#[test]
fn planning_trace_assigns_monotonic_sequences() {
    let mut trace = PlanningTrace::new("task-1");
    trace
        .push(PlanningEvent::new(
            PlanningEventType::PlanningStarted,
            "task-1",
        ))
        .unwrap();
    trace
        .push(PlanningEvent::new(
            PlanningEventType::PlanningCompleted,
            "task-1",
        ))
        .unwrap();

    assert_eq!(trace.events[0].sequence, 1);
    assert_eq!(trace.events[1].sequence, 2);
    trace.validate().unwrap();
}

#[test]
fn planning_trace_rejects_wrong_task_id() {
    let mut trace = PlanningTrace::new("task-1");
    let err = trace
        .push(PlanningEvent::new(
            PlanningEventType::PlanningStarted,
            "task-2",
        ))
        .unwrap_err();
    assert!(err.to_string().contains("task_id"));
}

#[test]
fn finished_trace_contains_terminal_task_event() {
    let mut trace = PlanningTrace::new("task-1");
    trace
        .finish(PlannerOutcome::Completed {
            summary: "done".into(),
        })
        .unwrap();

    assert!(trace.final_outcome.is_some());
    assert!(
        trace
            .events
            .iter()
            .any(|event| event.event_type == PlanningEventType::TaskCompleted)
    );
    trace.validate().unwrap();
}

#[test]
fn summary_redacts_secret_like_tokens() {
    let summary = planner_safe_summary("api_key=abc token=def normal=value");
    assert!(summary.contains("[REDACTED]"));
    assert!(!summary.contains("abc"));
    assert!(summary.contains("normal=value"));
}

#[test]
fn json_redaction_redacts_nested_secret_keys() {
    let value = serde_json::json!({
        "tool_arguments": {
            "path": "workspace/report.md",
            "access_token": "secret-token"
        }
    });
    let redacted = redact_json(&value);
    assert_eq!(redacted["tool_arguments"]["access_token"], "[REDACTED]");
    assert_eq!(redacted["tool_arguments"]["path"], "workspace/report.md");
}

#[test]
fn planning_trace_serializes_to_compat_json() {
    let mut trace = PlanningTrace::new("task-1");
    let plan = Plan::new("plan-1", "task-1", "objective", vec!["step".into()]);
    trace
        .push_event(
            PlanningEventType::PlanningCompleted,
            Some(plan.plan_id),
            None,
            Some("api_key=abc".into()),
        )
        .unwrap();
    let json = trace.to_compat_json().unwrap();
    assert_eq!(json["task_id"], "task-1");
    assert_eq!(json["events"][0]["sequence"], 1);
}
